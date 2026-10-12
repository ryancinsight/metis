#!/usr/bin/env python3
"""Compile the starter's canonical vector mark into a macOS application icon.

Apple's iconset contract names point sizes and display scales separately.
Every bitmap renders from the SVG source; no smaller bitmap is enlarged.
Run on macOS: python3 scripts/apple_icon.py
"""

from __future__ import annotations

import argparse
from pathlib import Path
import struct
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "crates/metis-starter/frontend/assets/metis-mark.svg"
OUTPUT = ROOT / "crates/metis-starter/icons/application.icns"
POINT_SIZES = (16, 32, 128, 256, 512)
SCALES = (1, 2)


def representations():
    """Return Apple's ten filenames and their required pixel dimensions."""
    return tuple(
        (f"icon_{points}x{points}{'@2x' if scale == 2 else ''}.png", points * scale)
        for points in POINT_SIZES for scale in SCALES
    )


def generate(source: Path, output: Path) -> None:
    """Render all resolutions and replace the destination after compilation."""
    if source.suffix.lower() != ".svg" or not source.is_file():
        raise ValueError("the application icon source must be a local SVG file")
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="metis-icon-", dir=output.parent) as directory:
        staging = Path(directory)
        iconset = staging / "Application.iconset"
        iconset.mkdir()
        for name, pixels in representations():
            image = iconset / name
            subprocess.run(
                ["sips", "-s", "format", "png", "-z", str(pixels), str(pixels),
                 str(source), "--out", str(image)],
                check=True, stdout=subprocess.DEVNULL, timeout=30,
            )
            png = image.read_bytes()
            if png[:8] != b"\x89PNG\r\n\x1a\n" or struct.unpack(">II", png[16:24]) != (pixels, pixels):
                raise ValueError(f"renderer returned incorrect dimensions for {name}")
        compiled = staging / "application.icns"
        subprocess.run(["iconutil", "-c", "icns", str(iconset), "-o", str(compiled)],
                       check=True, timeout=30)
        icon = compiled.read_bytes()
        if icon[:4] != b"icns" or struct.unpack(">I", icon[4:8])[0] != len(icon):
            raise ValueError("compiler returned an invalid icon container")
        compiled.replace(output)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=SOURCE)
    parser.add_argument("--output", type=Path, default=OUTPUT)
    args = parser.parse_args()
    generate(args.source.resolve(strict=True), args.output.absolute())


if __name__ == "__main__":
    main()
