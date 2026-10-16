"""Exact changed-pixel count, half-open bounds, and complete BGRA mask."""

from . import contract


def compare(expected, actual):
    """Return exact count, half-open bounds and every byte of a red/black mask.

    Equal rows discharge all their disjoint four-byte pixels. Unequal rows
    retain the prior pixel-pair comparison, including alpha. Increasing row
    order supplies the first/last changed y; changed pixels supply x extrema.
    The mask starts black with alpha 255; only a changed pixel's red byte moves
    to 255. See design.md for the invariant and complete scalar equivalence.
    """
    contract.require(expected[:2] == actual[:2], "Cannot compare different image dimensions")
    width, height, left = expected
    right = actual[2]
    contract.dimensions(width, height)
    contract.require(len(left) == len(right) == width * height * 4, "Difference pixel lengths disagree")
    changed, min_x, min_y, max_x, max_y = 0, width, height, 0, 0
    mask = bytearray(bytes((0, 0, 0, 255)) * (width * height))
    stride = width * 4
    for y in range(height):
        row_start = y * stride
        left_row = left[row_start:row_start + stride]
        right_row = right[row_start:row_start + stride]
        if left_row == right_row:
            continue
        if changed == 0:
            min_y = y
        max_y = y
        for x in range(width):
            start = x * 4
            if left_row[start:start + 4] != right_row[start:start + 4]:
                changed += 1
                if x < min_x:
                    min_x = x
                if x > max_x:
                    max_x = x
                mask[row_start + start + 2] = 255
    bounds = [min_x, min_y, max_x + 1, max_y + 1] if changed else None
    return {"changed_pixels": changed, "bounds": bounds}, (width, height, bytes(mask))
