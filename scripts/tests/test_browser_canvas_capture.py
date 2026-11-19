"""Pixel oracles shared by browser canvas captures."""
import pathlib
import hashlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
from browser_canvas_capture import pixel_observation
from browser_protocol import BrowserRuntimeError


class RasterPixelTests(unittest.TestCase):
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
