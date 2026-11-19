"""Path admission and physical screenshot extents for browser raster input."""
import pathlib
import os
import sys
import unittest
import tempfile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
from browser_canvas_input import _screenshot_dimensions, run
from browser_canvas_capture import pixel_observation
from browser_protocol import ROOT, BrowserRuntimeError
from browser_trace import BrowserEngine


class CanvasInputPathTests(unittest.TestCase):
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


class CanvasInputScreenshotTests(unittest.TestCase):
    def test_physical_extent_follows_display_scale(self):
        for scale, expected in ((1000, (100, 80)), (2000, (200, 160)),
                                (1250, (125, 100))):
            with self.subTest(scale=scale):
                self.assertEqual(_screenshot_dimensions(scale), expected)

    def test_fractional_edge_is_rejected_without_resampling(self):
        with self.assertRaisesRegex(BrowserRuntimeError, "fractional physical pixel"):
            _screenshot_dimensions(1333)

    def test_last_physical_pixel_at_double_scale_is_checked(self):
        frame = {"ok": True, "width": 200, "height": 160,
                 "pixels": [17, 34, 51, 255] * 31_999 + [17, 34, 50, 255]}
        with self.assertRaisesRegex(BrowserRuntimeError, r"pixel \(199, 159\)"):
            pixel_observation(frame, (200, 160), bytes((17, 34, 51, 255)) * 32_000)
