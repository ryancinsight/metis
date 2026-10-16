"""Analytical and scalar differential proofs of every output mask byte."""

import itertools
import pathlib
import sys
import unittest


sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
from visual_tools import contract, difference


def scalar_reference(expected, actual):
    """Independent scalar baseline checks all four channels of every pixel."""
    width, height, left = expected
    right = actual[2]
    changed = []
    mask = bytearray()
    for index in range(width * height):
        mismatch = any(left[4 * index + channel] != right[4 * index + channel]
                       for channel in range(4))
        mask.extend((0, 0, 255 if mismatch else 0, 255))
        if mismatch:
            changed.append((index % width, index // width))
    bounds = ([min(x for x, _ in changed), min(y for _, y in changed),
               max(x for x, _ in changed) + 1, max(y for _, y in changed) + 1]
              if changed else None)
    return {"changed_pixels": len(changed), "bounds": bounds}, (width, height, bytes(mask))


class ExactDifference(unittest.TestCase):
    def test_every_small_grid_subset_and_channel_has_analytical_mask(self):
        for width, height in ((1, 1), (1, 3), (3, 1), (2, 2), (3, 2)):
            size = width * height
            left = bytes((31, 47, 53, 61)) * size
            for subset, channel in itertools.product(range(1 << size), range(4)):
                selected = [index for index in range(size) if subset & (1 << index)]
                right = bytearray(left)
                for index in selected:
                    right[4 * index + channel] ^= 1
                bounds = ([min(i % width for i in selected), min(i // width for i in selected),
                           max(i % width for i in selected) + 1, max(i // width for i in selected) + 1]
                          if selected else None)
                mask = b"".join(bytes((0, 0, 255 if subset & (1 << index) else 0, 255))
                                for index in range(size))
                with self.subTest(width=width, height=height, subset=subset, channel=channel):
                    result = difference.compare((width, height, left), (width, height, bytes(right)))
                    self.assertEqual(result, ({"changed_pixels": len(selected), "bounds": bounds},
                                              (width, height, mask)))
                    self.assertEqual(result, scalar_reference((width, height, left),
                                                              (width, height, bytes(right))))
                    self.assertEqual(left, bytes((31, 47, 53, 61)) * size)

    def test_maximum_dimensions_equal_dense_alternating_sparse_match_scalar(self):
        for width, height in ((1, 600), (800, 1), (800, 600)):
            size = width * height
            left = bytes((31, 47, 53, 61)) * size
            cases = {
                "equal": (),
                "dense": range(size),
                "alternating": range(0, size, 2),
                "sparse": sorted({0, width - 1, (height - 1) * width, size - 1}),
            }
            for name, selected in cases.items():
                right = bytearray(left)
                for index in selected:
                    right[4 * index + index % 4] ^= 1
                before = bytes(right)
                with self.subTest(width=width, height=height, case=name):
                    expected = (width, height, left)
                    actual = (width, height, right)
                    self.assertEqual(difference.compare(expected, actual), scalar_reference(expected, actual))
                    self.assertEqual(bytes(right), before)
                    self.assertEqual(left, bytes((31, 47, 53, 61)) * size)

    def test_prior_sliceable_buffers_preserve_computed_outputs(self):
        left = bytes((31, 47, 53, 61)) * 6
        right = bytearray(left)
        right[4 * 2 + 3] ^= 1
        right[4 * 5] ^= 1
        representations = (
            (left, bytes(right)),
            (bytearray(left), right),
            (list(left), list(right)),
            (memoryview(left), memoryview(right)),
            (memoryview(b"".join(bytes((value, 99)) for value in left))[::2],
             memoryview(b"".join(bytes((value, 99)) for value in right))[::2]),
        )
        wanted = scalar_reference((3, 2, left), (3, 2, right))
        for a, b in representations:
            with self.subTest(representation=type(a).__name__):
                self.assertEqual(difference.compare((3, 2, a), (3, 2, b)), wanted)

    def test_existing_dimension_and_length_failures_keep_context(self):
        cases = (
            ((1, 1, bytes(4)), (2, 1, bytes(8)), "Cannot compare different image dimensions"),
            ((0, 1, b""), (0, 1, b""), "Image dimensions exceed"),
            ((801, 1, bytes(3204)), (801, 1, bytes(3204)), "Image dimensions exceed"),
            ((1, 601, bytes(2404)), (1, 601, bytes(2404)), "Image dimensions exceed"),
            ((1, 1, bytes(3)), (1, 1, bytes(4)), "Difference pixel lengths disagree"),
            ((1, 1, bytes(5)), (1, 1, bytes(5)), "Difference pixel lengths disagree"),
        )
        for expected, actual, message in cases:
            with self.subTest(expected=expected[:2], actual=actual[:2]), self.assertRaisesRegex(
                contract.VisualError, message
            ):
                difference.compare(expected, actual)


if __name__ == "__main__":
    unittest.main()
