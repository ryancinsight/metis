"""Value-semantic contracts for the native capture utility."""

from __future__ import annotations

import hashlib
import pathlib
import struct
import sys
import tempfile
import unittest
import zlib
from unittest import mock

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
import python_native_capture as capture


def decode_png(path: pathlib.Path) -> tuple[int, int, bytes]:
    """Decode the bounded RGBA subset emitted by the capture utility."""
    content = path.read_bytes()
    if content[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("invalid PNG signature")
    offset = 8
    header = None
    compressed = bytearray()
    saw_end = False
    while offset < len(content):
        if offset + 12 > len(content):
            raise ValueError("truncated PNG chunk")
        size = struct.unpack_from(">I", content, offset)[0]
        end = offset + 12 + size
        if end > len(content):
            raise ValueError("truncated PNG payload")
        kind = content[offset + 4 : offset + 8]
        payload = content[offset + 8 : offset + 8 + size]
        checksum = struct.unpack_from(">I", content, offset + 8 + size)[0]
        if zlib.crc32(kind + payload) & 0xFFFFFFFF != checksum:
            raise ValueError("invalid PNG checksum")
        if kind == b"IHDR":
            header = payload
        elif kind == b"IDAT":
            compressed.extend(payload)
        elif kind == b"IEND":
            saw_end = True
            break
        offset = end
    if header is None or not saw_end or len(header) != 13:
        raise ValueError("incomplete PNG")
    width, height, depth, color_type, compression, filtering, interlace = struct.unpack(
        ">IIBBBBB", header
    )
    if (depth, color_type, compression, filtering, interlace) != (8, 6, 0, 0, 0):
        raise ValueError("unsupported PNG encoding")
    raw = zlib.decompress(bytes(compressed))
    row_bytes = width * 4
    expected = height * (row_bytes + 1)
    if len(raw) != expected:
        raise ValueError("unexpected PNG scanline length")
    pixels = bytearray()
    for row in range(height):
        start = row * (row_bytes + 1)
        if raw[start] != 0:
            raise ValueError("unexpected PNG filter")
        pixels.extend(raw[start + 1 : start + row_bytes + 1])
    return width, height, bytes(pixels)


def encoded_png(width: int, rows: list[tuple[int, bytes]]) -> bytes:
    """Build a small RGBA PNG with the supplied filtered scanlines."""
    signature = b"\x89PNG\r\n\x1a\n"
    header = struct.pack(">IIBBBBB", width, len(rows), 8, 6, 0, 0, 0)

    def chunk(kind: bytes, payload: bytes) -> bytes:
        return (
            struct.pack(">I", len(payload))
            + kind
            + payload
            + struct.pack(">I", zlib.crc32(kind + payload) & 0xFFFFFFFF)
        )

    scanlines = b"".join(bytes((filter_type,)) + payload for filter_type, payload in rows)
    return (
        signature
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(scanlines))
        + chunk(b"IEND", b"")
    )


class NativeCaptureTests(unittest.TestCase):
    def test_read_png_round_trip_preserves_rgba_and_source_digest(self) -> None:
        bounds = capture._WindowBounds(handle=0, width=2, height=2)
        source_bgra = bytes(
            (
                3,
                2,
                1,
                0,
                6,
                5,
                4,
                0,
                9,
                8,
                7,
                0,
                12,
                11,
                10,
                0,
            )
        )
        with tempfile.TemporaryDirectory() as temporary:
            path = pathlib.Path(temporary) / "source.png"
            capture._write_capture(path, bounds, source_bgra)
            frame = capture._read_png(path)
            self.assertEqual((frame.width, frame.height), (2, 2))
            self.assertEqual(
                frame.rgba,
                bytes(
                    (
                        1,
                        2,
                        3,
                        255,
                        4,
                        5,
                        6,
                        255,
                        7,
                        8,
                        9,
                        255,
                        10,
                        11,
                        12,
                        255,
                    )
                ),
            )
            self.assertEqual(frame.sha256, hashlib.sha256(path.read_bytes()).hexdigest())

    def test_read_png_unfilters_all_supported_scanline_forms(self) -> None:
        width = 2
        source_rows = [
            bytes((1, 2, 3, 255, 11, 12, 13, 255)),
            bytes((21, 22, 23, 255, 31, 32, 33, 255)),
            bytes((41, 42, 43, 255, 51, 52, 53, 255)),
            bytes((61, 62, 63, 255, 71, 72, 73, 255)),
            bytes((81, 82, 83, 255, 91, 92, 93, 255)),
        ]
        encoded_rows: list[tuple[int, bytes]] = []
        previous = bytes(width * 4)
        for filter_type, current in enumerate(source_rows):
            filtered = bytearray(len(current))
            for index, value in enumerate(current):
                left = current[index - 4] if index >= 4 else 0
                above = previous[index]
                upper_left = previous[index - 4] if index >= 4 else 0
                if filter_type == 0:
                    prediction = 0
                elif filter_type == 1:
                    prediction = left
                elif filter_type == 2:
                    prediction = above
                elif filter_type == 3:
                    prediction = (left + above) // 2
                else:
                    prediction = capture._paeth(left, above, upper_left)
                filtered[index] = (value - prediction) & 0xFF
            encoded_rows.append((filter_type, bytes(filtered)))
            previous = current

        with tempfile.TemporaryDirectory() as temporary:
            path = pathlib.Path(temporary) / "filtered.png"
            path.write_bytes(encoded_png(width, encoded_rows))
            frame = capture._read_png(path)
            self.assertEqual(frame.rgba, b"".join(source_rows))

    def test_read_png_rejects_corrupt_checksum(self) -> None:
        bounds = capture._WindowBounds(handle=0, width=1, height=1)
        with tempfile.TemporaryDirectory() as temporary:
            path = pathlib.Path(temporary) / "corrupt.png"
            capture._write_capture(path, bounds, bytes((3, 2, 1, 0)))
            content = bytearray(path.read_bytes())
            content[29] ^= 1
            path.write_bytes(content)
            with self.assertRaisesRegex(ValueError, "checksum"):
                capture._read_png(path)

    def test_png_round_trip_preserves_dimensions_and_channels(self) -> None:
        bounds = capture._WindowBounds(handle=0, width=2, height=2)
        source_bgra = bytes(
            (
                3,
                2,
                1,
                0,
                6,
                5,
                4,
                0,
                9,
                8,
                7,
                0,
                12,
                11,
                10,
                0,
            )
        )
        with tempfile.TemporaryDirectory() as temporary:
            path = pathlib.Path(temporary) / "capture.png"
            digest = capture._write_capture(path, bounds, source_bgra)
            self.assertEqual(
                decode_png(path),
                (
                    2,
                    2,
                    bytes(
                        (
                            1,
                            2,
                            3,
                            255,
                            4,
                            5,
                            6,
                            255,
                            7,
                            8,
                            9,
                            255,
                            10,
                            11,
                            12,
                            255,
                        )
                    ),
                ),
            )
            self.assertEqual(digest, hashlib.sha256(path.read_bytes()).hexdigest())

    def test_capture_rejects_wrong_storage_and_extension(self) -> None:
        bounds = capture._WindowBounds(handle=0, width=1, height=1)
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            with self.assertRaisesRegex(ValueError, "pixel storage"):
                capture._write_capture(root / "capture.png", bounds, b"\0" * 3)
            with self.assertRaisesRegex(ValueError, r"\.bmp or \.png"):
                capture._write_capture(root / "capture.gif", bounds, b"\0" * 4)

    def test_command_arguments_are_repeated_and_not_recombined(self) -> None:
        parsed = capture._parser().parse_args(
            [
                "--command",
                "metis-app.exe",
                "--argument=--metis-native-window",
                "--argument",
                "value with spaces",
                "--cwd",
                "work dir",
                "--output",
                "capture.png",
            ]
        )
        self.assertEqual(
            parsed.command_arguments,
            ["--metis-native-window", "value with spaces"],
        )
        self.assertEqual(parsed.cwd, pathlib.Path("work dir"))

    def test_command_only_options_are_rejected_for_wheel_capture(self) -> None:
        with mock.patch.object(
            capture.sys,
            "argv",
            [
                "python_native_capture.py",
                "--wheel",
                "application.whl",
                "--argument",
                "ignored",
                "--output",
                "capture.png",
            ],
        ), self.assertRaisesRegex(SystemExit, "require --command"):
            capture.main()

    def test_frame_option_is_parsed_without_synthetic_dimensions(self) -> None:
        parsed = capture._parser().parse_args(
            [
                "--wheel",
                "application.whl",
                "--frame",
                "real-frame.png",
                "--output",
                "capture.png",
            ]
        )
        self.assertEqual(parsed.frame, pathlib.Path("real-frame.png"))
        self.assertIsNone(parsed.width)
        self.assertIsNone(parsed.height)


if __name__ == "__main__":
    unittest.main()
