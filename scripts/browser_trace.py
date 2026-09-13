"""Shared browser-engine and bounded trace artifacts."""
from __future__ import annotations

import enum
import hashlib
import math
import pathlib
import statistics
import struct
from dataclasses import dataclass, field
from typing import Any, Dict, List, Mapping, Optional, Tuple

from browser_protocol import (
    ROOT,
    BrowserRuntimeError,
    WebDriverClient,
    _safe_path,
)


UNSUPPORTED_NATIVE_OPERATIONS = (
    "native-file-dialog",
    "native-process-launch",
    "os-permission-grant",
)

# Frame timing is a bounded observation of the browser's animation-frame
# boundary. It is intentionally separate from compositor latency and does not
# claim to measure the host window or GPU driver.
FRAME_SAMPLE_COUNT = 8
MAX_FRAME_SAMPLE_COUNT = 32
MAX_FRAME_INTERVAL_MILLISECONDS = 120_000.0
MAX_BROWSER_HEAP_BYTES = 1 << 40
MAX_BROWSER_HEAP_LABEL_BYTES = 256


FRAME_TIMING_SCRIPT = """
const done = arguments[arguments.length - 1];
const count = arguments[0];
const timeout = arguments[1];
const maxCount = arguments[2];
if (!Number.isInteger(count) || count < 2 || count > maxCount) {
  done({ok: false, error: "invalid frame sample count"});
  return;
}
if (typeof window.requestAnimationFrame !== "function" ||
    !window.performance || typeof window.performance.now !== "function") {
  done({ok: false, error: "browser frame timing is unavailable"});
  return;
}
let settled = false;
const timestamps = [];
const finish = (value) => {
  if (settled) return;
  settled = true;
  window.clearTimeout(timer);
  done(value);
};
const sample = (timestamp) => {
  if (settled) return;
  timestamps.push(Number.isFinite(timestamp) ? timestamp : window.performance.now());
  if (timestamps.length === count) {
    finish({ok: true, timestamps});
    return;
  }
  window.requestAnimationFrame(sample);
};
const timer = window.setTimeout(() => finish({ok: false, error: "frame timing deadline exceeded"}), timeout);
window.requestAnimationFrame(sample);
"""


BROWSER_HEAP_SCRIPT = """
const memory = window.performance && window.performance.memory;
if (!memory || typeof memory !== "object") {
  return {available: false, reason: "performance.memory unavailable"};
}
return {
  available: true,
  source: "performance.memory",
  used_js_heap_bytes: memory.usedJSHeapSize,
  total_js_heap_bytes: memory.totalJSHeapSize,
  js_heap_limit_bytes: memory.jsHeapSizeLimit,
};
"""


class BrowserEngine(str, enum.Enum):
    """The browser engines admitted by the conformance matrix."""

    CHROMIUM = "chromium"
    FIREFOX = "firefox"
    WEBKIT = "webkit"

    @property
    def webdriver_name(self) -> str:
        """Return the W3C ``browserName`` capability for this engine."""
        return {self.CHROMIUM: "chrome", self.FIREFOX: "firefox", self.WEBKIT: "safari"}[self]

    @classmethod
    def parse(cls, value: str) -> "BrowserEngine":
        """Parse a matrix name and reject an untracked engine."""
        try:
            return cls(value.lower())
        except ValueError as error:
            admitted = ", ".join(engine.value for engine in cls)
            raise BrowserRuntimeError(f"unsupported browser engine {value!r}; choose {admitted}") from error


@dataclass
class Trace:
    """Structured evidence emitted by one browser-engine run."""

    engine: BrowserEngine
    url: str
    bridge: str
    revision: str
    capabilities: Mapping[str, Any]
    consumer_revision: Optional[str] = None
    actions: List[Dict[str, Any]] = field(default_factory=list)
    snapshots: List[Dict[str, Any]] = field(default_factory=list)
    screenshots: List[Dict[str, Any]] = field(default_factory=list)
    metrics: Dict[str, Any] = field(default_factory=dict)
    unsupported_operations: Tuple[str, ...] = UNSUPPORTED_NATIVE_OPERATIONS
    cleanup: Dict[str, Any] = field(default_factory=dict)

    def document(self, status: str = "passed") -> Dict[str, Any]:
        """Return the stable JSON schema consumed by the manual and CI."""
        document = {
            "schema": 1,
            "status": status,
            "engine": self.engine.value,
            "url": self.url,
            "bridge": self.bridge,
            "revision": self.revision,
            "capabilities": dict(self.capabilities),
            "actions": self.actions,
            "snapshots": self.snapshots,
            "screenshots": self.screenshots,
            "metrics": self.metrics,
            "unsupported_native_operations": list(self.unsupported_operations),
            "cleanup": self.cleanup,
        }
        if self.consumer_revision is not None:
            document["consumer_revision"] = self.consumer_revision
        return document


def screenshot(client: WebDriverClient, trace: Trace, directory: pathlib.Path, label: str) -> None:
    """Save one full-window PNG and record its digest and dimensions."""
    _safe_path(directory, directory=ROOT / "output")
    content = client.screenshot()
    path = _safe_path(directory / f"{label}.png", directory=directory)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)
    width, height = struct.unpack(">II", content[16:24])
    trace.screenshots.append(
        {
            "label": label,
            "path": path.relative_to(ROOT).as_posix(),
            "sha256": hashlib.sha256(content).hexdigest(),
            "width": width,
            "height": height,
            "bytes": len(content),
        }
    )


def frame_timing(
    client: WebDriverClient,
    trace: Trace,
    label: str,
    *,
    sample_count: int = FRAME_SAMPLE_COUNT,
    timeout_ms: int = 4_000,
) -> Dict[str, Any]:
    """Record bounded animation-frame intervals for one browser state.

    The returned values describe the interval between browser animation-frame
    callbacks. They are a browser-boundary metric; a caller must not treat
    them as operating-system compositor or GPU latency.
    """
    if not isinstance(label, str) or not label or len(label.encode("utf-8")) > 256:
        raise BrowserRuntimeError("frame timing label is empty or exceeds its bound")
    if not isinstance(sample_count, int) or not 2 <= sample_count <= MAX_FRAME_SAMPLE_COUNT:
        raise BrowserRuntimeError(
            f"frame sample count must be between 2 and {MAX_FRAME_SAMPLE_COUNT}"
        )
    if not isinstance(timeout_ms, int) or not 1 <= timeout_ms <= 120_000:
        raise BrowserRuntimeError("frame timing timeout is outside the configured bound")
    value = client.execute_async(
        FRAME_TIMING_SCRIPT, [sample_count, timeout_ms, MAX_FRAME_SAMPLE_COUNT]
    )
    if not isinstance(value, dict) or value.get("ok") is not True:
        detail = value.get("error") if isinstance(value, dict) else value
        raise BrowserRuntimeError(f"frame timing {label!r} failed: {detail!r}")
    timestamps = value.get("timestamps")
    if (
        not isinstance(timestamps, list)
        or len(timestamps) != sample_count
        or any(
            type(timestamp) not in (int, float)
            or not math.isfinite(float(timestamp))
            or float(timestamp) < 0.0
            for timestamp in timestamps
        )
    ):
        raise BrowserRuntimeError(f"frame timing {label!r} returned invalid timestamps")
    intervals = [float(later) - float(earlier) for earlier, later in zip(timestamps, timestamps[1:])]
    if any(
        not math.isfinite(interval)
        or interval <= 0.0
        or interval > MAX_FRAME_INTERVAL_MILLISECONDS
        for interval in intervals
    ):
        raise BrowserRuntimeError(f"frame timing {label!r} returned invalid intervals")
    measurement = {
        "label": label,
        "sample_count": len(intervals),
        "intervals_ms": intervals,
        "mean_ms": statistics.fmean(intervals),
        "stddev_ms": statistics.pstdev(intervals),
        "min_ms": min(intervals),
        "max_ms": max(intervals),
    }
    trace.metrics.setdefault("frame_intervals", []).append(measurement)
    return measurement


def browser_heap_sample(
    client: WebDriverClient,
    trace: Trace,
    label: str,
) -> Dict[str, Any]:
    """Record one optional browser JavaScript-heap observation.

    ``performance.memory`` is a Chromium-specific diagnostic surface.  An
    unavailable surface is recorded as an explicit observation instead of
    being treated as zero.  The result describes JavaScript heap counters only;
    it is not a WASM, native-process, compositor, GPU, or allocation-profile
    measurement.
    """
    if (
        not isinstance(label, str)
        or not label
        or len(label.encode("utf-8")) > MAX_BROWSER_HEAP_LABEL_BYTES
    ):
        raise BrowserRuntimeError("browser heap label is empty or exceeds its bound")
    value = client.execute(BROWSER_HEAP_SCRIPT)
    if not isinstance(value, dict):
        raise BrowserRuntimeError(f"browser heap sample {label!r} returned a non-object")
    if value.get("available") is False:
        reason = value.get("reason")
        if (
            not isinstance(reason, str)
            or not reason
            or len(reason.encode("utf-8")) > MAX_BROWSER_HEAP_LABEL_BYTES
        ):
            raise BrowserRuntimeError(f"browser heap sample {label!r} returned an invalid reason")
        measurement = {"label": label, "available": False, "reason": reason}
        trace.metrics.setdefault("browser_heap", []).append(measurement)
        return measurement
    if value.get("available") is not True or value.get("source") != "performance.memory":
        raise BrowserRuntimeError(f"browser heap sample {label!r} returned an invalid source")

    def memory_bytes(name: str) -> int:
        raw = value.get(name)
        if (
            type(raw) not in (int, float)
            or not math.isfinite(float(raw))
            or float(raw) < 0.0
            or float(raw) > MAX_BROWSER_HEAP_BYTES
            or not float(raw).is_integer()
        ):
            raise BrowserRuntimeError(f"browser heap sample {label!r} returned invalid {name}")
        return int(raw)

    used = memory_bytes("used_js_heap_bytes")
    total = memory_bytes("total_js_heap_bytes")
    limit = memory_bytes("js_heap_limit_bytes")
    if limit == 0 or used > total or total > limit:
        raise BrowserRuntimeError(f"browser heap sample {label!r} violated heap ordering")
    measurement = {
        "label": label,
        "available": True,
        "source": "performance.memory",
        "used_js_heap_bytes": used,
        "total_js_heap_bytes": total,
        "js_heap_limit_bytes": limit,
    }
    trace.metrics.setdefault("browser_heap", []).append(measurement)
    return measurement
