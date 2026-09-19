"""Capture a production native window before and after Win32 Unicode input."""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
import pathlib
import struct
import subprocess
import sys
from typing import Any

import python_native_capture as capture


MAX_TEXT_BYTES = 128
SEND_INPUT_KEYBOARD = 1
VK_CONTROL = 0x11
VK_RETURN = 0x0D
KEYEVENTF_KEYUP = 0x0002
KEYEVENTF_UNICODE = 0x0004
WM_NULL = 0x0000
SMTO_ABORT_IF_HUNG = 0x0002
SMTO_BLOCK = 0x0001
INPUT_FLUSH_TIMEOUT_MILLISECONDS = 1_000


class _KeyboardInput(ctypes.Structure):
    _fields_ = [
        ("virtual_key", ctypes.c_ushort),
        ("scan_code", ctypes.c_ushort),
        ("flags", ctypes.c_uint32),
        ("time", ctypes.c_uint32),
        ("extra_info", ctypes.c_size_t),
    ]


class _InputUnion(ctypes.Union):
    # INPUT is a tagged union whose MOUSEINPUT arm is wider than KEYBDINPUT
    # on 64-bit Windows. Retain that ABI width even though this runner sends
    # keyboard records only.
    _fields_ = [
        ("keyboard", _KeyboardInput),
        ("abi_padding", ctypes.c_ubyte * 32),
    ]


class _Input(ctypes.Structure):
    _anonymous_ = ("value",)
    _fields_ = [("kind", ctypes.c_uint32), ("value", _InputUnion)]


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Capture a production Windows window before and after Unicode input."
    )
    parser.add_argument("--command", type=pathlib.Path, required=True)
    parser.add_argument(
        "--argument",
        action="append",
        default=[],
        dest="command_arguments",
        help="one argument passed to --command; repeat for each argument",
    )
    parser.add_argument("--cwd", type=pathlib.Path)
    parser.add_argument(
        "--text",
        required=True,
        help="bounded Unicode text delivered through Win32 SendInput",
    )
    parser.add_argument(
        "--shortcut",
        choices=("enter", "control-enter"),
        help="optional submit shortcut delivered after the Unicode text",
    )
    parser.add_argument("--initial-output", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--manifest", type=pathlib.Path, required=True)
    return parser


def _validate_text(text: str) -> tuple[bytes, tuple[int, ...]]:
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


def _validate_paths(
    command: pathlib.Path,
    initial_output: pathlib.Path,
    output: pathlib.Path,
    manifest: pathlib.Path,
) -> pathlib.Path:
    executable = command.resolve(strict=True)
    if not executable.is_file():
        raise ValueError("--command must name a file")
    resolved = [initial_output.resolve(), output.resolve(), manifest.resolve()]
    if len(set(resolved)) != len(resolved):
        raise ValueError("capture outputs must be distinct")
    if initial_output.resolve() == output.resolve():
        raise ValueError("--initial-output must differ from --output")
    if initial_output.suffix.lower() not in {".bmp", ".png"}:
        raise ValueError("--initial-output must end in .bmp or .png")
    if output.suffix.lower() not in {".bmp", ".png"}:
        raise ValueError("--output must end in .bmp or .png")
    if manifest.suffix.lower() != ".json":
        raise ValueError("--manifest must end in .json")
    return executable


def _focus_window(handle: int) -> tuple[int, bool]:
    user32 = ctypes.windll.user32
    kernel32 = ctypes.windll.kernel32
    user32.GetWindowThreadProcessId.argtypes = [
        ctypes.c_void_p,
        ctypes.POINTER(ctypes.c_uint32),
    ]
    user32.GetWindowThreadProcessId.restype = ctypes.c_uint32
    user32.AttachThreadInput.argtypes = [ctypes.c_uint32, ctypes.c_uint32, ctypes.c_bool]
    user32.AttachThreadInput.restype = ctypes.c_bool
    user32.SetForegroundWindow.argtypes = [ctypes.c_void_p]
    user32.SetForegroundWindow.restype = ctypes.c_bool
    user32.SetFocus.argtypes = [ctypes.c_void_p]
    user32.SetFocus.restype = ctypes.c_void_p
    user32.GetFocus.restype = ctypes.c_void_p
    kernel32.GetCurrentThreadId.restype = ctypes.c_uint32
    user32.GetKeyboardLayout.argtypes = [ctypes.c_uint32]
    user32.GetKeyboardLayout.restype = ctypes.c_void_p

    process_id = ctypes.c_uint32()
    target_thread = user32.GetWindowThreadProcessId(handle, ctypes.byref(process_id))
    if target_thread == 0:
        raise ctypes.WinError()
    current_thread = kernel32.GetCurrentThreadId()
    attached = current_thread != target_thread
    if attached and not user32.AttachThreadInput(current_thread, target_thread, True):
        raise ctypes.WinError()
    try:
        foreground_set = bool(user32.SetForegroundWindow(handle))
        user32.SetFocus(handle)
        if int(user32.GetFocus() or 0) != handle:
            raise RuntimeError(
                "the native window could not receive keyboard focus "
                f"(foreground_set={foreground_set})"
            )
    finally:
        if attached and not user32.AttachThreadInput(current_thread, target_thread, False):
            raise ctypes.WinError()
    return int(user32.GetKeyboardLayout(target_thread)), foreground_set


def _send_unicode_text(text_units: tuple[int, ...]) -> None:
    user32 = ctypes.windll.user32
    user32.SendInput.argtypes = [
        ctypes.c_uint,
        ctypes.POINTER(_Input),
        ctypes.c_int,
    ]
    user32.SendInput.restype = ctypes.c_uint
    inputs = (_Input * (2 * len(text_units)))()
    for index, unit in enumerate(text_units):
        inputs[2 * index].kind = SEND_INPUT_KEYBOARD
        inputs[2 * index].keyboard = _KeyboardInput(
            virtual_key=0,
            scan_code=unit,
            flags=KEYEVENTF_UNICODE,
            time=0,
            extra_info=0,
        )
        inputs[2 * index + 1].kind = SEND_INPUT_KEYBOARD
        inputs[2 * index + 1].keyboard = _KeyboardInput(
            virtual_key=0,
            scan_code=unit,
            flags=KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
            time=0,
            extra_info=0,
        )
    sent = user32.SendInput(len(inputs), inputs, ctypes.sizeof(_Input))
    if sent != len(inputs):
        raise ctypes.WinError()


def _send_virtual_key(virtual_key: int, *, key_up: bool) -> None:
    user32 = ctypes.windll.user32
    user32.SendInput.argtypes = [
        ctypes.c_uint,
        ctypes.POINTER(_Input),
        ctypes.c_int,
    ]
    user32.SendInput.restype = ctypes.c_uint
    record = _Input(kind=SEND_INPUT_KEYBOARD)
    record.keyboard = _KeyboardInput(
        virtual_key=virtual_key,
        scan_code=0,
        flags=KEYEVENTF_KEYUP if key_up else 0,
        time=0,
        extra_info=0,
    )
    sent = user32.SendInput(1, ctypes.byref(record), ctypes.sizeof(_Input))
    if sent != 1:
        raise ctypes.WinError()


def _send_shortcut(shortcut: str) -> None:
    if shortcut == "enter":
        _send_virtual_key(VK_RETURN, key_up=False)
        _send_virtual_key(VK_RETURN, key_up=True)
        return
    if shortcut == "control-enter":
        _send_virtual_key(VK_CONTROL, key_up=False)
        try:
            _send_virtual_key(VK_RETURN, key_up=False)
            _send_virtual_key(VK_RETURN, key_up=True)
        finally:
            _send_virtual_key(VK_CONTROL, key_up=True)
        return
    raise ValueError(f"unsupported native shortcut: {shortcut!r}")


def _flush_window(handle: int) -> None:
    user32 = ctypes.windll.user32
    user32.SendMessageTimeoutW.argtypes = [
        ctypes.c_void_p,
        ctypes.c_uint,
        ctypes.c_size_t,
        ctypes.c_ssize_t,
        ctypes.c_uint,
        ctypes.c_uint,
        ctypes.POINTER(ctypes.c_size_t),
    ]
    user32.SendMessageTimeoutW.restype = ctypes.c_size_t
    result = ctypes.c_size_t()
    if not user32.SendMessageTimeoutW(
        handle,
        WM_NULL,
        0,
        0,
        SMTO_ABORT_IF_HUNG | SMTO_BLOCK,
        INPUT_FLUSH_TIMEOUT_MILLISECONDS,
        ctypes.byref(result),
    ):
        raise ctypes.WinError()


def _record(
    observation: Any,
    image: pathlib.Path,
    digest: str,
) -> dict[str, Any]:
    return {
        "window": {
            "outer": [observation.bounds.width, observation.bounds.height],
            "client": [observation.client_width, observation.client_height],
            "dpi": observation.dpi,
        },
        "image": image.as_posix(),
        "sha256": digest,
    }


def _capture_input(
    command: pathlib.Path,
    command_arguments: list[str],
    cwd: pathlib.Path | None,
    text: str,
    shortcut: str | None,
    initial_output: pathlib.Path,
    output: pathlib.Path,
    manifest: pathlib.Path,
) -> dict[str, Any]:
    if sys.platform != "win32":
        raise RuntimeError("native Unicode capture requires Windows")
    encoded, units = _validate_text(text)
    executable = _validate_paths(command, initial_output, output, manifest)
    if cwd is not None:
        cwd = cwd.resolve(strict=True)
        if not cwd.is_dir():
            raise NotADirectoryError(cwd)
    process = subprocess.Popen(
        [str(executable), *command_arguments],
        cwd=cwd,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    try:
        bounds = capture._wait_for_process_window(process)
        initial = capture._window_observation(bounds.handle)
        initial_pixels = capture._capture_window(initial.bounds)
        layout, foreground_set = _focus_window(bounds.handle)
        if not foreground_set:
            raise RuntimeError(
                "Windows denied foreground activation; no native input claim can be recorded"
            )
        initial_digest = capture._write_capture(
            initial_output, initial.bounds, initial_pixels
        )
        _send_unicode_text(units)
        if shortcut is not None:
            _send_shortcut(shortcut)
        _flush_window(bounds.handle)
        after = capture._window_observation(bounds.handle)
        after_pixels = capture._capture_window(after.bounds)
        after_digest = capture._write_capture(output, after.bounds, after_pixels)
        capture._close_window(bounds.handle)
        return_code = process.wait(timeout=capture.PROCESS_EXIT_TIMEOUT_SECONDS)
        if return_code != 0:
            raise RuntimeError(f"native process exited with status {return_code}")
        result: dict[str, Any] = {
            "process_returncode": return_code,
            "input": {
                "mechanism": "Win32 SendInput KEYEVENTF_UNICODE",
                "utf8_bytes": len(encoded),
                "utf16_units": len(units),
                "text_sha256": hashlib.sha256(encoded).hexdigest(),
                "target_keyboard_layout": f"0x{layout:016x}",
                "foreground_window_set": foreground_set,
                "shortcut": shortcut,
            },
            "initial": _record(initial, initial_output, initial_digest),
            "after": _record(after, output, after_digest),
            "pixels_changed": initial_pixels != after_pixels,
            "ime": {
                "status": "not_exercised",
                "reason": (
                    "KEYEVENTF_UNICODE delivers committed text; an installed IME "
                    "requires a separate host journey"
                ),
            },
        }
        manifest.parent.mkdir(parents=True, exist_ok=True)
        manifest.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        return result
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=capture.PROCESS_EXIT_TIMEOUT_SECONDS)


def main() -> None:
    if sys.platform != "win32":
        raise SystemExit("python_native_input_capture.py requires a Windows desktop")
    arguments = _parser().parse_args()
    try:
        result = _capture_input(
            arguments.command,
            arguments.command_arguments,
            arguments.cwd,
            arguments.text,
            arguments.shortcut,
            arguments.initial_output.resolve(),
            arguments.output.resolve(),
            arguments.manifest.resolve(),
        )
    except (OSError, RuntimeError, ValueError) as error:
        raise SystemExit(str(error)) from error
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
