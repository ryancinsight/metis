"""Compile the starter's canonical vector mark into a Windows application icon.

The ICO packs PNG-compressed entries at every required resolution; every
bitmap renders from the SVG source, no smaller bitmap is enlarged. The
renderer covers exactly the path grammar the shipped mark uses (straight
segments, solid fills, source-over compositing); anything else fails with a
message naming the unsupported construct instead of silently distorting the
mark. Run on any host: python3 scripts/windows_icon.py
"""

from __future__ import annotations

import argparse
import math
import pathlib
import re
import struct
import sys
import tempfile
import zlib
import xml.etree.ElementTree as ET

ROOT = pathlib.Path(__file__).resolve().parents[1]
SOURCE = ROOT / "examples" / "browser" / "assets" / "metis-mark.svg"
OUTPUT = ROOT / "examples" / "browser" / "assets" / "metis-mark.ico"

# ICO entry edge lengths; seven entries like the shipped icon.
SIZES = (16, 24, 32, 48, 64, 128, 256)

PNG_MAGIC = bytes.fromhex("89504e470d0a1a0a")
ICO_HEADER = struct.Struct("<HHH")
ICO_ENTRY = struct.Struct("<BBBBHHII")

_COMMAND = re.compile(r"[MmLlHhVvZz]")
_NUMBER = re.compile(r"-?(?:\d+\.?\d*|\.\d+)(?:[eE]-?\d+)?")
_PAIR = re.compile(
    r"\s*,?\s*(" + _NUMBER.pattern + r")\s*,?\s*(" + _NUMBER.pattern + r")"
)


def representations() -> tuple[int, ...]:
    """ICO entry edge lengths in pixels."""
    return SIZES


def _parse_color(value: str) -> tuple[int, int, int]:
    text = value.strip().lower()
    if text.startswith("#"):
        digits = text[1:]
        if len(digits) == 3:
            digits = "".join(char * 2 for char in digits)
        if len(digits) == 6 and re.fullmatch(r"[0-9a-f]{6}", digits):
            return (
                int(digits[0:2], 16),
                int(digits[2:4], 16),
                int(digits[4:6], 16),
            )
    raise ValueError(f"unsupported fill color {value!r}: use #rgb or #rrggbb")


def _commands(data: str) -> list[tuple[str, list[float]]]:
    """Split path data into (command, numeric parameters) items."""
    items: list[tuple[str, list[float]]] = []
    position = 0
    command: str | None = None
    while position < len(data):
        match = _COMMAND.match(data, position)
        if match is not None:
            command = match.group(0)
            position = match.end()
            if command in "Zz":
                items.append((command, []))
                command = None
            continue
        if command is None:
            if data[position].isspace() or data[position] == ",":
                position += 1
                continue
            raise ValueError(f"unsupported path data near {data[position:]!r}")
        if command in "MmLl":
            pair = _PAIR.match(data, position)
            if pair is None:
                raise ValueError(f"malformed coordinates near {data[position:]!r}")
            items.append((command, [float(pair.group(1)), float(pair.group(2))]))
            position = pair.end()
            if command == "M":
                command = "L"
            elif command == "m":
                command = "l"
            continue
        number = _NUMBER.match(data, position)
        if number is None:
            raise ValueError(f"malformed coordinate near {data[position:]!r}")
        items.append((command, [float(number.group(0))]))
        position = number.end()
    return items


def _trace(data: str) -> list[list[tuple[float, float]]]:
    """Trace path data into closed subpath polygons of absolute points."""
    subpaths: list[list[tuple[float, float]]] = []
    current: list[tuple[float, float]] = []
    point = (0.0, 0.0)
    start = (0.0, 0.0)
    for command, parameters in _commands(data):
        absolute = command.isupper()
        if command in "Mm":
            if current:
                subpaths.append(current)
            x = parameters[0] if absolute else point[0] + parameters[0]
            y = parameters[1] if absolute else point[1] + parameters[1]
            point = (x, y)
            start = point
            current = [point]
        elif command in "Ll":
            x = parameters[0] if absolute else point[0] + parameters[0]
            y = parameters[1] if absolute else point[1] + parameters[1]
            point = (x, y)
            current.append(point)
        elif command in "Hh":
            x = parameters[0] if absolute else point[0] + parameters[0]
            point = (x, point[1])
            current.append(point)
        elif command in "Vv":
            y = parameters[0] if absolute else point[1] + parameters[0]
            point = (point[0], y)
            current.append(point)
        elif command in "Zz":
            if current:
                point = start
                current.append(point)
                subpaths.append(current)
                current = []
        else:  # pragma: no cover - the tokenizer admits only the above.
            raise ValueError(f"unsupported path command {command!r}")
    if current:
        subpaths.append(current)
    return subpaths


def _shapes(svg: str) -> tuple[int, list[tuple[
    tuple[int, int, int, int], list[list[tuple[float, float]]]
]]]:
    """Parse the admitted vector subset into composited fill polygons."""
    try:
        root = ET.fromstring(svg)
    except ET.ParseError as error:
        raise ValueError(f"the source is not well-formed XML: {error}") from error
    width = root.get("width")
    height = root.get("height")
    view_box = root.get("viewBox")
    if width is None or height is None or view_box is None:
        raise ValueError("the source needs width, height and viewBox attributes")
    try:
        box = [float(part) for part in view_box.split()]
    except ValueError as error:
        raise ValueError(f"malformed viewBox {view_box!r}") from error
    if (
        len(box) != 4
        or box[0] != 0
        or box[1] != 0
        or box[2] != float(width)
        or box[3] != float(height)
    ):
        raise ValueError("the viewport must start at the origin and match width/height")
    if box[2] != box[3]:
        raise ValueError("icons need a square viewport; refusing to distort the mark")
    if box[2] <= 0 or box[2] > 4096:
        raise ValueError("refusing a viewport outside 1..=4096")
    shapes: list[tuple[tuple[int, int, int, int], list[list[tuple[float, float]]]]] = []
    for path in root.iter():
        if path is root:
            continue
        if path.tag != "{http://www.w3.org/2000/svg}path" and path.tag != "path":
            raise ValueError(f"unsupported element <{path.tag.split('}')[-1]}>")
        if len(path) != 0:
            raise ValueError("nested drawing content is not supported")
        for name in path.attrib:
            local = name.split("}")[-1]
            if local not in ("d", "fill", "fill-opacity", "id", "class"):
                raise ValueError(f"unsupported attribute {local!r}")
        if path.get("stroke") is not None:
            raise ValueError("stroked paths are not supported; flatten strokes to fills")
        if (path.get("fill-rule") or "nonzero") != "nonzero":
            raise ValueError("only the nonzero fill rule is supported")
        data = path.get("d")
        if data is None:
            raise ValueError("a path without geometry is not supported")
        color = _parse_color(path.get("fill", "#000000"))
        try:
            opacity = float(path.get("fill-opacity", "1"))
        except ValueError as error:
            raise ValueError(f"malformed fill-opacity: {error}") from error
        if not 0.0 <= opacity <= 1.0:
            raise ValueError("fill-opacity must sit in 0..=1")
        alpha = min(255, int(opacity * 255 + 0.5))
        shapes.append(((color[0], color[1], color[2], alpha), _trace(data)))
    if not shapes:
        raise ValueError("the source draws no paths")
    return int(box[2]), shapes


def _winding(point: tuple[float, float], polygon: list[tuple[float, float]]) -> int:
    """Count how often the polygon winds around the point (nonzero rule)."""
    total = 0
    for (x1, y1), (x2, y2) in zip(polygon, polygon[1:] + polygon[:1]):
        if (y1 <= point[1] < y2) or (y2 <= point[1] < y1):
            crossing = x1 + (point[1] - y1) / (y2 - y1) * (x2 - x1)
            if crossing > point[0]:
                total += 1 if y2 > y1 else -1
    return total


def _pixel_bounds(
    subpaths: list[list[tuple[float, float]]], scale: float, pixels: int
) -> tuple[range, range]:
    """Bound pixel centers that can lie inside one vector shape.

    A polygon and its interior lie within the extrema of its vertices. A pixel
    center is `(index + 0.5) / scale`, so floor at the lower vertex and ceil at
    the upper vertex conservatively include every center that can be inside.
    """
    points = [point for polygon in subpaths for point in polygon]
    if not points:
        raise ValueError("a path must draw at least one polygon point")
    if not all(
        math.isfinite(coordinate) for point in points for coordinate in point
    ):
        raise ValueError("path coordinates must be finite")
    source_edge = pixels / scale
    min_x = min(
        pixels,
        math.floor(min(source_edge, max(0.0, min(x for x, _ in points))) * scale),
    )
    max_x = min(
        pixels,
        math.ceil(min(source_edge, max(0.0, max(x for x, _ in points))) * scale),
    )
    min_y = min(
        pixels,
        math.floor(min(source_edge, max(0.0, min(y for _, y in points))) * scale),
    )
    max_y = min(
        pixels,
        math.ceil(min(source_edge, max(0.0, max(y for _, y in points))) * scale),
    )
    return range(min_x, max_x), range(min_y, max_y)


def render_mark(svg: str, pixels: int) -> bytes:
    """Render raw RGBA; pixel centers sample source-ordered shapes."""
    viewport, shapes = _shapes(svg)
    scale = pixels / viewport
    canvas = bytearray(pixels * pixels * 4)
    for (fr, fg, fb, fa), subpaths in shapes:
        if fa == 0:
            continue
        columns, rows = _pixel_bounds(subpaths, scale, pixels)
        for y in rows:
            for x in columns:
                point = ((x + 0.5) / scale, (y + 0.5) / scale)
                if not any(_winding(point, polygon) != 0 for polygon in subpaths):
                    continue
                offset = (y * pixels + x) * 4
                if fa == 255:
                    canvas[offset : offset + 4] = bytes((fr, fg, fb, 255))
                else:
                    red, green, blue, alpha = canvas[offset : offset + 4]
                    canvas[offset] = (fr * fa + red * (255 - fa)) // 255
                    canvas[offset + 1] = (fg * fa + green * (255 - fa)) // 255
                    canvas[offset + 2] = (fb * fa + blue * (255 - fa)) // 255
                    canvas[offset + 3] = fa + alpha * (255 - fa) // 255
    return bytes(canvas)


def _chunk(tag: bytes, payload: bytes) -> bytes:
    return struct.pack(">I", len(payload)) + tag + payload + struct.pack(
        ">I", zlib.crc32(tag + payload) & 0xFFFFFFFF
    )


def encode_png(rgba: bytes, pixels: int) -> bytes:
    """Encode square RGBA bytes as a minimal PNG (no filtering, level 9)."""
    header = struct.pack(">IIBBBBB", pixels, pixels, 8, 6, 0, 0, 0)
    rows = b"".join(
        b"\x00" + rgba[row * pixels * 4 : (row + 1) * pixels * 4]
        for row in range(pixels)
    )
    return (
        PNG_MAGIC
        + _chunk(b"IHDR", header)
        + _chunk(b"IDAT", zlib.compress(rows, 9))
        + _chunk(b"IEND", b"")
    )


def pack_ico(images: list[tuple[int, bytes]]) -> bytes:
    """Pack (edge, PNG) pairs into a PNG-compressed ICO container."""
    header = ICO_HEADER.pack(0, 1, len(images))
    offset = ICO_HEADER.size + ICO_ENTRY.size * len(images)
    directory = bytearray()
    payloads = bytearray()
    for edge, png in images:
        if not 1 <= edge <= 256:
            raise ValueError(f"ICO entries need edges in 1..=256, got {edge}")
        directory += ICO_ENTRY.pack(
            0 if edge == 256 else edge, 0 if edge == 256 else edge,
            0, 0, 1, 32, len(png), offset,
        )
        payloads += png
        offset += len(png)
    return bytes(header + directory + payloads)


def validate_ico(data: bytes) -> list[tuple[int, bytes]]:
    """Check the ICO container with the same rules the CLI enforces."""
    if len(data) < ICO_HEADER.size:
        raise ValueError("the icon is shorter than its header")
    reserved, icon_type, count = ICO_HEADER.unpack_from(data, 0)
    if reserved != 0 or icon_type != 1 or not 1 <= count <= 16:
        raise ValueError("not a 1..=16 entry icon directory")
    table_end = ICO_HEADER.size + ICO_ENTRY.size * count
    if len(data) < table_end:
        raise ValueError("the icon is shorter than its entry table")
    spans: list[tuple[int, int]] = []
    images: list[tuple[int, bytes]] = []
    for index in range(count):
        width, height, _, _, _, _, size, offset = ICO_ENTRY.unpack_from(
            data, ICO_HEADER.size + ICO_ENTRY.size * index
        )
        width = 256 if width == 0 else width
        height = 256 if height == 0 else height
        if width != height or not 1 <= width <= 256:
            raise ValueError(f"entry {index} is not a 1..=256 square")
        if offset < table_end or offset + size > len(data):
            raise ValueError(f"entry {index} points outside the icon")
        spans.append((offset, offset + size))
        payload = data[offset : offset + size]
        if payload[:8] != PNG_MAGIC:
            raise ValueError(f"entry {index} is not PNG-compressed")
        if payload[12:16] != b"IHDR" or len(payload) < 33:
            raise ValueError(f"entry {index} has no PNG header")
        png_width, png_height = struct.unpack(">II", payload[16:24])
        if png_width != width or png_height != height:
            raise ValueError(f"entry {index} PNG dimensions disagree with the ICO entry")
        position = 8
        seen_iend = False
        while position < len(payload):
            if position + 8 > len(payload):
                raise ValueError(f"entry {index} ends mid-chunk")
            length = struct.unpack(">I", payload[position : position + 4])[0]
            tag = payload[position + 4 : position + 8]
            end = position + 8 + length + 4
            if end > len(payload):
                raise ValueError(f"entry {index} chunk {tag!r} overruns its PNG")
            body = payload[position + 8 : position + 8 + length]
            (expected,) = struct.unpack(">I", payload[end - 4 : end])
            if zlib.crc32(tag + body) & 0xFFFFFFFF != expected:
                raise ValueError(f"entry {index} chunk {tag!r} fails its CRC")
            if tag == b"IEND":
                if seen_iend or end != len(payload):
                    raise ValueError(f"entry {index} carries data past IEND")
                seen_iend = True
            position = end
        if not seen_iend:
            raise ValueError(f"entry {index} PNG has no IEND")
        images.append((width, payload))
    spans.sort()
    for (first_start, first_end), (second_start, _) in zip(spans, spans[1:]):
        if second_start < first_end or second_start < first_start:
            raise ValueError("icon entries overlap")
    return images


def downsample(rgba: bytes, pixels: int, factor: int) -> bytes:
    """Average factor x factor blocks with half-up rounding (box filter)."""
    edge = pixels // factor
    out = bytearray(edge * edge * 4)
    for y in range(edge):
        for x in range(edge):
            accumulator = [0, 0, 0, 0]
            for dy in range(factor):
                row = (y * factor + dy) * pixels * 4
                for dx in range(factor):
                    offset = row + (x * factor + dx) * 4
                    for channel in range(4):
                        accumulator[channel] += rgba[offset + channel]
            area = factor * factor
            offset = (y * edge + x) * 4
            out[offset : offset + 4] = bytes(
                min(255, (total + area // 2) // area) for total in accumulator
            )
    return bytes(out)


def render_png(svg: str, pixels: int) -> bytes:
    """Render the source mark at one size with fourfold supersampling."""
    oversampled = render_mark(svg, pixels * 4)
    return encode_png(downsample(oversampled, pixels * 4, 4), pixels)


def generate(
    source: pathlib.Path,
    output: pathlib.Path,
    png: pathlib.Path | None = None,
    sizes: tuple[int, ...] | None = None,
) -> bytes:
    """Render the source mark at every size and pack the validated ICO.

    `sizes` overrides the shipped slot list for checks that only need the
    pipeline shape, not every slot; production always passes the default.
    """
    images = []
    svg = source.read_text(encoding="utf-8")
    for size in SIZES if sizes is None else sizes:
        images.append((size, render_png(svg, size)))
    data = pack_ico(images)
    validate_ico(data)
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        dir=str(output.parent), delete=False
    ) as staging:
        staging.write(data)
    pathlib.Path(staging.name).replace(output)
    if png is not None:
        png.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.NamedTemporaryFile(dir=str(png.parent), delete=False) as staging:
            staging.write(images[-1][1])
        pathlib.Path(staging.name).replace(png)
    return data


def main(argv: list[str] | None = None) -> int:
    """Compile the canonical mark into the shipped application icon."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source", type=pathlib.Path, default=SOURCE,
        help="local SVG mark to render",
    )
    parser.add_argument(
        "--output", type=pathlib.Path, default=OUTPUT,
        help="ICO file to replace atomically",
    )
    parser.add_argument(
        "--png", type=pathlib.Path, default=None,
        help="also write the 256-pixel PNG alternate to this path",
    )
    arguments = parser.parse_args(argv)
    if not arguments.source.is_file():
        print(f"icon source {arguments.source} is not a local file", file=sys.stderr)
        return 2
    try:
        data = generate(arguments.source, arguments.output, arguments.png)
    except (OSError, ValueError, ET.ParseError) as error:
        print(f"windows_icon: {error}", file=sys.stderr)
        return 1
    print(f"wrote {len(data)} bytes to {arguments.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
