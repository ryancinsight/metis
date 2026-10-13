"""Own the process, capture/input sequence and evidence manifest."""
from __future__ import annotations
import hashlib
import json
import pathlib
import subprocess
import sys
import time
from typing import Any
from native_capture import limits
from native_capture.images import files
from native_capture.windows import gdi, processes, window
from native_input import evidence, frames
from native_input.windows import focus, geometry, keyboard, pointer, queue


def validate_paths(
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


def capture_input(
    command: pathlib.Path,
    command_arguments: list[str],
    cwd: pathlib.Path | None,
    text: str | None,
    shortcut: str | None,
    click: tuple[int, int] | None,
    scroll: tuple[int, int, int] | None,
    initial_output: pathlib.Path,
    output: pathlib.Path,
    manifest: pathlib.Path,
    screen: bool = False,
) -> dict[str, Any]:
    if text is None and shortcut is None and click is None and scroll is None:
        raise ValueError("provide --text, --shortcut, --click-client, or --scroll-client")
    if sys.platform != "win32":
        raise RuntimeError("native Unicode capture requires Windows")
    encoded, units = keyboard.validate_text(text) if text is not None else (b"", ())
    if click is not None:
        geometry.validate_click(*click)
    if scroll is not None:
        geometry.validate_scroll(*scroll)
    executable = validate_paths(command, initial_output, output, manifest)
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
        bounds = processes.wait_for_process_window(process)
        layout, foreground_set, focus_target = focus.focus_window(bounds.handle)
        # Every input claim here is trusted OS input meant to reach the
        # target window, and Windows consumes the first click on an inactive
        # window for activation instead of delivering it (wheel delivery to
        # inactive windows is likewise a user setting), so recording any
        # input claim requires verified activation.
        if not foreground_set:
            raise RuntimeError(
                "Windows denied foreground activation; "
                "no trusted input claim can be recorded"
            )
        if screen:
            # Screen BitBlt reads whatever covers the window's screen rect,
            # so a covering window corrupts every frame even when the
            # capture succeeds; refuse before the initial frame is taken.
            pointer.guard_surface_visible(bounds.handle)
        window_capturer = (
            gdi.capture_screen_window if screen else gdi.capture_window
        )
        initial = window.window_observation(bounds.handle)
        initial_pixels = window_capturer(initial.bounds)
        # A composition host may show its window before the page paints;
        # wait for a settled, non-uniform frame so the recorded state is the
        # rendered surface, within the same bounded settle budget.
        initial_settle_rounds = 0
        stable_rounds = 0
        previous_pixels = initial_pixels
        while initial_settle_rounds < frames.SETTLE_ROUNDS:
            if (
                not frames.blank_frame(initial_pixels, initial.bounds.width, initial.bounds.height)
                and initial_pixels == previous_pixels
            ):
                stable_rounds += 1
                if stable_rounds >= frames.STABLE_ROUNDS:
                    break
            else:
                stable_rounds = 0
            previous_pixels = initial_pixels
            time.sleep(frames.SETTLE_WAIT_SECONDS)
            queue.flush_window(bounds.handle)
            initial = window.window_observation(bounds.handle)
            initial_pixels = window_capturer(initial.bounds)
            initial_settle_rounds += 1
        # The hosted content child (a browser render surface) is created
        # asynchronously after the top-level window appears; when keyboard
        # input follows and a content child now exists, move the keyboard
        # focus there so typed text and navigation keys reach the page
        # instead of the host's own focus chain.
        if (text is not None or shortcut is not None) and focus.content_child(
            bounds.handle
        ) not in (None, focus_target):
            layout, foreground_set, focus_target = focus.focus_window(bounds.handle)
            if not foreground_set:
                raise RuntimeError(
                    "Windows denied foreground activation; "
                    "no trusted input claim can be recorded"
                )
        keyboard_focus_record = {
            "kind": (
                "content-child" if focus_target != bounds.handle else "top-level"
            ),
            "class": focus.window_class(focus_target),
        }
        initial_digest = files.write_capture(initial_output, initial.bounds, initial_pixels)
        scroll_record = None
        if scroll is not None:
            before_scroll = window_capturer(
                window.window_observation(bounds.handle).bounds
            )
            focus.require_foreground(bounds.handle)
            pointer.send_wheel(bounds.handle, scroll[0], scroll[1], scroll[2])
            # Let the browser's scroll animation finish so a following click
            # targets the moved content instead of what briefly lies under
            # the cursor.
            scroll_settle_rounds = 0
            stable_rounds = 0
            settled_pixels = window_capturer(
                window.window_observation(bounds.handle).bounds
            )
            while scroll_settle_rounds < frames.SETTLE_ROUNDS:
                time.sleep(frames.SETTLE_WAIT_SECONDS)
                queue.flush_window(bounds.handle)
                current_pixels = window_capturer(
                    window.window_observation(bounds.handle).bounds
                )
                scroll_settle_rounds += 1
                if current_pixels == settled_pixels:
                    stable_rounds += 1
                    if stable_rounds >= frames.STABLE_ROUNDS:
                        break
                else:
                    stable_rounds = 0
                settled_pixels = current_pixels
            scroll_record = evidence.scroll(
                (scroll[0], scroll[1]), scroll[2], scroll_settle_rounds,
                settled_pixels != before_scroll,
            )
        click_record = None
        if click is not None:
            focus.require_foreground(bounds.handle)
            screen_x, screen_y = pointer.send_pointer_click(bounds.handle, click[0], click[1])
            click_record = evidence.click((click[0], click[1]), (screen_x, screen_y))
        # Typed text and submit shortcuts follow the pointer phases so they
        # reach the control the click focused.
        focus.require_foreground(bounds.handle)
        if text is not None:
            keyboard.send_unicode_text(units)
        if shortcut is not None:
            keyboard.send_shortcut(shortcut)
        queue.flush_window(bounds.handle)
        after = window.window_observation(bounds.handle)
        after_pixels = window_capturer(after.bounds)
        # The input-driven update may render after the input queue drains
        # (a click submits through an asynchronous bridge), and a focused
        # text caret keeps alternating frames while the page is idle, so a
        # frame that merely holds still is not evidence of a response.
        settle_rounds = 0
        settle_started = time.monotonic()
        reference_digest = hashlib.sha256(after_pixels).digest()
        recent_digests: list[bytes] = [reference_digest]
        while settle_rounds < frames.SETTLE_ROUNDS:
            time.sleep(frames.SETTLE_WAIT_SECONDS)
            queue.flush_window(bounds.handle)
            after = window.window_observation(bounds.handle)
            after_pixels = window_capturer(after.bounds)
            settle_rounds += 1
            frame_digest = hashlib.sha256(after_pixels).digest()
            repeated = frame_digest in recent_digests
            recent_digests.append(frame_digest)
            del recent_digests[:-frames.SETTLE_HISTORY]
            if frames.after_settled(
                changed=frame_digest != reference_digest,
                repeated=repeated,
                observed_seconds=time.monotonic() - settle_started,
                minimum_seconds=frames.POST_INPUT_MIN_OBSERVE_SECONDS,
            ):
                break
        after_digest = files.write_capture(output, after.bounds, after_pixels)
        window.close_window(bounds.handle)
        return_code = process.wait(timeout=limits.PROCESS_EXIT_TIMEOUT_SECONDS)
        if return_code != 0:
            raise RuntimeError(f"native process exited with status {return_code}")
        result: dict[str, Any] = {
            "process_returncode": return_code,
            "capture": {
                "mechanism": "screen BitBlt" if screen else "PrintWindow WM_PRINT",
                "initial_settle_rounds": initial_settle_rounds,
                "after_settle_rounds": settle_rounds,
                "after_observed_seconds": round(time.monotonic() - settle_started, 3),
            },
            "initial": frames.record(initial, initial_output, initial_digest),
            "after": frames.record(after, output, after_digest),
            "pixels_changed": initial_pixels != after_pixels,
        }
        result.update(evidence.record(evidence.InputObservation(
            encoded, units, text is not None, shortcut, layout,
            keyboard_focus_record, foreground_set, scroll_record, click_record,
        )))
        manifest.parent.mkdir(parents=True, exist_ok=True)
        manifest.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        return result
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=limits.PROCESS_EXIT_TIMEOUT_SECONDS)
