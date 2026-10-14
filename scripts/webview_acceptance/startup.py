"""Read-only initial-wait evidence, independent of native lifecycle authority.

The caller supplies its existing absolute deadline. Each existing event poll
may discover an actual owned window without another wait and read its desktop
rectangle. Full bounded BGRA/geometry equality suppresses unchanged image I/O;
only the latest changed BMP survives. A pending file replaces it after complete
encoding/hash, preserving the previous receipt on write failure. Focus and
occlusion remain unknown: these pixels do not establish DOM text or readiness.

Retained frame storage is one prior/current pair under the canonical native
frame limit; GDI/encoding retain their existing component bounds. There is no
frame history. Errors retain first/latest native context and counts per finite
operation, not an unbounded poll history. Metadata uses the existing trace byte
bound. Observer errors never replace the actual event wait's result or failure.
"""

import json
import os
from pathlib import Path
import sys
import time
import traceback

from native_capture import limits
from native_capture.images import files
from native_capture.windows import gdi, processes, window

from .contract import MAX_TRACE_BYTES


class StartupObservation:
    def __init__(self, native_process_id: int, deadline: float, output: Path):
        self.output = output
        self.deadline = deadline
        self.previous = None
        self.dirty = True
        self.report = {
            "schema": 1, "native_process_id": native_process_id,
            "deadline_monotonic": deadline, "attempts": 0,
            "successful_frames": 0, "changed_frames": 0, "latest_capture": None,
            "observer_errors": {},
            "source": "Win32 BitBlt desktop rectangle of an actually owned window",
            "visibility": "focus and occlusion unknown; no DOM or lifecycle claim",
        }

    def sample(self) -> None:
        if time.monotonic() >= self.deadline:
            return
        self.report["attempts"] += 1
        operation = "platform"
        try:
            if sys.platform != "win32":
                raise RuntimeError("actual native startup observation requires Windows")
            operation = "process_tree"
            owners = processes.process_tree(self.report["native_process_id"])
            operation = "window"
            bounds = window.window_for_processes(owners)
            operation = "geometry"
            observed = window.window_observation(bounds.handle)
            operation = "frame_bounds"
            limits.validate_dimensions(observed.bounds.width, observed.bounds.height,
                                       "startup observation")
            operation = "desktop_capture"
            captured = gdi.capture_screen_window(observed.bounds)
            if len(captured.pixels) != captured.width * captured.height * 4:
                raise ValueError("actual startup frame storage differs from its native geometry")
            frame = (observed.bounds.handle, captured.left, captured.top,
                     captured.width, captured.height, observed.dpi, captured.pixels)
            self.report["successful_frames"] += 1
            if self.previous == frame:
                return
            operation = "image_write"
            digest, size = self._write(observed.bounds, captured.pixels)
            self.report["latest_capture"] = {
                "path": str(self.output / "startup.bmp"), "sha256": digest,
                "bytes": size,
                "handle": observed.bounds.handle, "left": captured.left, "top": captured.top,
                "width": captured.width, "height": captured.height, "dpi": observed.dpi,
                "observed_monotonic_ns": time.monotonic_ns(),
            }
            self.previous = frame
            self.report["changed_frames"] += 1
            self.dirty = True
        except Exception as error:
            self._failure(operation, error)
        self._persist()

    def _write(self, bounds, pixels: bytes) -> tuple[str, int]:
        pending = self.output / "startup-pending.bmp"
        try:
            digest = files.write_capture(pending, bounds, pixels)
            size = pending.stat().st_size
            pending.replace(self.output / "startup.bmp")
            return digest, size
        finally:
            try:
                pending.unlink(missing_ok=True)
            except OSError as error:
                self._failure("image_cleanup", error)

    def _failure(self, operation: str, error: Exception) -> None:
        record = {
            "type": type(error).__qualname__, "message": str(error),
            "errno": getattr(error, "errno", None),
            "winerror": getattr(error, "winerror", None),
            "filename": os.fspath(error.filename) if getattr(error, "filename", None) else None,
            "filename2": os.fspath(error.filename2) if getattr(error, "filename2", None) else None,
            "notes": getattr(error, "__notes__", []),
            "traceback": "".join(traceback.format_exception(error)),
        }
        errors = self.report["observer_errors"]
        if operation not in errors:
            errors[operation] = {"count": 1, "first": record, "latest": record}
            self.dirty = True
        else:
            previous = errors[operation]
            previous["count"] += 1
            self.dirty |= previous["latest"] != record
            previous["latest"] = record

    def _persist(self) -> None:
        if not self.dirty:
            return
        try:
            data = json.dumps(self.report, indent=2, allow_nan=False).encode("utf-8")
            if len(data) > MAX_TRACE_BYTES:
                raise ValueError("actual startup metadata exceeds its existing trace byte bound")
            (self.output / "startup.json").write_bytes(data)
            self.dirty = False
        except (OSError, ValueError) as error:
            self._failure("metadata_write", error)

    def finish(self) -> None:
        """Flush final counters without acquiring another native observation."""
        self.dirty = True
        self._persist()
