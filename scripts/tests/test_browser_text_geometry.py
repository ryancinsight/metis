"""Value-semantic tests for the browser text geometry probe."""
from __future__ import annotations

import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

from browser_protocol import BrowserRuntimeError
from browser_text_geometry import TEXT_GEOMETRY_SCRIPT, capture_text_geometry


FIXTURE_CLUSTERS = [
    "A\u030A",
    " ",
    "影",
    "像",
    " ",
    "—",
    " ",
    "ש",
    "ל",
    "ו",
    "ם",
    " ",
    "👩‍🔬",
    "\n",
    "l",
    "i",
    "n",
    "e",
    " ",
    "t",
    "w",
    "o",
    " ",
    "/",
    " ",
    "東",
    "京",
]
FIXTURE = "".join(FIXTURE_CLUSTERS)
BOUNDARIES = [0]
for cluster in FIXTURE_CLUSTERS:
    BOUNDARIES.append(BOUNDARIES[-1] + len(cluster.encode("utf-16-le")) // 2)


def _rect(left: float, top: float, width: float = 8.0, height: float = 24.0) -> dict:
    return {"left": left, "top": top, "width": width, "height": height}


def _measurement() -> dict:
    visual_clusters = [
        {
            "start": start,
            "end": end,
            "line_index": 0 if index < 14 else 1,
            "left": float(index * 8),
            "right": float(index * 8 + 8),
            "top": 16.0 if index < 14 else 40.0,
            "bottom": 40.0 if index < 14 else 64.0,
        }
        for index, (start, end) in enumerate(zip(BOUNDARIES, BOUNDARIES[1:]))
    ]
    return {
        "available": True,
        "source": "Range.getClientRects",
        "fixture": FIXTURE,
        "utf16_length": BOUNDARIES[-1],
        "grapheme_boundaries": BOUNDARIES,
        "cluster_rects": [
            {"start": start, "end": end, "fragments": [_rect(float(index * 8), 16.0 if index < 14 else 40.0)]}
            for index, (start, end) in enumerate(zip(BOUNDARIES, BOUNDARIES[1:]))
        ],
        "visual_clusters": visual_clusters,
        "visual_order": BOUNDARIES[:-1],
        "line_rects": [_rect(0.0, 16.0, 320.0), _rect(0.0, 40.0, 200.0)],
        "line_tops": [16.0, 40.0],
        "line_count": 2,
        "element_rect": _rect(-100000.0, 0.0, 320.0, 48.0),
        "style": {
            "font_family": "system-ui",
            "font_size_px": 16.0,
            "line_height_px": 24.0,
            "direction": "ltr",
            "writing_mode": "horizontal-tb",
        },
        "textarea": {
            "value_length": 18,
            "selection_start": 0,
            "selection_end": 0,
            "client_width": 320,
            "client_height": 112,
            "scroll_height": 112,
        },
    }


class StubTrace:
    """Carry the metric collection used by the production trace."""

    def __init__(self) -> None:
        self.metrics = {}


class StubClient:
    """Return one controlled WebDriver result for validator tests."""

    def __init__(self, value: dict) -> None:
        self.value = value

    def execute(self, script: str):
        del script
        return self.value


class BrowserTextGeometryTests(unittest.TestCase):
    def test_script_uses_segmented_ranges_and_removes_probe(self):
        self.assertIn("Intl.Segmenter", TEXT_GEOMETRY_SCRIPT)
        self.assertIn("Range", TEXT_GEOMETRY_SCRIPT)
        self.assertIn("visual_order", TEXT_GEOMETRY_SCRIPT)
        self.assertIn("probe.remove()", TEXT_GEOMETRY_SCRIPT)

    def test_capture_records_finite_grapheme_and_line_geometry(self):
        trace = StubTrace()
        measurement = capture_text_geometry(StubClient(_measurement()), trace, "initial")
        self.assertTrue(measurement["available"])
        self.assertEqual(measurement["line_count"], 2)
        self.assertEqual(measurement["grapheme_boundaries"], BOUNDARIES)
        self.assertEqual(trace.metrics["text_geometry"][0]["label"], "initial")

    def test_capture_preserves_explicit_unavailable_reason(self):
        trace = StubTrace()
        measurement = capture_text_geometry(
            StubClient({"available": False, "reason": "Intl.Segmenter unavailable"}),
            trace,
            "unsupported",
        )
        self.assertEqual(measurement, {"label": "unsupported", "available": False, "reason": "Intl.Segmenter unavailable"})

    def test_capture_rejects_noncontiguous_boundaries(self):
        value = _measurement()
        value["grapheme_boundaries"] = [0, 0, *BOUNDARIES[2:]]
        with self.assertRaisesRegex(BrowserRuntimeError, "grapheme boundaries"):
            capture_text_geometry(StubClient(value), StubTrace(), "invalid")

    def test_capture_rejects_invalid_line_order_or_rect(self):
        value = _measurement()
        value["line_tops"] = [40.0, 16.0]
        with self.assertRaisesRegex(BrowserRuntimeError, "line tops"):
            capture_text_geometry(StubClient(value), StubTrace(), "invalid-lines")
        value = _measurement()
        value["cluster_rects"][0]["fragments"][0]["width"] = -1.0
        with self.assertRaisesRegex(BrowserRuntimeError, "width"):
            capture_text_geometry(StubClient(value), StubTrace(), "invalid-rect")

    def test_capture_rejects_selection_beyond_textarea_value(self):
        value = _measurement()
        value["textarea"]["selection_end"] = value["textarea"]["value_length"] + 1
        with self.assertRaisesRegex(BrowserRuntimeError, "textarea metrics"):
            capture_text_geometry(StubClient(value), StubTrace(), "invalid-selection")

    def test_capture_rejects_visual_order_duplicates(self):
        value = _measurement()
        value["visual_order"][-1] = value["visual_order"][0]
        with self.assertRaisesRegex(BrowserRuntimeError, "visual order"):
            capture_text_geometry(StubClient(value), StubTrace(), "invalid-order")

    def test_capture_rejects_visual_cluster_bounds(self):
        value = _measurement()
        value["visual_clusters"][0]["right"] = value["visual_clusters"][0]["left"] - 1.0
        with self.assertRaisesRegex(BrowserRuntimeError, "visual cluster bounds"):
            capture_text_geometry(StubClient(value), StubTrace(), "invalid-visual-bounds")


if __name__ == "__main__":
    unittest.main()
