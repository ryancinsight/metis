"""Canonical Windows ABI records, independent of host C long width.

Windows LONG/DWORD are 32-bit and WORD is 16-bit; ULONG_PTR is pointer-sized.
Natural field alignment derives the INPUT union width from its real arms.
Declarations: https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-input
Scalar widths: https://learn.microsoft.com/en-us/windows/win32/winprog/windows-data-types
"""
from __future__ import annotations
import ctypes


SEND_INPUT_MOUSE = 0


SEND_INPUT_KEYBOARD = 1


VK_CONTROL = 0x11


VK_RETURN = 0x0D


VK_MENU = 0x12


KEYEVENTF_KEYUP = 0x0002


KEYEVENTF_UNICODE = 0x0004


MOUSEEVENTF_MOVE = 0x0001


MOUSEEVENTF_LEFTDOWN = 0x0002


MOUSEEVENTF_LEFTUP = 0x0004


MOUSEEVENTF_WHEEL = 0x0800


MOUSEEVENTF_ABSOLUTE = 0x8000


MOUSEEVENTF_VIRTUALDESK = 0x4000


WHEEL_DELTA = 120


SM_XVIRTUALSCREEN = 76


SM_YVIRTUALSCREEN = 77


SM_CXVIRTUALSCREEN = 78


SM_CYVIRTUALSCREEN = 79


WM_NULL = 0x0000


SMTO_ABORT_IF_HUNG = 0x0002


SMTO_BLOCK = 0x0001


HWND_TOPMOST = ctypes.c_void_p(-1)


GA_ROOT = 3


SWP_NOSIZE = 0x0001


SWP_NOMOVE = 0x0002


SWP_NOACTIVATE = 0x0010


class KeyboardInput(ctypes.Structure):
    """KEYBDINPUT: WORD, WORD, DWORD, DWORD and ULONG_PTR."""
    _fields_ = [
        ("virtual_key", ctypes.c_uint16),
        ("scan_code", ctypes.c_uint16),
        ("flags", ctypes.c_uint32),
        ("time", ctypes.c_uint32),
        ("extra_info", ctypes.c_size_t),
    ]


class MouseInput(ctypes.Structure):
    """MOUSEINPUT: signed LONG coordinates and unsigned event metadata."""
    _fields_ = [
        ("dx", ctypes.c_int32),
        ("dy", ctypes.c_int32),
        ("mouse_data", ctypes.c_uint32),
        ("flags", ctypes.c_uint32),
        ("time", ctypes.c_uint32),
        ("extra_info", ctypes.c_size_t),
    ]


class HardwareInput(ctypes.Structure):
    """HARDWAREINPUT: one DWORD message and two WORD parameters."""
    _fields_ = [
        ("message", ctypes.c_uint32),
        ("parameter_low", ctypes.c_uint16),
        ("parameter_high", ctypes.c_uint16),
    ]


class Point(ctypes.Structure):
    """POINT: two signed 32-bit LONG coordinates."""
    _fields_ = [("x", ctypes.c_int32), ("y", ctypes.c_int32)]


class Rect(ctypes.Structure):
    """RECT: four signed 32-bit LONG edges."""
    _fields_ = [
        ("left", ctypes.c_int32),
        ("top", ctypes.c_int32),
        ("right", ctypes.c_int32),
        ("bottom", ctypes.c_int32),
    ]


class InputUnion(ctypes.Union):
    """INPUT payload, aligned and sized by its three Windows-defined arms."""
    _fields_ = [
        ("keyboard", KeyboardInput),
        ("mouse", MouseInput),
        ("hardware", HardwareInput),
    ]


class Input(ctypes.Structure):
    """INPUT: one DWORD tag followed by its naturally aligned union."""
    _anonymous_ = ("value",)
    _fields_ = [("kind", ctypes.c_uint32), ("value", InputUnion)]
