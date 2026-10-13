"""Bounded RGBA PNG decoding and opaque captured-BGRA encoding."""
from __future__ import annotations
import hashlib
import pathlib
import struct
import zlib
from native_capture import limits, model


PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def write_png(path: pathlib.Path, bounds: model.WindowBounds, pixels: bytes) -> str:
    """Write the captured top-down BGRA rows as an RGBA PNG."""
    expected = bounds.width * bounds.height * 4
    if len(pixels) != expected:
        raise ValueError("captured pixel storage does not match the window bounds")
    rows = bytearray()
    row_bytes = bounds.width * 4
    for offset in range(0, len(pixels), row_bytes):
        source = pixels[offset : offset + row_bytes]
        rows.append(0)
        for blue, green, red, _alpha in zip(
            source[0::4], source[1::4], source[2::4], source[3::4], strict=True
        ):
            rows.extend((red, green, blue, 255))
    signature = b"\x89PNG\r\n\x1a\n"
    header = struct.pack(">IIBBBBB", bounds.width, bounds.height, 8, 6, 0, 0, 0)

    def chunk(kind: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + kind
            + data
            + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
        )

    content = signature + chunk(b"IHDR", header)
    content += chunk(b"IDAT", zlib.compress(bytes(rows)))
    content += chunk(b"IEND", b"")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)
    return hashlib.sha256(content).hexdigest()


def paeth(left: int, above: int, upper_left: int) -> int:
    estimate = left + above - upper_left
    left_distance = abs(estimate - left)
    above_distance = abs(estimate - above)
    upper_left_distance = abs(estimate - upper_left)
    if left_distance <= above_distance and left_distance <= upper_left_distance:
        return left
    if above_distance <= upper_left_distance:
        return above
    return upper_left


def read_png(path: pathlib.Path) -> model.Frame:
    """Read a bounded, non-interlaced 8-bit RGBA PNG into row-major bytes.

    This is the interchange boundary for an already-decoded application frame;
    it never inspects DICOM or other medical-format data. The decoder accepts
    all PNG scanline filters so a RITK framebuffer capture can be handed to the
    Python host without a third-party image package.
    """
    path = path.resolve(strict=True)
    if not path.is_file():
        raise ValueError(f"frame is not a regular file: {path}")
    size = path.stat().st_size
    if size > limits.MAX_INPUT_BYTES:
        raise ValueError("frame PNG exceeds the bounded input size")
    content = path.read_bytes()
    if len(content) != size or len(content) < len(PNG_SIGNATURE) or content[:8] != PNG_SIGNATURE:
        raise ValueError("frame is not a complete PNG")

    offset = len(PNG_SIGNATURE)
    width = height = None
    compressed = bytearray()
    saw_idat = False
    saw_iend = False
    while offset < len(content):
        if len(content) - offset < 12:
            raise ValueError("frame PNG has a truncated chunk")
        length = struct.unpack_from(">I", content, offset)[0]
        end = offset + 12 + length
        if end > len(content):
            raise ValueError("frame PNG has a truncated payload")
        kind = content[offset + 4 : offset + 8]
        payload_start = offset + 8
        payload = content[payload_start : payload_start + length]
        checksum = struct.unpack_from(">I", content, payload_start + length)[0]
        if zlib.crc32(kind + payload) & 0xFFFFFFFF != checksum:
            raise ValueError("frame PNG has an invalid chunk checksum")
        if kind == b"IHDR":
            if offset != len(PNG_SIGNATURE) or width is not None or length != 13:
                raise ValueError("frame PNG has an invalid image header")
            width, height, depth, color_type, compression, filtering, interlace = struct.unpack(
                ">IIBBBBB", payload
            )
            if (
                width == 0
                or height == 0
                or width > limits.MAX_FRAME_DIMENSION
                or height > limits.MAX_FRAME_DIMENSION
                or width * height > limits.MAX_FRAME_PIXELS
                or (depth, color_type, compression, filtering, interlace) != (8, 6, 0, 0, 0)
            ):
                raise ValueError("frame PNG must be a bounded non-interlaced RGBA image")
        elif kind == b"IDAT":
            if width is None:
                raise ValueError("frame PNG data appears before its image header")
            saw_idat = True
            compressed.extend(payload)
        elif kind == b"IEND":
            if length != 0 or not saw_idat:
                raise ValueError("frame PNG is missing image data")
            saw_iend = True
            offset = end
            break
        offset = end

    if width is None or height is None or not saw_iend or offset != len(content):
        raise ValueError("frame PNG is missing its terminal chunk")
    row_bytes = width * 4
    raw_size = height * (row_bytes + 1)
    decoder = zlib.decompressobj()
    raw = decoder.decompress(bytes(compressed), raw_size + 1)
    if len(raw) > raw_size or decoder.unconsumed_tail or not decoder.eof:
        raise ValueError("frame PNG decompression exceeds the bounded image size")
    if decoder.unused_data:
        raise ValueError("frame PNG contains trailing compressed data")
    raw += decoder.flush(raw_size + 1 - len(raw))
    if len(raw) != raw_size:
        raise ValueError("frame PNG scanlines do not match the image dimensions")

    pixels = bytearray(width * height * 4)
    previous = bytearray(row_bytes)
    raw_offset = 0
    output_offset = 0
    for _row in range(height):
        filter_type = raw[raw_offset]
        scanline = raw[raw_offset + 1 : raw_offset + 1 + row_bytes]
        reconstructed = bytearray(row_bytes)
        if filter_type == 0:
            reconstructed[:] = scanline
        elif filter_type == 1:
            for index, value in enumerate(scanline):
                left = reconstructed[index - 4] if index >= 4 else 0
                reconstructed[index] = (value + left) & 0xFF
        elif filter_type == 2:
            for index, value in enumerate(scanline):
                reconstructed[index] = (value + previous[index]) & 0xFF
        elif filter_type == 3:
            for index, value in enumerate(scanline):
                left = reconstructed[index - 4] if index >= 4 else 0
                reconstructed[index] = (value + ((left + previous[index]) // 2)) & 0xFF
        elif filter_type == 4:
            for index, value in enumerate(scanline):
                left = reconstructed[index - 4] if index >= 4 else 0
                above = previous[index]
                upper_left = previous[index - 4] if index >= 4 else 0
                reconstructed[index] = (value + paeth(left, above, upper_left)) & 0xFF
        else:
            raise ValueError("frame PNG uses an unsupported scanline filter")
        pixels[output_offset : output_offset + row_bytes] = reconstructed
        previous = reconstructed
        raw_offset += row_bytes + 1
        output_offset += row_bytes
    return model.Frame(width, height, bytes(pixels), hashlib.sha256(content).hexdigest())
