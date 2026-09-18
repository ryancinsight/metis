"""Bounded runtime probe for the browser's same-origin font loading."""
from __future__ import annotations

import math
from typing import Any, Dict, Mapping

from browser_protocol import BrowserRuntimeError, WebDriverClient, _bounded_text


MAX_FONT_FAMILY_BYTES = 64
MAX_FONT_PATH_BYTES = 128
MAX_FONT_REASON_BYTES = 256
MAX_FONT_SAMPLE_BYTES = 16
MAX_FONT_SAMPLE_PIXELS = 4_096.0
MAX_FONT_TIMEOUT_MILLISECONDS = 10_000
MAX_FONT_FIXTURE_BYTES = 64 * 1024
# The committed fixture is project-owned and deliberately unlike a text face:
# every supported glyph carries a 1,200/1,000 em advance, so a face that is
# actually applied measures the sample far wider than the fallback stack.
FONT_FAMILY = "Metis Probe"
FONT_FIXTURE_PATH = "assets/metis-probe.woff2"
FONT_FIXTURE_SOURCE = "/assets/metis-probe.woff2"
FONT_INVALID_PATH = "assets/metis-mark.svg"
FONT_INVALID_SOURCE = "/assets/metis-mark.svg"
FONT_SAMPLE_TEXT = "AB01"
FONT_METRIC_SIZE_PIXELS = 64


FONT_SCRIPT = r"""
const done = arguments[arguments.length - 1];
const path = arguments[0];
const invalid = arguments[1];
const family = arguments[2];
const sample = arguments[3];
const size = arguments[4];
const timeout = arguments[5];
if (typeof path !== "string" || path.length === 0 || path.length > 128 ||
    typeof invalid !== "string" || invalid.length === 0 || invalid.length > 128 ||
    typeof family !== "string" || family.length === 0 || family.length > 64 ||
    typeof sample !== "string" || sample.length === 0 || sample.length > 16 ||
    !Number.isInteger(size) || size < 8 || size > 512 ||
    !Number.isInteger(timeout) || timeout < 1 || timeout > 10000) {
  done({ok: false, error: "invalid font probe arguments"});
  return;
}
const boundedReason = (value) => String(value || "font load failed").slice(0, 256);
const fontSet = document.fonts;
if (!fontSet || typeof window.FontFace !== "function" ||
    typeof fontSet.add !== "function" || typeof fontSet.delete !== "function" ||
    typeof fontSet.has !== "function") {
  done({ok: false, error: "FontFaceSet API unavailable"});
  return;
}
const sameOrigin = (value) => {
  let url;
  try {
    url = new URL(value, document.baseURI);
  } catch (error) {
    return {error: "font URL is malformed"};
  }
  if (url.origin !== window.location.origin) {
    return {error: "font URL is cross-origin"};
  }
  return {url};
};
const measure = (stack) => {
  const context = document.createElement("canvas").getContext("2d");
  if (!context) return null;
  context.font = `${size}px ${stack}`;
  return context.measureText(sample).width;
};
const deadline = (promise) => new Promise((resolve, reject) => {
  const timer = window.setTimeout(() => reject(new Error("font load deadline exceeded")), timeout);
  promise.then(
    (value) => {
      window.clearTimeout(timer);
      resolve(value);
    },
    (error) => {
      window.clearTimeout(timer);
      reject(error);
    },
  );
});
const loadFace = async (url) => {
  const face = new window.FontFace(family, `url("${url.href}")`, {
    style: "normal",
    weight: "400",
    stretch: "normal",
  });
  await deadline(face.load());
  return face;
};
(async () => {
  const target = sameOrigin(path);
  if (target.error) {
    done({ok: false, error: target.error});
    return;
  }
  const denied = sameOrigin(invalid);
  if (denied.error) {
    done({ok: false, error: denied.error});
    return;
  }
  const baseline_size = fontSet.size;
  const fallback_width = measure("sans-serif");
  const face = await loadFace(target.url);
  fontSet.add(face);
  const loaded_width = measure(`"${family}", sans-serif`);
  const check_loaded = typeof fontSet.check === "function" ? fontSet.check(`${size}px "${family}"`, sample) : null;
  const size_before_teardown = fontSet.size;
  fontSet.delete(face);
  const size_after_teardown = fontSet.size;
  let denied_reason = null;
  let denied_registered = false;
  try {
    const rejected = await loadFace(denied.url);
    denied_registered = fontSet.has(rejected) === true;
    fontSet.delete(rejected);
  } catch (error) {
    denied_reason = boundedReason(error && error.message);
  }
  done({ok: true, font: {
    family,
    sample,
    path,
    source: target.url.pathname,
    status: face.status,
    baseline_size,
    size_before_teardown,
    size_after_teardown,
    registered_after_delete: fontSet.has(face) === true,
    fallback_width,
    loaded_width,
    check_loaded,
    denied_path: invalid,
    denied_source: denied.url.pathname,
    denied_reason,
    denied_registered,
  }});
})().catch((error) => {
  done({ok: false, error: boundedReason(error && error.message)});
});
"""


def _bounded_string(value: Any, label: str, limit: int, *, required: bool = True) -> str:
    if not isinstance(value, str):
        if required and value is None:
            raise BrowserRuntimeError(f"font {label} is missing")
        raise BrowserRuntimeError(f"font {label} is not text")
    return _bounded_text(value, f"font {label}", limit)


def _count(value: Any, label: str) -> int:
    if type(value) is not int or value < 0:
        raise BrowserRuntimeError(f"font {label} is not a bounded count")
    return value


def _pixels(value: Any, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        raise BrowserRuntimeError(f"font {label} is not a finite number")
    if not 0.0 < value <= MAX_FONT_SAMPLE_PIXELS:
        raise BrowserRuntimeError(f"font {label} is outside its {MAX_FONT_SAMPLE_PIXELS}px bound")
    return float(value)


def _validate_result(value: Any) -> Dict[str, Any]:
    """Require a same-origin face to load, apply, and release, and a non-font to fail."""
    if not isinstance(value, Mapping) or value.get("ok") is not True:
        detail = value.get("error") if isinstance(value, Mapping) else value
        raise BrowserRuntimeError(f"font probe failed: {detail!r}")
    record = value.get("font")
    if not isinstance(record, Mapping):
        raise BrowserRuntimeError("font probe returned a malformed record")
    if record.get("family") != FONT_FAMILY:
        raise BrowserRuntimeError("font probe returned an unexpected family")
    if record.get("sample") != FONT_SAMPLE_TEXT:
        raise BrowserRuntimeError("font probe measured an unexpected sample")
    if record.get("path") != FONT_FIXTURE_PATH:
        raise BrowserRuntimeError("font probe returned an unexpected fixture path")
    source = _bounded_string(record.get("source"), "source", MAX_FONT_PATH_BYTES, required=False)
    if source != FONT_FIXTURE_SOURCE:
        raise BrowserRuntimeError("font fixture resolved to an unexpected source")
    if record.get("status") != "loaded":
        raise BrowserRuntimeError(f"font fixture did not reach the loaded state: {record.get('status')!r}")
    baseline_size = _count(record.get("baseline_size"), "baseline size")
    size_before_teardown = _count(record.get("size_before_teardown"), "registered size")
    size_after_teardown = _count(record.get("size_after_teardown"), "released size")
    if size_before_teardown != baseline_size + 1:
        raise BrowserRuntimeError("font probe did not retain exactly one registered face")
    if size_after_teardown != baseline_size:
        raise BrowserRuntimeError("font probe did not release its registered face")
    if record.get("registered_after_delete") is not False:
        raise BrowserRuntimeError("font probe left its face registered after teardown")
    fallback_width = _pixels(record.get("fallback_width"), "fallback width")
    loaded_width = _pixels(record.get("loaded_width"), "loaded width")
    if abs(loaded_width - fallback_width) <= 1.0:
        raise BrowserRuntimeError("font probe could not separate the loaded face from the fallback stack")
    if record.get("check_loaded") is not True:
        raise BrowserRuntimeError("FontFaceSet.check did not report the loaded family")
    if record.get("denied_path") != FONT_INVALID_PATH:
        raise BrowserRuntimeError("font probe returned an unexpected invalid-resource path")
    denied_source = _bounded_string(record.get("denied_source"), "denied source", MAX_FONT_PATH_BYTES, required=False)
    if denied_source != FONT_INVALID_SOURCE:
        raise BrowserRuntimeError("invalid font resource resolved to an unexpected source")
    if record.get("denied_registered") is not False:
        raise BrowserRuntimeError("invalid font resource was registered as a loaded face")
    denied_reason = _bounded_string(record.get("denied_reason"), "denied reason", MAX_FONT_REASON_BYTES)
    if not denied_reason:
        raise BrowserRuntimeError("invalid font resource was not rejected with a reason")
    return {
        "family": FONT_FAMILY,
        "sample": FONT_SAMPLE_TEXT,
        "path": FONT_FIXTURE_PATH,
        "source": source,
        "status": "loaded",
        "baseline_size": baseline_size,
        "size_before_teardown": size_before_teardown,
        "size_after_teardown": size_after_teardown,
        "registered_after_delete": False,
        "fallback_width": fallback_width,
        "loaded_width": loaded_width,
        "check_loaded": True,
        "denied_path": FONT_INVALID_PATH,
        "denied_source": denied_source,
        "denied_reason": denied_reason,
        "denied_registered": False,
    }


def capture_font(
    client: WebDriverClient,
    trace: Any,
    label: str,
    *,
    timeout_ms: int = 5_000,
) -> Dict[str, Any]:
    """Load one same-origin font face, confirm it is applied, then release it."""
    label = _bounded_text(label, "font label", MAX_FONT_REASON_BYTES)
    if not label:
        raise BrowserRuntimeError("font label is empty")
    if type(timeout_ms) is not int or not 1 <= timeout_ms <= MAX_FONT_TIMEOUT_MILLISECONDS:
        raise BrowserRuntimeError("font probe timeout is outside its bound")
    measurement = {
        "label": label,
        **_validate_result(
            client.execute_async(
                FONT_SCRIPT,
                [FONT_FIXTURE_PATH, FONT_INVALID_PATH, FONT_FAMILY, FONT_SAMPLE_TEXT, FONT_METRIC_SIZE_PIXELS, timeout_ms],
            )
        ),
    }
    trace.metrics.setdefault("font", []).append(measurement)
    return measurement
