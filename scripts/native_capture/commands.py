"""Native command capture, resize/display probes and process cleanup."""
from __future__ import annotations
import pathlib
import subprocess
import sys
from typing import Any
from native_capture import limits, model
from native_capture.images import files
from native_capture.windows import gdi, processes, window
from native_display import display_scale_milli, enumerate_monitors, flush_window_queue, move_window_to_monitor


def validate_resize_options(
    command: pathlib.Path | None,
    resize: tuple[int, int] | None,
    resize_output: pathlib.Path | None,
    output: pathlib.Path | None = None,
) -> None:
    if resize is None:
        if resize_output is not None:
            raise ValueError("--resize-output requires --resize")
        return
    if command is None:
        raise ValueError("--resize requires --command")
    limits.validate_dimensions(*resize, "resize")
    if resize_output is None:
        raise ValueError("--resize requires --resize-output")
    if output is not None and resize_output.resolve() == output.resolve():
        raise ValueError("--resize-output must differ from --output")


def validate_monitor_options(
    command: pathlib.Path | None,
    move_monitor: int | None,
    monitor_output: pathlib.Path | None,
    require_dpi_change: bool,
    output: pathlib.Path | None = None,
) -> None:
    if move_monitor is None:
        if monitor_output is not None:
            raise ValueError("--monitor-output requires --move-monitor")
        if require_dpi_change:
            raise ValueError("--require-dpi-change requires --move-monitor")
        return
    if command is None:
        raise ValueError("--move-monitor requires --command")
    if move_monitor < 0:
        raise ValueError("--move-monitor index must be non-negative")
    if monitor_output is None:
        raise ValueError("--move-monitor requires --monitor-output")
    if output is not None and monitor_output.resolve() == output.resolve():
        raise ValueError("--monitor-output must differ from --output")


def capture_command(
    command: pathlib.Path,
    command_arguments: list[str],
    cwd: pathlib.Path | None,
    output: pathlib.Path,
    resize: tuple[int, int] | None = None,
    resize_output: pathlib.Path | None = None,
    move_monitor: int | None = None,
    monitor_output: pathlib.Path | None = None,
    require_dpi_change: bool = False,
) -> dict[str, Any]:
    """Launch one visible process, capture it, and close it after bounded probes."""
    if sys.platform != "win32":
        raise RuntimeError("visible native capture requires Windows")
    validate_resize_options(command, resize, resize_output, output)
    validate_monitor_options(
        command, move_monitor, monitor_output, require_dpi_change, output
    )
    if resize is not None and move_monitor is not None:
        raise ValueError("--resize and --move-monitor are mutually exclusive")
    executable = command.resolve(strict=True)
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
    bounds: model.WindowBounds | None = None
    try:
        bounds = processes.wait_for_process_window(process)
        initial = window.window_observation(bounds.handle)
        initial_pixels = gdi.capture_window(initial.bounds)
        initial_digest = files.write_capture(output, initial.bounds, initial_pixels)
        result: dict[str, Any]
        if resize is None and move_monitor is None:
            result = {
                "process_returncode": None,
                "window": {
                    "width": initial.bounds.width,
                    "height": initial.bounds.height,
                },
                "client": {
                    "width": initial.client_width,
                    "height": initial.client_height,
                },
                "dpi": initial.dpi,
                "image": output.as_posix(),
                "sha256": initial_digest,
            }
        elif resize is not None:
            if resize_output is None:
                raise ValueError("--resize requires --resize-output")
            resized = window.resize_window(bounds.handle, *resize)
            resized_pixels = gdi.capture_window(resized.bounds)
            resized_digest = files.write_capture(resize_output, resized.bounds, resized_pixels)
            result = {
                "initial": {
                    "window": {
                        "width": initial.bounds.width,
                        "height": initial.bounds.height,
                    },
                    "client": {
                        "width": initial.client_width,
                        "height": initial.client_height,
                    },
                    "dpi": initial.dpi,
                    "image": output.as_posix(),
                    "sha256": initial_digest,
                },
                "resized": {
                    "window": {
                        "width": resized.bounds.width,
                        "height": resized.bounds.height,
                    },
                    "client": {
                        "width": resized.client_width,
                        "height": resized.client_height,
                    },
                    "dpi": resized.dpi,
                    "image": resize_output.as_posix(),
                    "sha256": resized_digest,
                },
                "resize": {
                    "requested_client": {
                        "width": resize[0],
                        "height": resize[1],
                    },
                    "client_size_applied": (
                        resized.client_width == resize[0]
                        and resized.client_height == resize[1]
                    ),
                    "pixels_changed": initial_pixels != resized_pixels,
                    "dpi_changed": initial.dpi != resized.dpi,
                },
            }
        else:
            if monitor_output is None:
                raise ValueError("--move-monitor requires --monitor-output")
            monitors = enumerate_monitors()
            if move_monitor >= len(monitors):
                raise ValueError(
                    f"--move-monitor index {move_monitor} exceeds {len(monitors)} attached monitors"
                )
            monitor = monitors[move_monitor]
            move_window_to_monitor(
                bounds.handle,
                monitor,
                initial.bounds.width,
                initial.bounds.height,
            )
            # WM_DPICHANGED is delivered on the window thread. Synchronize
            # after the move before reading the host observation.
            flush_window_queue(bounds.handle)
            moved = window.window_observation(bounds.handle)
            moved_pixels = gdi.capture_window(moved.bounds)
            moved_digest = files.write_capture(monitor_output, moved.bounds, moved_pixels)
            dpi_changed = initial.dpi != moved.dpi
            if require_dpi_change and not dpi_changed:
                raise RuntimeError(
                    "moving the window did not produce a physical display-scale transition"
                )
            result = {
                "initial": {
                    "window": {
                        "width": initial.bounds.width,
                        "height": initial.bounds.height,
                    },
                    "client": {
                        "width": initial.client_width,
                        "height": initial.client_height,
                    },
                    "dpi": initial.dpi,
                    "display_scale_milli": display_scale_milli(initial.dpi),
                    "image": output.as_posix(),
                    "sha256": initial_digest,
                },
                "moved": {
                    "window": {
                        "width": moved.bounds.width,
                        "height": moved.bounds.height,
                    },
                    "client": {
                        "width": moved.client_width,
                        "height": moved.client_height,
                    },
                    "dpi": moved.dpi,
                    "display_scale_milli": display_scale_milli(moved.dpi),
                    "image": monitor_output.as_posix(),
                    "sha256": moved_digest,
                },
                "monitor_transition": {
                    "monitor_index": move_monitor,
                    "monitor_count": len(monitors),
                    "monitor": {
                        "left": monitor.left,
                        "top": monitor.top,
                        "right": monitor.right,
                        "bottom": monitor.bottom,
                    },
                    "dpi_changed": dpi_changed,
                    "pixels_changed": initial_pixels != moved_pixels,
                    "observed": dpi_changed,
                },
            }
        window.close_window(bounds.handle)
        return_code = process.wait(timeout=limits.PROCESS_EXIT_TIMEOUT_SECONDS)
        if return_code != 0:
            raise RuntimeError(f"native process exited with status {return_code}")
        result["process_returncode"] = return_code
        return result
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=limits.PROCESS_EXIT_TIMEOUT_SECONDS)
