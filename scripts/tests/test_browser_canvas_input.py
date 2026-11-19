"""Value oracles for the rendered raster input fixture."""
from __future__ import annotations

import pathlib
import hashlib
import os
import sys
import unittest
import tempfile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
from browser_canvas_capture import pixel_observation
from browser_canvas_input import run
from browser_protocol import ROOT, BrowserRuntimeError
from browser_trace import BrowserEngine


class RasterPixelTests(unittest.TestCase):
    def test_linked_assets_are_rejected_before_browser_requests(self):
        with tempfile.TemporaryDirectory() as temporary:
            link = pathlib.Path(temporary) / "browser-assets"
            try:
                link.symlink_to(ROOT / "output" / "browser", target_is_directory=True)
            except OSError as error:
                if os.name == "nt" and error.winerror == 1314:
                    self.skipTest(f"host cannot create directory symlinks: {error}")
                raise
            with self.assertRaisesRegex(BrowserRuntimeError, "path is linked"):
                run("http://127.0.0.1:0", BrowserEngine.CHROMIUM, link, 1.0, 1000, "1" * 40)

    def frame(self):
        return {"ok": True, "width": 100, "height": 80,
                "pixels": [17, 34, 51, 255] * 8_000}

    def test_every_visible_pixel_matches_the_literal_frame(self):
        frame = self.frame()
        result = pixel_observation(frame, (100, 80), bytes((17, 34, 51, 255)) * 8_000)
        self.assertEqual(result, {
            "width": 100, "height": 80,
            "rgba_sha256": hashlib.sha256(bytes((17, 34, 51, 255)) * 8_000).hexdigest(),
        })

    def test_last_pixel_and_blank_frames_fail(self):
        for pixels in ([17, 34, 51, 255] * 7_999 + [17, 34, 50, 255], [0] * 32_000):
            frame = self.frame()
            frame["pixels"] = pixels
            with self.assertRaisesRegex(BrowserRuntimeError, r"pixel \("):
                pixel_observation(frame, (100, 80), bytes((17, 34, 51, 255)) * 8_000)

    def test_wrong_dimensions_fail_even_with_the_expected_pixel_count(self):
        frame = self.frame()
        frame["width"], frame["height"] = 80, 100
        with self.assertRaisesRegex(BrowserRuntimeError, "dimensions differ"):
            pixel_observation(frame, (100, 80), bytes((17, 34, 51, 255)) * 8_000)

    def test_decode_failure_and_invalid_buffers_fail(self):
        frames = [{"ok": False, "error": "decode rejected"}, self.frame(), self.frame()]
        frames[1]["pixels"].pop()
        frames[2]["pixels"][0] = 17.0
        for frame in frames:
            with self.subTest(frame_kind=frame.get("error", "pixels")):
                with self.assertRaises(BrowserRuntimeError):
                    pixel_observation(frame, (100, 80), bytes((17, 34, 51, 255)) * 8_000)
