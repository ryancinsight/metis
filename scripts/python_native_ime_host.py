"""Windows primitives used by the production native IME evidence runner."""

from __future__ import annotations

import base64
import ctypes
import hashlib
import json
import pathlib
import subprocess
from typing import Any

import python_native_capture as capture
import python_native_input_capture as unicode_capture


MAX_KEYS = 32
MAX_UIA_OUTPUT_BYTES = 16 * 1024
ENGLISH_KLID = "00000409"
KLF_ACTIVATE = 0x00000001
KLF_NOTELLSHELL = 0x00000080
VK_ESCAPE = 0x1B
VK_IME_ON = 0x16
VK_KANJI = 0x19
VK_MENU = 0x12
VK_RETURN = 0x0D
VK_SPACE = 0x20
UIA_TIMEOUT_SECONDS = 5


IME_LAYOUTS: dict[str, dict[str, Any]] = {
    "japanese": {
        "klid": "00000411",
        "keys": "konnichiha",
        "commit_keys": (VK_RETURN,),
        "expected_commit": "こんにちは",
    },
    "chinese-simplified": {
        "klid": "00000804",
        "keys": "nihao",
        "commit_keys": (VK_SPACE, VK_RETURN),
        "expected_commit": "你好",
    },
}

UNICODE_FIXTURES = {
    "combining": "e\u0301",
    "emoji": "👩‍🔬",
    "mixed-direction": "ABC אבג 123",
}


def _powershell_script(handle: int) -> str:
    return f"""
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$handle = [IntPtr]{handle}
$scope = [System.Windows.Automation.TreeScope]::Descendants
$trueCondition = [System.Windows.Automation.Condition]::TrueCondition
$root = [System.Windows.Automation.AutomationElement]::FromHandle($handle)
if ($null -eq $root) {{ throw 'UI Automation root is unavailable' }}
$patient = @($root.FindAll($scope, $trueCondition) | Where-Object {{
    $_.Current.ControlType.ProgrammaticName -eq 'ControlType.Edit' -and
    $_.Current.Name -eq 'Patient ID'
}} | Select-Object -First 1)
if ($patient.Count -ne 1) {{ throw 'Patient ID UI Automation edit is unavailable' }}
$valuePattern = $patient[0].GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
$value = $valuePattern.Current.Value
if ($null -eq $value) {{ throw 'Patient ID UI Automation value is null' }}
$valueBase64 = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes([string]$value))
[pscustomobject]@{{ schema = 1; value_base64 = $valueBase64 }} |
    ConvertTo-Json -Compress -Depth 3
"""


def _read_patient_value(handle: int) -> str:
    encoded = base64.b64encode(_powershell_script(handle).encode("utf-16-le")).decode("ascii")
    completed = subprocess.run(
        [
            "powershell.exe",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-EncodedCommand",
            encoded,
        ],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
        timeout=UIA_TIMEOUT_SECONDS + 2,
    )
    stdout = completed.stdout.strip()
    stderr = completed.stderr.strip()
    if completed.returncode != 0:
        detail = stderr or stdout or f"PowerShell exited with {completed.returncode}"
        raise RuntimeError(f"UI Automation value read failed: {detail}")
    if not stdout or len(stdout.encode("utf-8")) > MAX_UIA_OUTPUT_BYTES:
        raise RuntimeError("UI Automation value read returned empty or oversized output")
    try:
        result = json.loads(stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError("UI Automation value read returned malformed JSON") from error
    if not isinstance(result, dict) or result.get("schema") != 1:
        raise RuntimeError("UI Automation value read returned an unsupported schema")
    value_base64 = result.get("value_base64")
    if not isinstance(value_base64, str):
        raise RuntimeError("UI Automation value read returned no base64 value")
    try:
        return base64.b64decode(value_base64, validate=True).decode("utf-8")
    except (ValueError, UnicodeDecodeError) as error:
        raise RuntimeError("UI Automation value read returned invalid UTF-8") from error


def installed_input_methods() -> list[dict[str, str]]:
    script = """
$ErrorActionPreference = 'Stop'
$list = @(Get-WinUserLanguageList | ForEach-Object {
    [pscustomobject]@{
        language_tag = [string]$_.LanguageTag
        input_method_tips = [string]($_.InputMethodTips -join ',')
    }
})
$list | ConvertTo-Json -Compress -Depth 3
"""
    encoded = base64.b64encode(script.encode("utf-16-le")).decode("ascii")
    completed = subprocess.run(
        [
            "powershell.exe",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-EncodedCommand",
            encoded,
        ],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
        timeout=UIA_TIMEOUT_SECONDS,
    )
    stdout = completed.stdout.strip()
    stderr = completed.stderr.strip()
    if completed.returncode != 0:
        detail = stderr or stdout or f"PowerShell exited with {completed.returncode}"
        raise RuntimeError(f"Windows input-method inventory failed: {detail}")
    if not stdout or len(stdout.encode("utf-8")) > MAX_UIA_OUTPUT_BYTES:
        raise RuntimeError("Windows input-method inventory returned empty or oversized output")
    try:
        value = json.loads(stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError("Windows input-method inventory returned malformed JSON") from error
    if isinstance(value, dict):
        value = [value]
    if not isinstance(value, list) or any(not isinstance(item, dict) for item in value):
        raise RuntimeError("Windows input-method inventory returned an unsupported shape")
    return [
        {
            "language_tag": str(item.get("language_tag", "")),
            "input_method_tips": str(item.get("input_method_tips", "")),
        }
        for item in value
    ]


def ime_state(handle: int) -> dict[str, Any]:
    imm32 = ctypes.WinDLL("imm32", use_last_error=True)
    imm32.ImmGetContext.argtypes = [ctypes.c_void_p]
    imm32.ImmGetContext.restype = ctypes.c_void_p
    imm32.ImmGetOpenStatus.argtypes = [ctypes.c_void_p]
    imm32.ImmGetOpenStatus.restype = ctypes.c_bool
    imm32.ImmGetConversionStatus.argtypes = [
        ctypes.c_void_p,
        ctypes.POINTER(ctypes.c_uint32),
        ctypes.POINTER(ctypes.c_uint32),
    ]
    imm32.ImmGetConversionStatus.restype = ctypes.c_bool
    imm32.ImmReleaseContext.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
    imm32.ImmReleaseContext.restype = ctypes.c_bool
    context = imm32.ImmGetContext(handle)
    if not context:
        return {"context": False, "open": False, "conversion": None, "sentence": None}
    try:
        conversion = ctypes.c_uint32()
        sentence = ctypes.c_uint32()
        has_conversion = bool(
            imm32.ImmGetConversionStatus(context, ctypes.byref(conversion), ctypes.byref(sentence))
        )
        return {
            "context": True,
            "open": bool(imm32.ImmGetOpenStatus(context)),
            "conversion": conversion.value if has_conversion else None,
            "sentence": sentence.value if has_conversion else None,
        }
    finally:
        if not imm32.ImmReleaseContext(handle, context):
            raise ctypes.WinError()


def activate_layout(handle: int, klid: str) -> dict[str, Any]:
    user32 = ctypes.windll.user32
    kernel32 = ctypes.windll.kernel32
    user32.GetWindowThreadProcessId.argtypes = [
        ctypes.c_void_p,
        ctypes.POINTER(ctypes.c_uint32),
    ]
    user32.GetWindowThreadProcessId.restype = ctypes.c_uint32
    user32.AttachThreadInput.argtypes = [ctypes.c_uint32, ctypes.c_uint32, ctypes.c_bool]
    user32.AttachThreadInput.restype = ctypes.c_bool
    user32.LoadKeyboardLayoutW.argtypes = [ctypes.c_wchar_p, ctypes.c_uint32]
    user32.LoadKeyboardLayoutW.restype = ctypes.c_void_p
    user32.ActivateKeyboardLayout.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
    user32.ActivateKeyboardLayout.restype = ctypes.c_void_p
    user32.GetKeyboardLayout.argtypes = [ctypes.c_uint32]
    user32.GetKeyboardLayout.restype = ctypes.c_void_p
    kernel32.GetCurrentThreadId.restype = ctypes.c_uint32

    process_id = ctypes.c_uint32()
    target_thread = user32.GetWindowThreadProcessId(handle, ctypes.byref(process_id))
    if target_thread == 0:
        raise ctypes.WinError()
    current_thread = kernel32.GetCurrentThreadId()
    previous_layout = int(user32.GetKeyboardLayout(target_thread) or 0)
    attached = current_thread != target_thread
    if attached and not user32.AttachThreadInput(current_thread, target_thread, True):
        raise ctypes.WinError()
    try:
        layout = user32.LoadKeyboardLayoutW(klid, KLF_ACTIVATE | KLF_NOTELLSHELL)
        if not layout:
            raise ctypes.WinError()
        if not user32.ActivateKeyboardLayout(layout, 0):
            raise ctypes.WinError()
        active_layout = int(user32.GetKeyboardLayout(target_thread) or 0)
    finally:
        if attached and not user32.AttachThreadInput(current_thread, target_thread, False):
            raise ctypes.WinError()
    return {
        "klid": klid,
        "hkl": f"0x{int(layout):016x}",
        "previous_hkl": f"0x{previous_layout:016x}",
        "active_hkl": f"0x{active_layout:016x}",
        "target_thread": target_thread,
    }


def send_physical_keys(keys: tuple[int, ...]) -> None:
    if not keys or len(keys) > MAX_KEYS:
        raise ValueError(f"physical key sequence must contain 1..{MAX_KEYS} keys")
    for virtual_key in keys:
        if not 0 < virtual_key <= 0xFF:
            raise ValueError(f"virtual key is outside the Win32 byte range: {virtual_key}")
        unicode_capture._send_virtual_key(virtual_key, key_up=False)
        unicode_capture._send_virtual_key(virtual_key, key_up=True)


def release_foreground_lock() -> None:
    user32 = ctypes.windll.user32
    user32.keybd_event.argtypes = [
        ctypes.c_ubyte,
        ctypes.c_ubyte,
        ctypes.c_uint32,
        ctypes.c_size_t,
    ]
    user32.keybd_event.restype = None
    user32.keybd_event(VK_MENU, 0, 0, 0)
    user32.keybd_event(VK_MENU, 0, unicode_capture.KEYEVENTF_KEYUP, 0)


def focus_native_window(handle: int) -> tuple[int, bool]:
    user32 = ctypes.windll.user32
    kernel32 = ctypes.windll.kernel32
    user32.GetForegroundWindow.restype = ctypes.c_void_p
    user32.GetWindowThreadProcessId.argtypes = [
        ctypes.c_void_p,
        ctypes.POINTER(ctypes.c_uint32),
    ]
    user32.GetWindowThreadProcessId.restype = ctypes.c_uint32
    user32.AttachThreadInput.argtypes = [ctypes.c_uint32, ctypes.c_uint32, ctypes.c_bool]
    user32.AttachThreadInput.restype = ctypes.c_bool
    user32.SetForegroundWindow.argtypes = [ctypes.c_void_p]
    user32.SetForegroundWindow.restype = ctypes.c_bool
    user32.SetActiveWindow.argtypes = [ctypes.c_void_p]
    user32.SetActiveWindow.restype = ctypes.c_void_p
    user32.BringWindowToTop.argtypes = [ctypes.c_void_p]
    user32.BringWindowToTop.restype = ctypes.c_bool
    user32.SetFocus.argtypes = [ctypes.c_void_p]
    user32.SetFocus.restype = ctypes.c_void_p
    user32.GetKeyboardLayout.argtypes = [ctypes.c_uint32]
    user32.GetKeyboardLayout.restype = ctypes.c_void_p
    kernel32.GetCurrentThreadId.restype = ctypes.c_uint32

    target_thread = user32.GetWindowThreadProcessId(handle, None)
    if target_thread == 0:
        raise ctypes.WinError()
    foreground = int(user32.GetForegroundWindow() or 0)
    foreground_thread = user32.GetWindowThreadProcessId(foreground, None) if foreground else 0
    current_thread = kernel32.GetCurrentThreadId()
    release_foreground_lock()
    attached = foreground_thread not in (0, current_thread) and bool(
        user32.AttachThreadInput(current_thread, foreground_thread, True)
    )
    try:
        user32.BringWindowToTop(handle)
        user32.SetActiveWindow(handle)
        user32.SetForegroundWindow(handle)
        user32.SetFocus(handle)
        # The Alt tap releases the foreground lock, but Windows can leave the
        # target queue's menu modifier logically down across the attachment.
        # Repeat a complete tap on the target queue before delivering text.
        unicode_capture._send_virtual_key(VK_MENU, key_up=False)
        unicode_capture._send_virtual_key(VK_MENU, key_up=True)
    finally:
        if attached and not user32.AttachThreadInput(current_thread, foreground_thread, False):
            raise ctypes.WinError()
    active = int(user32.GetForegroundWindow() or 0)
    return int(user32.GetKeyboardLayout(target_thread) or 0), active == handle


def ascii_keys(value: str) -> tuple[int, ...]:
    keys = tuple(ord(character.upper()) for character in value)
    if any(key < ord("A") or key > ord("Z") for key in keys):
        raise ValueError("IME fixture keys must be ASCII letters")
    return keys


def value_record(value: str) -> dict[str, Any]:
    encoded = value.encode("utf-8")
    return {
        "text": value,
        "utf8_bytes": len(encoded),
        "sha256": hashlib.sha256(encoded).hexdigest(),
    }


def capture_state(handle: int, name: str, output_dir: pathlib.Path) -> dict[str, Any]:
    unicode_capture._flush_window(handle)
    observation = capture._window_observation(handle)
    pixels = capture._capture_window(observation.bounds)
    image = output_dir / f"{name}.png"
    digest = capture._write_capture(image, observation.bounds, pixels)
    value = _read_patient_value(handle)
    return {
        "image": image.as_posix(),
        "sha256": digest,
        "window": {
            "outer": [observation.bounds.width, observation.bounds.height],
            "client": [observation.client_width, observation.client_height],
            "dpi": observation.dpi,
        },
        "value": value_record(value),
    }
