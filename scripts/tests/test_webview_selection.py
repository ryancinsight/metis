"""Selection evidence admission preserves native orders and absence states."""

import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from webview_acceptance.progress import Progress
from webview_acceptance.selection import admit


class SelectionEvidence(unittest.TestCase):
    def test_absence_and_unique_endpoint_orders_survive_retention(self):
        vectors = [
            {"available": False, "supported": None, "range_count": None, "comparisons": None},
            {"available": True, "supported": "None", "range_count": None, "comparisons": None},
            {"available": True, "supported": "Single", "range_count": 0, "comparisons": None},
            {"available": True, "supported": "Multiple", "range_count": 2, "comparisons": None},
        ]
        # CompareEndpoints specifies sign, not distance. Preserve arbitrary
        # signed values, degenerate ranges, full selection and inconsistent
        # non-atomic observations without inferring replacement success.
        for orders in ((0, 0, -257, -257), (0, -257, 0, -257),
                       (257, -257, -1, -32896), (1, 1, 1, 1),
                       (-0x80000000, 0x7FFFFFFF, 0, -1)):
            vectors.append({"available": True, "supported": "Single", "range_count": 1,
                            "comparisons": dict(zip(("start_to_document_start", "end_to_document_end",
                                                     "start_to_end", "document_start_to_end"), orders))})
        with tempfile.TemporaryDirectory() as directory:
            progress = Progress(Path(directory))
            for value in vectors:
                with self.subTest(value=value):
                    admitted = admit(value)
                    self.assertIs(admitted, value)
                    progress.input({"selections": {"after_select": admitted}})
                    progress.begin("snapshot", "patient")
                    actual = json.loads((Path(directory) / "uia-progress.json").read_bytes())
                    self.assertEqual(actual["input"]["observation"]["selections"]["after_select"], value)
                    self.assertEqual(actual["sequence"], str(progress.sequence))

    def test_invalid_shapes_and_absence_claims_reject_before_retention(self):
        absent = {"available": False, "supported": None, "range_count": None, "comparisons": None}
        invalid = [None, [], {}, absent | {"extra": None}, absent | {"available": 0},
                   absent | {"supported": "Single"}, absent | {"range_count": 0},
                   absent | {"comparisons": {}}, absent | {"available": True},
                   absent | {"available": True, "supported": "None", "range_count": 0},
                   absent | {"available": True, "supported": "Unknown"}]
        with tempfile.TemporaryDirectory() as directory:
            progress = Progress(Path(directory))
            progress.input({"selection": admit(absent)})
            path = Path(directory) / "uia-progress.json"
            original = path.read_bytes()
            for value in invalid:
                with self.subTest(value=value):
                    with self.assertRaisesRegex(ValueError, "UIA") as caught:
                        progress.input({"selection": admit(value)})
                    self.assertGreater(len(str(caught.exception)), len("UIA"))
                    self.assertEqual(path.read_bytes(), original)

    def test_count_and_native_signed_integer_boundaries_remain_distinct(self):
        base = {"available": True, "supported": "Multiple", "range_count": 2, "comparisons": None}
        for count in (0, 2, 0x7FFFFFFF):
            self.assertEqual(admit(base | {"range_count": count})["range_count"], count)
        for count in (None, True, -1, 0x80000000, 1.0, "1"):
            with self.subTest(count=count), self.assertRaisesRegex(ValueError, "count"):
                admit(base | {"range_count": count})
        comparisons = {"start_to_document_start": 0, "end_to_document_end": 0,
                       "start_to_end": -1, "document_start_to_end": -1}
        unique = base | {"range_count": 1, "comparisons": comparisons}
        self.assertEqual(admit(unique)["comparisons"], comparisons)
        for malformed in (None, {}, comparisons | {"extra": 0},
                          comparisons | {"start_to_end": True},
                          comparisons | {"start_to_end": -0x80000001},
                          comparisons | {"start_to_end": 0x80000000}):
            with self.subTest(comparisons=malformed), self.assertRaisesRegex(ValueError, "UIA"):
                admit(unique | {"comparisons": malformed})
        with self.assertRaisesRegex(ValueError, "nonunique"):
            admit(base | {"comparisons": comparisons})


if __name__ == "__main__":
    unittest.main()
