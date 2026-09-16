"""Exercise the saved-study browser gallery and record exact evidence."""

from __future__ import annotations

import json
import math
import pathlib
import sys
from typing import Any, Mapping

from browser_canvas import settle_canvas_input
from browser_gallery_actions import _arrow_batch, _keyboard_action
from browser_gallery_artifacts import _write_gallery_screenshots
from browser_gallery_trace import (
    ARROW_BATCH_SIZE,
    AXES,
    _cleanup_event_trace,
    _consume_events,
    _install_event_trace,
    _probe_invalid_slice_api,
    _snapshot,
    _validate_expected_counts,
    _validate_transition,
)
from browser_protocol import ROOT, BrowserRuntimeError, WebDriverClient, _safe_path


def capture_slice_gallery(
    client: WebDriverClient,
    output_directory: pathlib.Path,
    *,
    expected_counts: Mapping[str, int],
) -> dict[str, Any]:
    """Exercise all consumer gallery sliders and capture trusted evidence.

    The caller must load its study or media and wait for its canvases to report
    presented frames. ``expected_counts`` is the caller's independent
    per-axis shape oracle, keyed by ``axial``, ``coronal`` and ``sagittal``.
    """
    if not isinstance(output_directory, pathlib.Path):
        raise BrowserRuntimeError("gallery output directory must be a pathlib.Path")
    directory = _safe_path(output_directory, directory=ROOT / "output")
    directory.mkdir(parents=True, exist_ok=True)
    counts = _validate_expected_counts(expected_counts)
    initial = _snapshot(client, counts)
    invalid_api_probes = _probe_invalid_slice_api(client, counts, initial)
    initial_indices = {axis: initial[axis]["index"] for axis in AXES}
    actions: list[dict[str, Any]] = []
    expected_listener_count = _install_event_trace(client)
    trace_installed = True
    actions_released = False
    released_listeners = 0
    current = initial
    try:
        for axis in AXES:
            slider_id = f"slice-{axis}"
            slider_element = client.find(f"#{slider_id}")

            before = current
            client.click(slider_element)
            settle_canvas_input(client)
            current = _snapshot(client, counts)
            click_changed = before[axis]["index"] != current[axis]["index"]
            click_required = ["click"] + (["input", "change"] if click_changed else [])
            click_events = _consume_events(client, axis, click_required)
            _validate_transition(before, current, axis, None)
            actions.append(
                {
                    "axis": axis,
                    "action": "native-click",
                    "from_index": before[axis]["index"],
                    "to_index": current[axis]["index"],
                    "generation": current[axis]["generation"],
                    "rgba_sha256": current[axis]["rgba_sha256"],
                    "events": click_events,
                }
            )

            current = _keyboard_action(
                client, counts, actions, axis, "Home", "home", current, 0
            )
            current = _keyboard_action(
                client, counts, actions, axis, "End", "end", current, counts[axis] - 1
            )

            slider_width = current[axis]["slider_width"]
            horizontal_extent = max(1, math.floor(slider_width / 2.0) - 3)
            before = current
            client.pointer_drag(
                slider_element,
                (horizontal_extent, 0),
                (-horizontal_extent, 0),
                source_id=f"metis-gallery-{axis}-pointer",
            )
            settle_canvas_input(client)
            current = _snapshot(client, counts)
            drag_events = _consume_events(
                client,
                axis,
                ("pointerdown", "pointermove", "pointerup", "input", "change"),
            )
            _validate_transition(before, current, axis, 0)
            actions.append(
                {
                    "axis": axis,
                    "action": "pointer-drag-end-to-home",
                    "start": [horizontal_extent, 0],
                    "end": [-horizontal_extent, 0],
                    "from_index": before[axis]["index"],
                    "to_index": current[axis]["index"],
                    "generation": current[axis]["generation"],
                    "rgba_sha256": current[axis]["rgba_sha256"],
                    "events": drag_events,
                }
            )

            current = _keyboard_action(
                client, counts, actions, axis, "Home", "restore-home", current, 0
            )
            remaining = initial_indices[axis]
            while remaining:
                batch = min(remaining, ARROW_BATCH_SIZE)
                current = _arrow_batch(client, counts, actions, axis, current, batch)
                remaining -= batch
            if current[axis]["index"] != initial_indices[axis]:
                raise BrowserRuntimeError(f"{axis} slider did not restore its initial slice")

        restored = _snapshot(client, counts)
        for axis in AXES:
            if (
                restored[axis]["index"] != initial[axis]["index"]
                or restored[axis]["rgba_sha256"] != initial[axis]["rgba_sha256"]
            ):
                raise BrowserRuntimeError(f"{axis} gallery frame did not restore exactly")
            # Different indices can legitimately contain identical RGBA planes,
            # for example empty boundary slices.  Generation proves every index
            # transition rendered; the digest set independently proves the full
            # traversal displayed more than one anatomical image.
            observed_digests = {initial[axis]["rgba_sha256"]}
            observed_digests.update(
                action["rgba_sha256"] for action in actions if action["axis"] == axis
            )
            if len(observed_digests) < 2:
                raise BrowserRuntimeError(f"{axis} slider traversal exposed no changed RGBA frame")
        client.release_actions()
        actions_released = True
        released_listeners = _cleanup_event_trace(client, expected_listener_count)
        trace_installed = False
        screenshots = _write_gallery_screenshots(client, directory)
        evidence = {
            "schema": 1,
            "expected_counts": counts,
            "invalid_api_probes": invalid_api_probes,
            "initial": initial,
            "actions": actions,
            "restored": restored,
            "screenshots": screenshots,
            "cleanup": {
                "active_input_sources_released": True,
                "diagnostic_listener_count": released_listeners,
                "diagnostic_listeners_released": True,
            },
        }
        evidence_path = _safe_path(directory / "gallery-slices.json", directory=directory)
        evidence["artifact"] = evidence_path.relative_to(ROOT).as_posix()
        encoded = json.dumps(evidence, indent=2, sort_keys=True) + "\n"
        if len(encoded.encode("utf-8")) > 512 * 1024:
            raise BrowserRuntimeError("gallery slice evidence exceeds the 512 KiB trace bound")
        evidence_path.write_text(encoded, encoding="utf-8", newline="\n")
        return evidence
    finally:
        primary_error = sys.exc_info()[1]
        cleanup_errors = []
        if trace_installed:
            try:
                _cleanup_event_trace(client, expected_listener_count)
            except BrowserRuntimeError as cleanup_error:
                cleanup_errors.append(("gallery slider event listener cleanup", cleanup_error))
        if not actions_released:
            try:
                client.release_actions()
            except BrowserRuntimeError as cleanup_error:
                cleanup_errors.append(("browser input release", cleanup_error))
        if cleanup_errors:
            if primary_error is not None:
                for operation, cleanup_error in cleanup_errors:
                    primary_error.add_note(f"{operation} also failed: {cleanup_error}")
            else:
                operation, cleanup_error = cleanup_errors[0]
                for later_operation, later_error in cleanup_errors[1:]:
                    cleanup_error.add_note(f"{later_operation} also failed: {later_error}")
                cleanup_error.add_note(f"failed operation: {operation}")
                raise cleanup_error
