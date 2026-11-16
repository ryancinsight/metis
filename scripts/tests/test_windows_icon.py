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

    def test_generated_icon_validates_and_decodes(self):
        with tempfile.TemporaryDirectory() as scratch:
            output = pathlib.Path(scratch) / "metis-mark.ico"
            data = windows_icon.generate(windows_icon.SOURCE, output)
            self.assertEqual(data, output.read_bytes())
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
                windows_icon.SOURCE, pathlib.Path(scratch) / "out.ico", png
            )
            images = dict(windows_icon.validate_ico(data))
            self.assertEqual(png.read_bytes(), images[256])
            width, rgba = decode_png(png.read_bytes())
            self.assertEqual(width, 256)
            self.assertEqual(pixel(rgba, 256, 128, 40), (15, 118, 110, 255))

    def test_generation_is_deterministic(self):
        with tempfile.TemporaryDirectory() as scratch:
            first = windows_icon.generate(
                windows_icon.SOURCE, pathlib.Path(scratch) / "first.ico"
            )
            second = windows_icon.generate(
                windows_icon.SOURCE, pathlib.Path(scratch) / "second.ico"
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


if __name__ == "__main__":
    unittest.main()
