"""Lifecycle stability checks for the browser text-layout contract."""
from __future__ import annotations

import copy
import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

from browser_protocol import BrowserRuntimeError
from browser_text_geometry import capture_text_geometry
from browser_text_stability import validate_text_geometry_stability
from test_browser_text_geometry import StubClient, StubTrace, _measurement


class BrowserTextStabilityTests(unittest.TestCase):
    """Keep lifecycle remounts on one bounded layout contract."""

    def test_stable_contract_reports_observations_and_structure(self):
        trace = StubTrace()
        capture_text_geometry(StubClient(_measurement()), trace, "initial")
        capture_text_geometry(StubClient(_measurement()), trace, "remounted")
        result = validate_text_geometry_stability(trace.metrics["text_geometry"])
        self.assertEqual(result["observations"], 2)
        self.assertEqual(result["grapheme_clusters"], 27)
        self.assertEqual(result["line_count"], 2)
        self.assertEqual(result["direction"], "ltr")
        self.assertTrue(result["stable"])

    def test_contract_change_is_rejected(self):
        trace = StubTrace()
        capture_text_geometry(StubClient(_measurement()), trace, "initial")
        changed = _measurement()
        changed["visual_order"] = list(reversed(changed["visual_order"]))
        capture_text_geometry(StubClient(changed), trace, "remounted")
        with self.assertRaisesRegex(BrowserRuntimeError, "contract changed"):
            validate_text_geometry_stability(trace.metrics["text_geometry"])

    def test_availability_change_is_rejected(self):
        with self.assertRaisesRegex(BrowserRuntimeError, "availability changed"):
            validate_text_geometry_stability([
                {"available": False, "reason": "unsupported"},
                {"available": True},
            ])

    def test_unavailable_reason_is_stable(self):
        result = validate_text_geometry_stability([
            {"available": False, "reason": "Intl.Segmenter unavailable"},
            {"available": False, "reason": "Intl.Segmenter unavailable"},
        ])
        self.assertEqual(result, {
            "available": False,
            "observations": 2,
            "reason": "Intl.Segmenter unavailable",
        })

    def test_unavailable_reason_change_is_rejected(self):
        with self.assertRaisesRegex(BrowserRuntimeError, "unavailable reasons changed"):
            validate_text_geometry_stability([
                {"available": False, "reason": "first"},
                {"available": False, "reason": "second"},
            ])

    def test_observation_count_is_bounded(self):
        with self.assertRaisesRegex(BrowserRuntimeError, "between 2 and 8"):
            validate_text_geometry_stability([copy.deepcopy(_measurement())])


if __name__ == "__main__":
    unittest.main()
