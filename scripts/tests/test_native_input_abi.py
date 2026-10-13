"""Windows-declaration oracles for native record layouts and encoded values."""
from __future__ import annotations

import ctypes
import pathlib
import struct
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
from native_input.windows import abi, pointer


class WindowsInputAbiTests(unittest.TestCase):
    def test_native_pointer_width_selects_the_windows_record_layout(self) -> None:
        # Microsoft Windows uses LLP64: only the ULONG_PTR member varies.
        width = ctypes.sizeof(ctypes.c_void_p)
        self.assertEqual(ctypes.sizeof(ctypes.c_size_t), width)
        self.assertIn(width, (4, 8))
        if width == 8:
            keyboard = (24, 8, (0, 2, 4, 8, 16))
            mouse = (32, 8, (0, 4, 8, 12, 16, 24))
            union = (32, 8, (0, 0, 0))
            record = (40, 8, (0, 8))
        else:
            keyboard = (16, 4, (0, 2, 4, 8, 12))
            mouse = (24, 4, (0, 4, 8, 12, 16, 20))
            union = (24, 4, (0, 0, 0))
            record = (28, 4, (0, 4))
        for kind, (size, alignment, offsets) in (
            (abi.KeyboardInput, keyboard),
            (abi.MouseInput, mouse),
            (abi.HardwareInput, (8, 4, (0, 4, 6))),
            (abi.Point, (8, 4, (0, 4))),
            (abi.Rect, (16, 4, (0, 4, 8, 12))),
            (abi.InputUnion, union),
            (abi.Input, record),
        ):
            with self.subTest(record=kind.__name__):
                self.assertEqual(ctypes.sizeof(kind), size)
                self.assertEqual(ctypes.alignment(kind), alignment)
                self.assertEqual(
                    tuple(getattr(kind, name).offset for name, _ in kind._fields_),
                    offsets,
                )

    def test_mouse_record_preserves_signed_longs_unsigned_bits_and_pointer(self) -> None:
        width = ctypes.sizeof(ctypes.c_void_p)
        extra = (1 << (width * 8)) - 1
        mouse = abi.MouseInput(
            dx=-(1 << 31), dy=(1 << 31) - 1,
            mouse_data=0xFFFFFF88, flags=0xC001, time=0x01020304,
            extra_info=extra,
        )
        self.assertEqual((mouse.dx, mouse.dy), (-(1 << 31), (1 << 31) - 1))
        self.assertEqual(mouse.mouse_data, 0xFFFFFF88)
        self.assertEqual(mouse.extra_info, extra)
        encoding = "<iiIII4xQ" if width == 8 else "<iiIIII"
        expected = struct.pack(
            encoding, -(1 << 31), (1 << 31) - 1,
            0xFFFFFF88, 0xC001, 0x01020304, extra,
        )
        self.assertEqual(bytes(mouse), expected)
        record = abi.Input(kind=abi.SEND_INPUT_MOUSE)
        record.mouse = mouse
        tag = struct.pack("<I4x" if width == 8 else "<I", 0)
        self.assertEqual(bytes(record), tag + expected)

    def test_point_and_rect_encode_each_signed_coordinate_as_four_bytes(self) -> None:
        minimum, maximum = -(1 << 31), (1 << 31) - 1
        point = abi.Point(minimum, maximum)
        rectangle = abi.Rect(minimum, -1, maximum, 1)
        self.assertEqual(bytes(point), struct.pack("<ii", minimum, maximum))
        self.assertEqual(bytes(rectangle), struct.pack("<iiii", minimum, -1, maximum, 1))
        self.assertEqual((rectangle.left, rectangle.top, rectangle.right, rectangle.bottom),
                         (minimum, -1, maximum, 1))

    def test_hardware_arm_encodes_the_declared_message_and_word_parameters(self) -> None:
        hardware = abi.HardwareInput(0x01020304, 0x1122, 0x3344)
        self.assertEqual(bytes(hardware), struct.pack("<IHH", 0x01020304, 0x1122, 0x3344))
        union = abi.InputUnion(hardware=hardware)
        self.assertEqual(bytes(union)[:8], bytes(hardware))
        self.assertEqual(union.hardware.parameter_low, 0x1122)
        self.assertEqual(union.hardware.parameter_high, 0x3344)

    def test_absolute_move_uses_the_documented_virtual_desktop_flag(self) -> None:
        # 0x0100 means XUP; the virtual desktop selector is 0x4000.
        self.assertEqual(abi.MOUSEEVENTF_VIRTUALDESK, 0x4000)
        move, press, release = pointer.click_records(10926, 24400)
        self.assertEqual(move.mouse.flags, 0xC001)
        self.assertEqual((move.mouse.dx, move.mouse.dy), (10926, 24400))
        self.assertEqual((press.mouse.flags, press.mouse.dx, press.mouse.dy), (0x0002, 0, 0))
        self.assertEqual((release.mouse.flags, release.mouse.dx, release.mouse.dy), (0x0004, 0, 0))


if __name__ == "__main__":
    unittest.main()
