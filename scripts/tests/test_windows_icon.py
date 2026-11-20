"""Check the Windows application icon and the generator behind it."""

from __future__ import annotations

import pathlib
import struct
import sys
import tempfile
import unittest
import zlib


ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "scripts"
sys.path.insert(0, str(SCRIPTS))

import windows_icon


def decode_png(png: bytes) -> tuple[int, bytes]:
    """Decode a filter-0-only RGBA PNG the generator writes."""
    assert png[:8] == bytes.fromhex("89504e470d0a1a0a")
    assert png[12:16] == b"IHDR"
    width, height = struct.unpack(">II", png[16:24])
    assert struct.unpack(">BBBBB", png[24:29]) == (8, 6, 0, 0, 0)
    position = 8
    raw = bytearray()
    while position < len(png):
        (length,) = struct.unpack(">I", png[position : position + 4])
        tag = png[position + 4 : position + 8]
        body = png[position + 8 : position + 8 + length]
        assert (
            zlib.crc32(tag + body) & 0xFFFFFFFF
            == struct.unpack(">I", png[position + 8 + length : position + 12 + length])[0]
        )
        if tag == b"IDAT":
            raw += zlib.decompress(body)
        if tag == b"IEND":
            assert position + 12 + length == len(png)
            break
        position += 12 + length
    stride = width * 4
    pixels = bytearray()
    for row in range(height):
        assert raw[row * (stride + 1)] == 0
        pixels += raw[row * (stride + 1) + 1 : (row + 1) * (stride + 1)]
    return width, bytes(pixels)


def pixel(pixels: bytes, edge: int, x: int, y: int) -> tuple[int, int, int, int]:
    offset = (y * edge + x) * 4
    return tuple(pixels[offset : offset + 4])  # type: ignore[return-value]


class WindowsIconTests(unittest.TestCase):
    """The ICO packs seven validated PNGs rendered from the SVG mark."""

    def test_sizes_cover_the_shell_slots(self):
        self.assertEqual(windows_icon.representations(), (16, 24, 32, 48, 64, 128, 256))

    def test_shipped_icon_parses_like_the_cli_expects(self):
        data = (ROOT / "examples" / "browser" / "assets" / "metis-mark.ico").read_bytes()
        images = windows_icon.validate_ico(data)
        self.assertEqual([edge for edge, _ in images], [16, 24, 32, 48, 64, 128, 256])

    def test_render_marks_the_expected_pixels(self):
        svg = (ROOT / "examples" / "browser" / "assets" / "metis-mark.svg").read_text(
            encoding="utf-8"
        )
        rgba = windows_icon.render_mark(svg, 256)
        self.assertEqual(pixel(rgba, 256, 0, 0), (0, 0, 0, 0))
        self.assertEqual(pixel(rgba, 256, 128, 40), (15, 118, 110, 255))
        self.assertEqual(pixel(rgba, 256, 60, 170), (67, 56, 202, 255))
        blended = pixel(rgba, 256, 128, 160)
        self.assertEqual(blended[3], 255)
        self.assertNotIn(blended[:3], ((15, 118, 110), (67, 56, 202)))

    def test_render_bounds_keep_fractional_and_clipped_shape_edges(self):
        fractional = windows_icon.render_mark(
            '<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"'
            ' viewBox="0 0 4 4"><path d="M.25 .25H3.75V3.75H.25Z"'
            ' fill="#f00"/></svg>',
            4,
        )
        self.assertEqual(
            [pixel(fractional, 4, x, y) for y in range(4) for x in range(4)],
            [(255, 0, 0, 255)] * 16,
        )
        clipped = windows_icon.render_mark(
            '<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"'
            ' viewBox="0 0 4 4"><path d="M-1-1H1.1V1.1H-1Z"'
            ' fill="#0f0"/></svg>',
            4,
        )
        self.assertEqual(pixel(clipped, 4, 0, 0), (0, 255, 0, 255))
        self.assertEqual(
            [
                pixel(clipped, 4, x, y)
                for y in range(4)
                for x in range(4)
                if (x, y) != (0, 0)
            ],
            [(0, 0, 0, 0)] * 15,
        )
        far_offscreen = windows_icon.render_mark(
            '<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"'
            ' viewBox="0 0 4 4"><path d="M1e308 1e308H1.1e308V1.1e308H1e308Z"'
            ' fill="#00f"/></svg>',
            16,
        )
        self.assertEqual(far_offscreen, bytes(16 * 16 * 4))
        rounded_edge = windows_icon.render_mark(
            '<svg xmlns="http://www.w3.org/2000/svg" width="273" height="273"'
            ' viewBox="0 0 273 273"><path d="M0 0H300V300H0Z"'
            ' fill="#f00"/></svg>',
            24,
        )
        self.assertEqual(len(rounded_edge), 24 * 24 * 4)
        self.assertEqual(
            [pixel(rounded_edge, 24, x, y) for y in range(24) for x in range(24)],
            [(255, 0, 0, 255)] * (24 * 24),
        )

    def test_open_fill_subpath_matches_explicit_close(self):
        prefix = (
            '<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"'
            ' viewBox="0 0 4 4"><path d="'
        )
        suffix = '" fill="#f00"/></svg>'
        implicit = windows_icon.render_mark(prefix + "M0 0H4V4H0" + suffix, 4)
        explicit = windows_icon.render_mark(prefix + "M0 0H4V4H0Z" + suffix, 4)
        self.assertEqual(implicit, explicit)
        same = windows_icon.render_mark(prefix + "M0 0H4V4H0ZM1 1H3V3H1Z" + suffix, 4)
        hole = windows_icon.render_mark(prefix + "M0 0H4V4H0ZM1 1V3H3V1Z" + suffix, 4)
        self.assertEqual(pixel(same, 4, 1, 1), (255, 0, 0, 255))
        self.assertEqual(pixel(hole, 4, 1, 1), (0, 0, 0, 0))
        for continuation in (
            "L4 0V4H0",
            "H4V4H0",
            "M0 0H4V4H0",
            "ZZH4V4H0",
        ):
            with self.subTest(continuation=continuation):
                after_close = prefix + "M0 0H2V2H0Z" + continuation + suffix
                self.assertEqual(windows_icon.render_mark(after_close, 4), explicit)
        vertical = windows_icon.render_mark(
            prefix + "M0 0H2V2H0ZV4H4V0" + suffix, 4
        )
        self.assertEqual(pixel(vertical, 4, 0, 0), (0, 0, 0, 0))
        self.assertEqual(pixel(vertical, 4, 2, 0), (255, 0, 0, 255))
        self.assertEqual(
            windows_icon.render_mark(prefix + "M0 0H4V4H0ZM2 2Z" + suffix, 4),
            explicit,
        )

    def test_pixel_bounds_stay_within_canvas(self):
        for viewport, pixels in ((1, 1), (3, 2), (273, 24), (4095, 257)):
            with self.subTest(viewport=viewport, pixels=pixels):
                scale = pixels / viewport
                beyond_edge = float(viewport + 1)
                columns, rows = windows_icon._pixel_bounds(
                    [[(0.0, 0.0), (beyond_edge, beyond_edge)]], scale, pixels
                )
                self.assertEqual(columns, range(pixels))
                self.assertEqual(rows, range(pixels))

    def test_generated_icon_validates_and_decodes(self):
        with tempfile.TemporaryDirectory() as scratch:
            output = pathlib.Path(scratch) / "metis-mark.ico"
            data = windows_icon.generate(windows_icon.SOURCE, output)
            self.assertEqual(data, output.read_bytes())
            self.assertEqual(
                data,
                (ROOT / "examples" / "browser" / "assets" / "metis-mark.ico").read_bytes(),
            )
            images = windows_icon.validate_ico(data)
            self.assertEqual([edge for edge, _ in images], [16, 24, 32, 48, 64, 128, 256])
            width, rgba = decode_png(images[-1][1])
            self.assertEqual(width, 256)
            self.assertEqual(pixel(rgba, 256, 128, 40), (15, 118, 110, 255))

    def test_downsample_averages_blocks_half_up(self):
        rgba = bytes(
            [0, 0, 0, 0] * 12 + [100, 100, 100, 100] * 3 + [255, 255, 255, 255]
        )
        self.assertEqual(
            windows_icon.downsample(rgba, 4, 2),
            bytes([0, 0, 0, 0] * 2 + [50, 50, 50, 50] + [89, 89, 89, 89]),
        )
        half = bytes([0, 0, 0, 0] * 3 + [2, 2, 2, 2])
        self.assertEqual(
            windows_icon.downsample(half, 2, 2), bytes([1, 1, 1, 1])
        )

    def test_png_alternate_matches_the_largest_entry(self):
        with tempfile.TemporaryDirectory() as scratch:
            png = pathlib.Path(scratch) / "metis-mark.png"
            data = windows_icon.generate(
                windows_icon.SOURCE, pathlib.Path(scratch) / "out.ico", png,
                sizes=(256,),
            )
            images = dict(windows_icon.validate_ico(data))
            self.assertEqual(png.read_bytes(), images[256])
            width, rgba = decode_png(png.read_bytes())
            self.assertEqual(width, 256)
            self.assertEqual(pixel(rgba, 256, 128, 40), (15, 118, 110, 255))

    def test_generation_is_deterministic(self):
        # One slot proves the pipeline shape is deterministic; the full
        # seven-slot generation runs once in the validation test above, and
        # the visual-tests stage budgets the whole scripts suite at a minute.
        with tempfile.TemporaryDirectory() as scratch:
            first = windows_icon.generate(
                windows_icon.SOURCE, pathlib.Path(scratch) / "first.ico",
                sizes=(16,),
            )
            second = windows_icon.generate(
                windows_icon.SOURCE, pathlib.Path(scratch) / "second.ico",
                sizes=(16,),
            )
            self.assertEqual(first, second)

    def test_curves_and_strokes_fail_loudly(self):
        with self.assertRaises(ValueError):
            windows_icon.render_mark(
                '<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256"'
                ' viewBox="0 0 256 256"><path d="M0 0 C10 10 20 20 30 30"/></svg>',
                16,
            )
        with self.assertRaises(ValueError):
            windows_icon.render_mark(
                '<svg xmlns="http://www.w3.org/2000/svg" width="256" height="128"'
                ' viewBox="0 0 256 128"><path d="M0 0H10V10Z"/></svg>',
                16,
            )
        with self.assertRaises(ValueError):
            windows_icon.render_mark(
                '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"'
                ' viewBox="0 0 16 16"><path d="Z"/></svg>',
                16,
            )
        with self.assertRaises(ValueError):
            windows_icon.render_mark(
                '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"'
                ' viewBox="0 0 16 16"><path d="M0 0H1e999V1Z"/></svg>',
                16,
            )


if __name__ == "__main__":
    unittest.main()
