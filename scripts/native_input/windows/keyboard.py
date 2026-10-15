"""Validated Unicode units and Win32 keyboard input delivery."""
from __future__ import annotations
import ctypes
import struct
from native_input.windows import abi


MAX_TEXT_BYTES = 128


def validate_text(text: str) -> tuple[bytes, tuple[int, ...]]:
    encoded = text.encode("utf-8")
    if not text:
        raise ValueError("--text must not be empty")
    if len(encoded) > MAX_TEXT_BYTES:
        raise ValueError(f"--text exceeds the {MAX_TEXT_BYTES}-byte bound")
    if any(ord(character) < 0x20 or ord(character) == 0x7F for character in text):
        raise ValueError("--text contains a control character")
    utf16 = text.encode("utf-16-le", "strict")
    units = struct.unpack(f"<{len(utf16) // 2}H", utf16)
    return encoded, units


def send_unicode_text(text_units: tuple[int, ...]) -> None:
    user32 = ctypes.windll.user32
    user32.SendInput.argtypes = [
        ctypes.c_uint,
        ctypes.POINTER(abi.Input),
        ctypes.c_int,
    ]
    user32.SendInput.restype = ctypes.c_uint
    inputs = (abi.Input * (2 * len(text_units)))()
    for index, unit in enumerate(text_units):
        inputs[2 * index].kind = abi.SEND_INPUT_KEYBOARD
        inputs[2 * index].keyboard = abi.KeyboardInput(
            virtual_key=0,
            scan_code=unit,
            flags=abi.KEYEVENTF_UNICODE,
            time=0,
            extra_info=0,
        )
        inputs[2 * index + 1].kind = abi.SEND_INPUT_KEYBOARD
        inputs[2 * index + 1].keyboard = abi.KeyboardInput(
            virtual_key=0,
            scan_code=unit,
            flags=abi.KEYEVENTF_UNICODE | abi.KEYEVENTF_KEYUP,
            time=0,
            extra_info=0,
        )
    sent = user32.SendInput(len(inputs), inputs, ctypes.sizeof(abi.Input))
    if sent != len(inputs):
        raise ctypes.WinError()


def send_virtual_key(virtual_key: int, *, key_up: bool) -> None:
    user32 = ctypes.windll.user32
    user32.SendInput.argtypes = [
        ctypes.c_uint,
        ctypes.POINTER(abi.Input),
        ctypes.c_int,
    ]
    user32.SendInput.restype = ctypes.c_uint
    record = abi.Input(kind=abi.SEND_INPUT_KEYBOARD)
    record.keyboard = abi.KeyboardInput(
        virtual_key=virtual_key,
        scan_code=0,
        flags=abi.KEYEVENTF_KEYUP if key_up else 0,
        time=0,
        extra_info=0,
    )
    sent = user32.SendInput(1, ctypes.byref(record), ctypes.sizeof(abi.Input))
    if sent != 1:
        raise ctypes.WinError()


def send_shortcut(shortcut: str) -> None:
    if shortcut == "enter":
        send_virtual_key(abi.VK_RETURN, key_up=False)
        send_virtual_key(abi.VK_RETURN, key_up=True)
        return
    if shortcut == "control-enter":
        send_virtual_key(abi.VK_CONTROL, key_up=False)
        try:
            send_virtual_key(abi.VK_RETURN, key_up=False)
            send_virtual_key(abi.VK_RETURN, key_up=True)
        finally:
            send_virtual_key(abi.VK_CONTROL, key_up=True)
        return
    if shortcut == "alt-shift-c":
        send_virtual_key(abi.VK_MENU, key_up=False)
        try:
            send_virtual_key(abi.VK_SHIFT, key_up=False)
            send_virtual_key(ord("C"), key_up=False)
            send_virtual_key(ord("C"), key_up=True)
        finally:
            send_virtual_key(abi.VK_SHIFT, key_up=True)
            send_virtual_key(abi.VK_MENU, key_up=True)
        return
    raise ValueError(f"unsupported native shortcut: {shortcut!r}")
