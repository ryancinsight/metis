"""Capture an installed Windows IME journey through the production native host."""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import subprocess
import sys
from typing import Any

import python_native_capture as native_capture
import python_native_input_capture as unicode_capture
from python_native_ime_host import (
    ENGLISH_KLID,
    IME_LAYOUTS,
    MAX_KEYS,
    UNICODE_FIXTURES,
    VK_ESCAPE,
    VK_IME_ON,
    VK_KANJI,
    activate_layout,
    ascii_keys,
    capture_state,
    focus_native_window,
    ime_state,
    installed_input_methods,
    send_physical_keys,
    value_record,
)


# Keep the validation helpers discoverable to the focused script tests.
_ascii_keys = ascii_keys


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Capture installed Windows IME input through production Metis."
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
    parser.add_argument("--layout", choices=tuple(IME_LAYOUTS), default="japanese")
    parser.add_argument("--output-dir", type=pathlib.Path, required=True)
    parser.add_argument("--manifest", type=pathlib.Path, required=True)
    parser.add_argument("--source-revision", required=True)
    return parser


def _validate_source_revision(revision: str) -> str:
    if re.fullmatch(r"[0-9a-f]{7,64}", revision) is None:
        raise ValueError("--source-revision must be a lowercase hexadecimal Git revision")
    return revision


def _validate_command(command: pathlib.Path, cwd: pathlib.Path | None) -> pathlib.Path:
    executable = command.resolve(strict=True)
    if not executable.is_file():
        raise ValueError("--command must name a file")
    if cwd is not None:
        resolved_cwd = cwd.resolve(strict=True)
        if not resolved_cwd.is_dir():
            raise NotADirectoryError(resolved_cwd)
    return executable


def _validate_outputs(
    output_dir: pathlib.Path, manifest: pathlib.Path
) -> tuple[pathlib.Path, pathlib.Path]:
    output_dir = output_dir.resolve()
    manifest = manifest.resolve()
    if manifest.suffix.lower() != ".json":
        raise ValueError("--manifest must end in .json")
    if manifest.parent != output_dir:
        raise ValueError("--manifest must be directly inside --output-dir")
    return output_dir, manifest


def _launch(
    executable: pathlib.Path,
    command_arguments: list[str],
    cwd: pathlib.Path | None,
) -> tuple[subprocess.Popen[bytes], native_capture._WindowBounds]:
    process = subprocess.Popen(
        [str(executable), *command_arguments],
        cwd=cwd,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    try:
        return process, native_capture._wait_for_process_window(process)
    except BaseException:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=native_capture.PROCESS_EXIT_TIMEOUT_SECONDS)
        raise


def _close(process: subprocess.Popen[bytes], handle: int) -> None:
    if process.poll() is None:
        native_capture._close_window(handle)
        process.wait(timeout=native_capture.PROCESS_EXIT_TIMEOUT_SECONDS)
    if process.returncode != 0:
        raise RuntimeError(f"native process exited with status {process.returncode}")


def _capture_ime_journey(
    executable: pathlib.Path,
    command_arguments: list[str],
    cwd: pathlib.Path | None,
    layout_name: str,
    output_dir: pathlib.Path,
) -> dict[str, Any]:
    layout_spec = IME_LAYOUTS[layout_name]
    journey_dir = output_dir / "cjk"
    journey_dir.mkdir(parents=True, exist_ok=True)
    process, bounds = _launch(executable, command_arguments, cwd)
    try:
        before = capture_state(bounds.handle, "before", journey_dir)
        layout, foreground_set = focus_native_window(bounds.handle)
        if not foreground_set:
            raise RuntimeError("Windows denied foreground activation for the IME journey")
        activated = activate_layout(bounds.handle, layout_spec["klid"])
        ime_before = ime_state(bounds.handle)
        send_physical_keys((VK_IME_ON,))
        unicode_capture._flush_window(bounds.handle)
        ime_after_on = ime_state(bounds.handle)
        toggle = ("VK_IME_ON",)
        if not ime_after_on["open"]:
            send_physical_keys((VK_KANJI,))
            unicode_capture._flush_window(bounds.handle)
            ime_after_on = ime_state(bounds.handle)
            toggle = ("VK_IME_ON", "VK_KANJI")
        if not ime_after_on["context"] or not ime_after_on["open"]:
            raise RuntimeError(
                f"layout {layout_name} did not expose an open installed IME "
                f"(before={ime_before}, after={ime_after_on})"
            )

        send_physical_keys(ascii_keys(layout_spec["keys"]))
        preedit = capture_state(bounds.handle, "preedit", journey_dir)
        _, foreground_set = focus_native_window(bounds.handle)
        if not foreground_set:
            raise RuntimeError("Windows lost foreground activation before IME commit")
        send_physical_keys(tuple(layout_spec["commit_keys"]))
        committed = capture_state(bounds.handle, "commit", journey_dir)
        _, foreground_set = focus_native_window(bounds.handle)
        if not foreground_set:
            raise RuntimeError("Windows lost foreground activation before IME cancellation")
        send_physical_keys(ascii_keys(layout_spec["keys"]))
        cancel_preedit = capture_state(bounds.handle, "cancel-preedit", journey_dir)
        _, foreground_set = focus_native_window(bounds.handle)
        if not foreground_set:
            raise RuntimeError("Windows lost foreground activation before IME escape")
        send_physical_keys((VK_ESCAPE,))
        canceled = capture_state(bounds.handle, "cancel", journey_dir)
        expected = before["value"]["text"] + layout_spec["expected_commit"]
        checks = {
            "preedit_value_unchanged": preedit["value"] == before["value"],
            "preedit_pixels_changed": preedit["sha256"] != before["sha256"],
            "commit_value_exact": committed["value"]["text"] == expected,
            "cancel_value_unchanged": canceled["value"] == committed["value"],
            "cancel_preedit_value_unchanged": cancel_preedit["value"] == committed["value"],
        }
        if not all(checks.values()):
            raise RuntimeError(f"IME journey contract failed: {checks}")
        _close(process, bounds.handle)
        return {
            "status": "passed",
            "layout": activated,
            "focus_keyboard_layout": f"0x{layout:016x}",
            "foreground_window_set": foreground_set,
            "ime": {
                "before": ime_before,
                "after_on": ime_after_on,
                "toggle": toggle,
            },
            "keys": layout_spec["keys"],
            "expected_commit": value_record(layout_spec["expected_commit"]),
            "states": {
                "before": before,
                "preedit": preedit,
                "commit": committed,
                "cancel_preedit": cancel_preedit,
                "cancel": canceled,
            },
            "checks": checks,
        }
    finally:
        if process.poll() is None:
            try:
                native_capture._close_window(bounds.handle)
            except OSError:
                process.terminate()
            process.wait(timeout=native_capture.PROCESS_EXIT_TIMEOUT_SECONDS)


def _capture_unicode_fixture(
    executable: pathlib.Path,
    command_arguments: list[str],
    cwd: pathlib.Path | None,
    name: str,
    text: str,
    output_dir: pathlib.Path,
) -> dict[str, Any]:
    fixture_dir = output_dir / name
    fixture_dir.mkdir(parents=True, exist_ok=True)
    encoded, units = unicode_capture._validate_text(text)
    process, bounds = _launch(executable, command_arguments, cwd)
    try:
        before = capture_state(bounds.handle, "before", fixture_dir)
        layout, foreground_set = focus_native_window(bounds.handle)
        if not foreground_set:
            raise RuntimeError(f"Windows denied foreground activation for {name}")
        fixture_layout = activate_layout(bounds.handle, ENGLISH_KLID)
        _, foreground_set = focus_native_window(bounds.handle)
        if not foreground_set:
            raise RuntimeError(f"Windows denied foreground activation after layout setup for {name}")
        unicode_capture._send_unicode_text(units)
        after = capture_state(bounds.handle, "after", fixture_dir)
        checks = {
            "value_exact": after["value"]["text"] == before["value"]["text"] + text,
            "pixels_changed": after["sha256"] != before["sha256"],
        }
        if not all(checks.values()):
            raise RuntimeError(f"Unicode fixture contract failed for {name}: {checks}")
        _close(process, bounds.handle)
        return {
            "mechanism": "Win32 SendInput KEYEVENTF_UNICODE",
            "text": value_record(text),
            "focus_keyboard_layout": f"0x{layout:016x}",
            "fixture_keyboard_layout": fixture_layout,
            "foreground_window_set": foreground_set,
            "states": {"before": before, "after": after},
            "checks": checks,
            "utf8_bytes": len(encoded),
            "utf16_units": len(units),
        }
    finally:
        if process.poll() is None:
            try:
                native_capture._close_window(bounds.handle)
            except OSError:
                process.terminate()
            process.wait(timeout=native_capture.PROCESS_EXIT_TIMEOUT_SECONDS)


def _capture(
    command: pathlib.Path,
    command_arguments: list[str],
    cwd: pathlib.Path | None,
    layout_name: str,
    output_dir: pathlib.Path,
    manifest: pathlib.Path,
    source_revision: str,
) -> dict[str, Any]:
    if sys.platform != "win32":
        raise RuntimeError("native IME capture requires Windows")
    executable = _validate_command(command, cwd)
    output_dir, manifest = _validate_outputs(output_dir, manifest)
    source_revision = _validate_source_revision(source_revision)
    output_dir.mkdir(parents=True, exist_ok=True)
    host_input_methods = installed_input_methods()
    fixtures = {
        name: _capture_unicode_fixture(
            executable,
            command_arguments,
            cwd,
            name,
            text,
            output_dir,
        )
        for name, text in UNICODE_FIXTURES.items()
    }
    try:
        ime = _capture_ime_journey(executable, command_arguments, cwd, layout_name, output_dir)
        status = "passed"
    except RuntimeError as error:
        ime = {
            "status": "blocked",
            "requested_layout": layout_name,
            "reason": str(error),
        }
        status = "blocked"
    result = {
        "schema": 1,
        "status": status,
        "source_revision": source_revision,
        "command": [str(executable), *command_arguments],
        "host_input_methods": host_input_methods,
        "ime_journey": ime,
        "unicode_fixtures": fixtures,
    }
    manifest.write_text(
        json.dumps(result, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return result


def main() -> None:
    if sys.platform != "win32":
        raise SystemExit("python_native_ime_capture.py requires a Windows desktop")
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
    arguments = _parser().parse_args()
    try:
        result = _capture(
            arguments.command,
            arguments.command_arguments,
            arguments.cwd,
            arguments.layout,
            arguments.output_dir,
            arguments.manifest,
            arguments.source_revision,
        )
    except (OSError, RuntimeError, ValueError, subprocess.SubprocessError) as error:
        raise SystemExit(str(error)) from error
    print(json.dumps(result, ensure_ascii=False, indent=2, sort_keys=True))
    if result["status"] != "passed":
        raise SystemExit(2)


if __name__ == "__main__":
    main()
