"""Bounded BMP and PNG codecs for the admitted software capture grammar."""

import struct
import zlib

from . import contract


def decode_bitmap(content):
    """Read the renderer's exact 54-byte-header, bottom-up ARGB bitmap format."""
    contract.require(54 <= len(content) <= contract.MAX_BYTES, "Truncated or oversized bitmap")
    contract.require(content[:2] == b"BM", "Bitmap magic differs")
    size, reserved, offset = struct.unpack_from("<III", content, 2)
    dib, width, height, planes, bits, compression, image_size = struct.unpack_from("<IiiHHII", content, 14)
    contract.dimensions(width, height)
    contract.require((size, reserved, offset, dib, planes, bits, compression, image_size) ==
             (len(content), 0, 54, 40, 1, 32, 0, width * height * 4), "Unsupported bitmap header")
    contract.require(content[38:54] == bytes(16) and len(content) == 54 + image_size,
             "Bitmap header or pixel length differs")
    stride = width * 4
    pixels = b"".join(content[54 + row * stride:54 + (row + 1) * stride]
                      for row in reversed(range(height)))
    return width, height, pixels


_PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
# The capture writer's exact header: 8-bit RGBA, deflate, standard filters,
# no interlace.
_PNG_FORMAT = (8, 6, 0, 0, 0)


def decode_png(content):
    """Read only the writer's encoding: 8-bit RGBA, unfiltered rows, no ancillary chunks.

    Chunk lengths are bounded by the remaining input and every CRC is checked,
    so truncated or tampered captures fail before decompression, and the
    inflated stream may not exceed the exact size the header implies.
    """
    contract.require(len(_PNG_SIGNATURE) < len(content) <= contract.MAX_BYTES, "Empty or oversized PNG")
    contract.require(content.startswith(_PNG_SIGNATURE), "PNG signature differs")
    offset, chunks = len(_PNG_SIGNATURE), []
    while offset < len(content):
        contract.require(offset + 12 <= len(content), "Truncated PNG chunk")
        length, kind = struct.unpack_from(">I4s", content, offset)
        end = offset + 8 + length
        contract.require(end + 4 <= len(content), "PNG chunk exceeds the file")
        data = content[offset + 8:end]
        (crc,) = struct.unpack_from(">I", content, end)
        contract.require(zlib.crc32(kind + data) == crc, "PNG chunk CRC differs")
        chunks.append((kind, data))
        offset = end + 4
    kinds = [kind for kind, _ in chunks]
    contract.require(len(kinds) >= 3 and kinds[0] == b"IHDR" and kinds[-1] == b"IEND"
             and set(kinds[1:-1]) == {b"IDAT"} and not chunks[-1][1],
             "PNG chunks differ from the writer's IHDR, IDAT and IEND")
    header = chunks[0][1]
    contract.require(len(header) == 13, "PNG header length differs")
    width, height, *layout = struct.unpack(">IIBBBBB", header)
    contract.dimensions(width, height)
    contract.require(tuple(layout) == _PNG_FORMAT, "PNG is not 8-bit RGBA without interlace")
    stride = 1 + width * 4
    expected = stride * height
    stream = zlib.decompressobj()
    try:
        raw = stream.decompress(b"".join(data for _, data in chunks[1:-1]), expected + 1)
    except zlib.error as error:
        raise contract.VisualError("Malformed PNG image data") from error
    contract.require(len(raw) == expected and stream.eof and not stream.unused_data
             and not stream.unconsumed_tail, "PNG image data length differs")
    contract.require(all(raw[row * stride] == 0 for row in range(height)), "PNG rows are filtered")
    rgba = b"".join(raw[row * stride + 1:(row + 1) * stride] for row in range(height))
    pixels = bytearray(len(rgba))
    pixels[0::4], pixels[1::4], pixels[2::4], pixels[3::4] = rgba[2::4], rgba[1::4], rgba[0::4], rgba[3::4]
    return width, height, bytes(pixels)


def encode_png(image):
    """Write review difference masks in the capture writer's PNG encoding."""
    width, height, pixels = image
    contract.dimensions(width, height)
    contract.require(len(pixels) == width * height * 4, "Pixel length differs from image dimensions")
    rgba = bytearray(len(pixels))
    rgba[0::4], rgba[1::4], rgba[2::4], rgba[3::4] = pixels[2::4], pixels[1::4], pixels[0::4], pixels[3::4]
    stride = width * 4
    raw = b"".join(b"\x00" + bytes(rgba[row * stride:(row + 1) * stride]) for row in range(height))

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    content = (_PNG_SIGNATURE + chunk(b"IHDR", struct.pack(">II", width, height) + bytes(_PNG_FORMAT))
               + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))
    contract.require(len(content) <= contract.MAX_BYTES, "Difference PNG exceeds artifact budget")
    return content
