"""The canonical macOS icon and Apple's point/scale representation law."""
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import apple_icon


class AppleIconTests(unittest.TestCase):
    def test_representation_names_encode_point_times_scale(self):
        self.assertEqual(apple_icon.representations(), (
            ("icon_16x16.png", 16), ("icon_16x16@2x.png", 32),
            ("icon_32x32.png", 32), ("icon_32x32@2x.png", 64),
            ("icon_128x128.png", 128), ("icon_128x128@2x.png", 256),
            ("icon_256x256.png", 256), ("icon_256x256@2x.png", 512),
            ("icon_512x512.png", 512), ("icon_512x512@2x.png", 1024),
        ))

    def test_compiled_icon_contains_every_representation(self):
        icon = apple_icon.OUTPUT.read_bytes()
        self.assertEqual(icon[:4], b"icns")
        self.assertEqual(struct.unpack(">I", icon[4:8])[0], len(icon))
        images = {}
        offset = 8
        while offset < len(icon):
            kind = icon[offset:offset + 4]
            size = struct.unpack(">I", icon[offset + 4:offset + 8])[0]
            self.assertGreaterEqual(size, 8)
            self.assertLessEqual(offset + size, len(icon))
            payload = icon[offset + 8:offset + size]
            if payload[:8] == b"\x89PNG\r\n\x1a\n":
                images[kind] = struct.unpack(">II", payload[16:24])
            elif kind in {b"ic04", b"ic05"}:
                self.assertEqual(payload[:4], b"ARGB")
                edge = 16 if kind == b"ic04" else 32
                images[kind] = (edge, edge)
            offset += size
        self.assertEqual(offset, len(icon))
        self.assertEqual(images, {
            b"ic04": (16, 16), b"ic11": (32, 32), b"ic05": (32, 32),
            b"ic12": (64, 64), b"ic07": (128, 128), b"ic13": (256, 256),
            b"ic08": (256, 256), b"ic14": (512, 512), b"ic09": (512, 512),
            b"ic10": (1024, 1024),
        })

    @unittest.skipUnless(sys.platform == "darwin", "Apple's native icon decoder")
    def test_native_roundtrip_has_exact_dimensions_and_mark_pixels(self):
        with tempfile.TemporaryDirectory(prefix="metis-icon-test-") as directory:
            iconset = Path(directory) / "Decoded.iconset"
            subprocess.run(["iconutil", "-c", "iconset", "-o", str(iconset),
                            str(apple_icon.OUTPUT)], check=True, timeout=30)
            self.assertEqual({p.name for p in iconset.glob("*.png")},
                             {name for name, _ in apple_icon.representations()})
            for name, pixels in apple_icon.representations():
                png = (iconset / name).read_bytes()
                self.assertEqual(struct.unpack(">II", png[16:24]), (pixels, pixels))
            # Interior points derive from the SVG's two solid path fills.
            # Transparent corners distinguish the mark from a constant bitmap.
            for pixels in (256, 1024):
                name = "icon_256x256.png" if pixels == 256 else "icon_512x512@2x.png"
                bitmap = Path(directory) / "Decoded.bmp"
                subprocess.run(["sips", "-s", "format", "bmp", str(iconset / name),
                                "--out", str(bitmap)], check=True, timeout=30,
                               stdout=subprocess.DEVNULL)
                raw = bitmap.read_bytes()
                self.assertEqual(raw[:2], b"BM")
                width, height = struct.unpack("<ii", raw[18:26])
                self.assertEqual((width, abs(height)), (pixels, pixels))
                self.assertEqual(struct.unpack("<HI", raw[28:34]), (32, 3))
                self.assertEqual(struct.unpack("<IIII", raw[54:70]),
                                 (0xFF0000, 0xFF00, 0xFF, 0xFF000000))
                offset = struct.unpack("<I", raw[10:14])[0]
                for x, y, rgba in [(0, 0, (0, 0, 0, 0)),
                                   (128, 40, (15, 118, 110, 255)),
                                   (60, 170, (67, 56, 202, 255))]:
                    x, y = x * pixels // 256, y * pixels // 256
                    row = y if height < 0 else height - 1 - y
                    index = offset + (row * width + x) * 4
                    blue, green, red, alpha = raw[index:index + 4]
                    self.assertEqual((red, green, blue, alpha), rgba)


if __name__ == "__main__":
    unittest.main()
