"""Exercise the production Windows UI Automation surface of metis-app."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import pathlib
import subprocess
import sys
import time
from typing import Any

import python_native_capture as capture


MAX_VALUE_BYTES = 128
MAX_UIA_OUTPUT_BYTES = 256 * 1024
UIA_TIMEOUT_SECONDS = 15
UIA_POLL_MILLISECONDS = 25


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Exercise a visible production Metis window through Windows UI Automation."
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
        "--patient-value",
        default="UIA-PATIENT",
        help="bounded value delivered through ValuePattern.SetValue",
    )
    parser.add_argument("--initial-output", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--manifest", type=pathlib.Path, required=True)
    return parser


def _validate_value(value: str) -> str:
    encoded = value.encode("utf-8")
    if not value:
        raise ValueError("--patient-value must not be empty")
    if len(encoded) > MAX_VALUE_BYTES:
        raise ValueError(f"--patient-value exceeds the {MAX_VALUE_BYTES}-byte bound")
    if any(ord(character) < 0x20 or ord(character) == 0x7F for character in value):
        raise ValueError("--patient-value contains a control character")
    return value


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
    for label, path in (("--initial-output", initial_output), ("--output", output)):
        if path.suffix.lower() not in {".bmp", ".png"}:
            raise ValueError(f"{label} must end in .bmp or .png")
    if manifest.suffix.lower() != ".json":
        raise ValueError("--manifest must end in .json")
    return executable


def _powershell_script(handle: int, value: str) -> str:
    encoded_value = base64.b64encode(value.encode("utf-8")).decode("ascii")
    return f"""
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$handle = [IntPtr]{handle}
$requested = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{encoded_value}'))
$scope = [System.Windows.Automation.TreeScope]::Descendants
$trueCondition = [System.Windows.Automation.Condition]::TrueCondition

function Get-Nodes([System.Windows.Automation.AutomationElement] $root) {{
    @($root.FindAll($scope, $trueCondition))
}}

function Get-Patient([System.Windows.Automation.AutomationElement] $root) {{
    Get-Nodes $root | Where-Object {{
        $_.Current.ControlType.ProgrammaticName -eq 'ControlType.Edit' -and
        $_.Current.Name -eq 'Patient ID'
    }} | Select-Object -First 1
}}

function Get-Submit([System.Windows.Automation.AutomationElement] $root) {{
    Get-Nodes $root | Where-Object {{
        $_.Current.ControlType.ProgrammaticName -eq 'ControlType.Button' -and
        $_.Current.Name -like '*SUBMIT CALCULATION*'
    }} | Select-Object -First 1
}}

function Get-Status([System.Windows.Automation.AutomationElement] $root) {{
    Get-Nodes $root | Where-Object {{
        $_.Current.ControlType.ProgrammaticName -eq 'ControlType.Group' -and
        $_.Current.Name -like 'Backend Calculation Output*'
    }} | Select-Object -First 1
}}

function Get-TreeRecord([System.Windows.Automation.AutomationElement] $root) {{
    $records = @(Get-Nodes $root | Where-Object {{ $_.Current.Name -or $_.Current.AutomationId }} | ForEach-Object {{
        [pscustomobject]@{{
            role = $_.Current.ControlType.ProgrammaticName
            name = $_.Current.Name
            automation_id = $_.Current.AutomationId
            enabled = $_.Current.IsEnabled
            keyboard_focusable = $_.Current.IsKeyboardFocusable
        }}
    }})
    if ($records.Count -eq 0 -or $records.Count -gt 256) {{ throw 'bounded UI Automation tree is empty or oversized' }}
    $records
}}

$root = [System.Windows.Automation.AutomationElement]::FromHandle($handle)
if ($null -eq $root) {{ throw 'UI Automation root is unavailable' }}
$tree = @(Get-TreeRecord $root)
$patient = Get-Patient $root
$submit = Get-Submit $root
if ($null -eq $patient -or $null -eq $submit) {{ throw 'required UI Automation controls are missing' }}
$valuePattern = $patient.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
$invokePattern = $submit.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
$before = $valuePattern.Current.Value
$patient.SetFocus()
$valuePattern.SetValue($requested)
$deadline = [DateTime]::UtcNow.AddSeconds({UIA_TIMEOUT_SECONDS})
$after = $null
do {{
    $root = [System.Windows.Automation.AutomationElement]::FromHandle($handle)
    $patient = Get-Patient $root
    if ($null -ne $patient) {{ $after = $patient.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value }}
    if ($after -eq $requested) {{ break }}
    Start-Sleep -Milliseconds {UIA_POLL_MILLISECONDS}
}} while ([DateTime]::UtcNow -lt $deadline)
if ($after -ne $requested) {{ throw "UI Automation SetValue did not apply the requested value; observed '$after'" }}
$invokePattern.Invoke()
$status = $null
do {{
    $root = [System.Windows.Automation.AutomationElement]::FromHandle($handle)
    $statusNode = Get-Status $root
    if ($null -ne $statusNode) {{ $status = $statusNode.Current.Name }}
    if ($status -and $status -notlike '*Awaiting Backend Calculation*') {{ break }}
    Start-Sleep -Milliseconds {UIA_POLL_MILLISECONDS}
}} while ([DateTime]::UtcNow -lt $deadline)
if (-not $status -or $status -like '*Awaiting Backend Calculation*') {{ throw 'UI Automation submit did not produce a backend result' }}
[pscustomobject]@{{
    schema = 1
    controls = [pscustomobject]@{{
        patient = [pscustomobject]@{{ role = $patient.Current.ControlType.ProgrammaticName; name = $patient.Current.Name }}
        submit = [pscustomobject]@{{ role = $submit.Current.ControlType.ProgrammaticName; name = $submit.Current.Name }}
    }}
    tree = [pscustomobject]@{{ named_count = $tree.Count; nodes = $tree }}
    value = [pscustomobject]@{{ before = $before; after = $after; action = 'ValuePattern.SetValue' }}
    submit_action = 'InvokePattern.Invoke'
    status = $status
}} | ConvertTo-Json -Compress -Depth 8
"""


def _run_uia(handle: int, value: str) -> dict[str, Any]:
    script = _powershell_script(handle, value)
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
        check=False,
        timeout=UIA_TIMEOUT_SECONDS + 5,
    )
    stdout = completed.stdout.strip()
    stderr = completed.stderr.strip()
    if completed.returncode != 0:
        detail = stderr or stdout or f"PowerShell exited with {completed.returncode}"
        raise RuntimeError(f"Windows UI Automation probe failed: {detail}")
    if not stdout or len(stdout.encode("utf-8")) > MAX_UIA_OUTPUT_BYTES:
        raise RuntimeError("Windows UI Automation probe returned an empty or oversized trace")
    try:
        result = json.loads(stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError("Windows UI Automation probe returned malformed JSON") from error
    if not isinstance(result, dict) or result.get("schema") != 1:
        raise RuntimeError("Windows UI Automation probe returned an unsupported schema")
    return result


def _capture(
    command: pathlib.Path,
    command_arguments: list[str],
    cwd: pathlib.Path | None,
    value: str,
    initial_output: pathlib.Path,
    output: pathlib.Path,
    manifest: pathlib.Path,
) -> dict[str, Any]:
    if sys.platform != "win32":
        raise RuntimeError("native accessibility capture requires Windows")
    executable = _validate_paths(command, initial_output, output, manifest)
    value = _validate_value(value)
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
    bounds: capture._WindowBounds | None = None
    try:
        bounds = capture._wait_for_process_window(process)
        initial_observation = capture._window_observation(bounds.handle)
        initial_pixels = capture._capture_window(initial_observation.bounds)
        initial_digest = capture._write_capture(
            initial_output, initial_observation.bounds, initial_pixels
        )
        trace = _run_uia(bounds.handle, value)
        after_observation = capture._window_observation(bounds.handle)
        after_pixels = capture._capture_window(after_observation.bounds)
        after_digest = capture._write_capture(output, after_observation.bounds, after_pixels)
        capture._close_window(bounds.handle)
        return_code = process.wait(timeout=capture.PROCESS_EXIT_TIMEOUT_SECONDS)
        if return_code != 0:
            raise RuntimeError(f"native process exited with status {return_code}")
        result: dict[str, Any] = {
            "schema": 1,
            "process_returncode": return_code,
            "window": {
                "handle": bounds.handle,
                "initial": [initial_observation.bounds.width, initial_observation.bounds.height],
                "after": [after_observation.bounds.width, after_observation.bounds.height],
                "dpi": after_observation.dpi,
            },
            "value_sha256": hashlib.sha256(value.encode("utf-8")).hexdigest(),
            "initial": {"image": initial_output.as_posix(), "sha256": initial_digest},
            "after": {"image": output.as_posix(), "sha256": after_digest},
            "pixels_changed": initial_pixels != after_pixels,
            "uia": trace,
        }
        manifest.parent.mkdir(parents=True, exist_ok=True)
        manifest.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        return result
    finally:
        if bounds is not None and process.poll() is None:
            try:
                capture._close_window(bounds.handle)
            except OSError:
                pass
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=capture.PROCESS_EXIT_TIMEOUT_SECONDS)


def main() -> None:
    if sys.platform != "win32":
        raise SystemExit("python_native_accessibility.py requires a Windows desktop")
    arguments = _parser().parse_args()
    try:
        result = _capture(
            arguments.command,
            arguments.command_arguments,
            arguments.cwd,
            arguments.patient_value,
            arguments.initial_output.resolve(),
            arguments.output.resolve(),
            arguments.manifest.resolve(),
        )
    except (OSError, RuntimeError, ValueError, subprocess.SubprocessError) as error:
        raise SystemExit(str(error)) from error
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
