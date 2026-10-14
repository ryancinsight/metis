"""Validated Unicode units and Win32 keyboard input delivery."""
from __future__ import annotations
import ctypes
import struct
from collections.abc import Iterator
from contextlib import contextmanager
from dataclasses import dataclass
from enum import Enum
from native_input.windows import abi


MAX_TEXT_BYTES = 128


class Recipient(Enum):
    """Explicit mapping domain; activation has no focused-window prerequisite."""
    FOCUSED = "focused"
    FOREGROUND_QUEUE = "foreground_queue"


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


def _key_byte(value: int, name: str) -> None:
    if type(value) is not int:
        raise TypeError(f"{name} must be an integer, excluding bool")
    if not 1 <= value <= 0xFF:
        raise ValueError(f"{name} is outside the nonzero Win32 byte range: {value}")


@dataclass(frozen=True)
class PhysicalKey:
    """One admitted MAPVK_VK_TO_VSC_EX mapping, reused for down and up.

    The admitted domain is a nonzero scan byte with no prefix or E0. E1
    requires a different multi-record protocol and is rejected. SCANCODE
    makes wVk irrelevant; keeping it zero prevents a second key identity.
    https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-keybdinput
    https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-mapvirtualkeyexw
    """
    virtual_key: int
    scan_code: int
    extended: bool

    def __post_init__(self) -> None:
        _key_byte(self.virtual_key, "virtual key")
        _key_byte(self.scan_code, "scan code")
        if type(self.extended) is not bool:
            raise TypeError("extended must be bool")

    @classmethod
    def from_mapping(cls, virtual_key: int, mapped: int) -> PhysicalKey:
        _key_byte(virtual_key, "virtual key")
        if type(mapped) is not int:
            raise TypeError("mapped scan code must be an integer, excluding bool")
        if not 0 <= mapped <= 0xFFFF:
            raise ValueError(f"mapped scan code exceeds the two-byte domain: {mapped}")
        prefix, scan = mapped >> 8, mapped & 0xFF
        if prefix not in (0, 0xE0) or scan == 0:
            raise ValueError(f"unsupported physical key mapping: {mapped:#06x}")
        return cls(virtual_key, scan, prefix == 0xE0)

    def record(self, *, key_up: bool) -> abi.Input:
        if type(key_up) is not bool:
            raise TypeError("key_up must be bool")
        flags = 0x0008 | (0x0001 if self.extended else 0)
        if key_up:
            flags |= abi.KEYEVENTF_KEYUP
        record = abi.Input(kind=abi.SEND_INPUT_KEYBOARD)
        record.keyboard = abi.KeyboardInput(
            virtual_key=0, scan_code=self.scan_code, flags=flags, time=0, extra_info=0,
        )
        return record

    def send(self, *, key_up: bool) -> None:
        record = self.record(key_up=key_up)
        user32 = ctypes.WinDLL("user32", use_last_error=True)
        user32.SendInput.argtypes = [ctypes.c_uint32, ctypes.POINTER(abi.Input), ctypes.c_int32]
        user32.SendInput.restype = ctypes.c_uint32
        sent = user32.SendInput(1, ctypes.byref(record), ctypes.sizeof(abi.Input))
        if sent != 1:
            error = ctypes.WinError(ctypes.get_last_error())
            error.add_note(f"SendInput physical key={self.virtual_key:#04x}, scan={self.scan_code:#04x}, "
                           f"extended={self.extended}, key_up={key_up}, sent={sent}/1")
            raise error

    @contextmanager
    def held(self) -> Iterator[None]:
        """Release a successfully submitted down exactly once, preserving failure.

        This scope owns the physical key lifetime. A rejected down never enters
        the body. A failed up propagates unless the body already failed, in
        which case the original exception remains primary with release context.
        No failed delivery is retried or treated as successful release.
        """
        self.send(key_up=False)
        primary = None
        try:
            yield
        except BaseException as error:
            primary = error
            raise
        finally:
            try:
                self.send(key_up=True)
            except BaseException as error:
                if primary is None:
                    raise
                primary.add_note(f"physical key release failed for {self}: {type(error).__name__}: {error}")


def _recipient_layout(user32, recipient: Recipient) -> int:
    """Read the selected actual foreground ownership domain's HKL once.

    FOCUSED requires a real focused HWND and uses its owning thread. Explicit
    FOREGROUND_QUEUE uses the real foreground thread for system activation:
    acquiring application focus cannot itself require application focus.
    These observations are non-atomic; neither domain substitutes the injector
    thread or falls back to another domain when an observation fails.
    https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getguithreadinfo
    https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getkeyboardlayout
    """
    user32.GetForegroundWindow.argtypes, user32.GetForegroundWindow.restype = [], ctypes.c_void_p
    user32.GetWindowThreadProcessId.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint32)]
    user32.GetWindowThreadProcessId.restype = ctypes.c_uint32
    user32.GetGUIThreadInfo.argtypes = [ctypes.c_uint32, ctypes.POINTER(abi.GuiThreadInfo)]
    user32.GetGUIThreadInfo.restype = ctypes.c_int32
    user32.GetKeyboardLayout.argtypes, user32.GetKeyboardLayout.restype = [ctypes.c_uint32], ctypes.c_void_p
    foreground = user32.GetForegroundWindow()
    if not foreground:
        raise RuntimeError("GetForegroundWindow has no physical key recipient")
    foreground_thread = user32.GetWindowThreadProcessId(foreground, None)
    if not foreground_thread:
        raise ctypes.WinError(ctypes.get_last_error())
    recipient_thread = foreground_thread
    if recipient is Recipient.FOCUSED:
        info = abi.GuiThreadInfo(byte_size=ctypes.sizeof(abi.GuiThreadInfo))
        if not user32.GetGUIThreadInfo(foreground_thread, ctypes.byref(info)):
            raise ctypes.WinError(ctypes.get_last_error())
        if not info.focus:
            raise RuntimeError("GetGUIThreadInfo reports no focused physical key recipient")
        recipient_thread = user32.GetWindowThreadProcessId(info.focus, None)
        if not recipient_thread:
            raise ctypes.WinError(ctypes.get_last_error())
    layout = user32.GetKeyboardLayout(recipient_thread)
    if not layout:
        raise RuntimeError(f"GetKeyboardLayout reports no layout for recipient thread {recipient_thread}")
    return int(layout)


def physical_keys(virtual_keys: tuple[int, ...], *, recipient: Recipient = Recipient.FOCUSED) -> tuple[PhysicalKey, ...]:
    """Map one entire chord against one observed recipient HKL before delivery.

    Each distinct key is mapped once; repeated keys reuse the admitted record.
    No mapping failure can deliver a prefix of the requested chord.
    """
    if not virtual_keys:
        raise ValueError("physical key sequence must not be empty")
    if not isinstance(recipient, Recipient):
        raise TypeError("physical key recipient must be a Recipient")
    for key in virtual_keys:
        _key_byte(key, "virtual key")
    user32 = ctypes.WinDLL("user32", use_last_error=True)
    layout = _recipient_layout(user32, recipient)
    user32.MapVirtualKeyExW.argtypes = [ctypes.c_uint32, ctypes.c_uint32, ctypes.c_void_p]
    user32.MapVirtualKeyExW.restype = ctypes.c_uint32
    mapped = {key: PhysicalKey.from_mapping(key, user32.MapVirtualKeyExW(key, 4, layout))
              for key in dict.fromkeys(virtual_keys)}
    return tuple(mapped[key] for key in virtual_keys)


def send_shortcut(shortcut: str) -> None:
    if shortcut == "enter":
        (enter,) = physical_keys((abi.VK_RETURN,))
        with enter.held():
            pass
        return
    if shortcut == "control-enter":
        control, enter = physical_keys((abi.VK_CONTROL, abi.VK_RETURN))
        with control.held(), enter.held():
            pass
        return
    raise ValueError(f"unsupported native shortcut: {shortcut!r}")
