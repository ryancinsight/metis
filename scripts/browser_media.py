"""Bounded runtime probes for browser media error and teardown semantics."""
from __future__ import annotations

from typing import Any, Dict, Mapping

from browser_protocol import BrowserRuntimeError, WebDriverClient, _bounded_text


MAX_MEDIA_PROBES = 2
MAX_MEDIA_KIND_BYTES = 16
MAX_MEDIA_REASON_BYTES = 256
MAX_MEDIA_SOURCE_BYTES = 128
MAX_MEDIA_TIMEOUT_MILLISECONDS = 10_000
MEDIA_PROBES = ("audio", "video")
MEDIA_SOURCES = ("data:audio/wav;base64,AAAA", "data:video/mp4;base64,AAAA")
MEDIA_EMPTY_NETWORK_STATES = (0, 3)  # NETWORK_EMPTY and NETWORK_NO_SOURCE.


MEDIA_SCRIPT = r"""
const done = arguments[arguments.length - 1];
const timeout = arguments[0];
if (!Number.isInteger(timeout) || timeout < 1 || timeout > 10000) {
  done({ok: false, error: "invalid media probe timeout"});
  return;
}
const container = document.createElement("div");
container.setAttribute("data-metis-media-probe", "true");
container.setAttribute("aria-hidden", "true");
container.style.cssText = "position:absolute; width:1px; height:1px; overflow:hidden;";
document.body.appendChild(container);
const sources = [
  {kind: "audio", source: "data:audio/wav;base64,AAAA"},
  {kind: "video", source: "data:video/mp4;base64,AAAA"},
];
const boundedReason = (value) => String(value || "media error").slice(0, 256);
const probe = (spec) => new Promise((resolve) => {
  const media = document.createElement(spec.kind);
  media.preload = "metadata";
  media.controls = false;
  media.muted = true;
  media.setAttribute("aria-hidden", "true");
  container.appendChild(media);
  let settled = false;
  let timer = 0;
  const finish = (eventName, reason) => {
    if (settled) return;
    settled = true;
    window.clearTimeout(timer);
    const mediaError = media.error;
    const errorCode = mediaError && Number.isInteger(mediaError.code) ? mediaError.code : null;
    const errorMessage = mediaError && mediaError.message ? boundedReason(mediaError.message) : null;
    media.pause();
    media.removeAttribute("src");
    media.load();
    const teardown = {
      ready_state: media.readyState,
      network_state: media.networkState,
      current_src: media.currentSrc || "",
      src_attribute: media.getAttribute("src"),
    };
    media.remove();
    resolve({
      kind: spec.kind,
      source: spec.source,
      event: eventName,
      reason: reason || null,
      error_code: errorCode,
      error_message: errorMessage,
      teardown,
      attached_after_remove: media.isConnected === true,
    });
  };
  media.addEventListener("error", () => finish("error", null), {once: true});
  media.addEventListener("loadedmetadata", () => finish("loadedmetadata", "invalid fixture decoded"), {once: true});
  timer = window.setTimeout(() => finish("timeout", "media error deadline exceeded"), timeout);
  media.src = spec.source;
  media.load();
});
(async () => {
  const records = [];
  for (const spec of sources) records.push(await probe(spec));
  container.remove();
  done({
    ok: true,
    media: records,
    remaining_probe_elements: document.querySelectorAll("[data-metis-media-probe]").length,
  });
})().catch((error) => {
  container.remove();
  done({ok: false, error: boundedReason(error && error.message)});
});
"""


def _bounded_string(value: Any, label: str, limit: int, *, required: bool = True) -> str:
    if not isinstance(value, str):
        if required and value is None:
            raise BrowserRuntimeError(f"media {label} is missing")
        raise BrowserRuntimeError(f"media {label} is not text")
    return _bounded_text(value, f"media {label}", limit)


def _bounded_optional_string(value: Any, label: str, limit: int) -> str | None:
    if value is None:
        return None
    return _bounded_string(value, label, limit)


def _validate_result(value: Any) -> Dict[str, Any]:
    if not isinstance(value, Mapping) or value.get("ok") is not True:
        detail = value.get("error") if isinstance(value, Mapping) else value
        raise BrowserRuntimeError(f"media probe failed: {detail!r}")
    records = value.get("media")
    if not isinstance(records, list) or len(records) != MAX_MEDIA_PROBES:
        raise BrowserRuntimeError("media probe returned an unexpected record count")
    validated = []
    for expected_kind, expected_source, record in zip(MEDIA_PROBES, MEDIA_SOURCES, records):
        if not isinstance(record, Mapping):
            raise BrowserRuntimeError("media probe returned a malformed record")
        kind = _bounded_string(record.get("kind"), "kind", MAX_MEDIA_KIND_BYTES)
        if kind != expected_kind:
            raise BrowserRuntimeError("media probe returned records in the wrong order")
        source = _bounded_string(record.get("source"), "source", MAX_MEDIA_SOURCE_BYTES)
        if source != expected_source:
            raise BrowserRuntimeError(f"media {kind!r} used an unexpected source")
        if record.get("event") != "error":
            reason = _bounded_optional_string(record.get("reason"), "reason", MAX_MEDIA_REASON_BYTES)
            raise BrowserRuntimeError(f"media {kind!r} did not report a decode error: {reason}")
        error_code = record.get("error_code")
        if type(error_code) is not int or not 1 <= error_code <= 4:
            raise BrowserRuntimeError(f"media {kind!r} returned an invalid MediaError code")
        error_message = _bounded_optional_string(record.get("error_message"), "error message", MAX_MEDIA_REASON_BYTES)
        teardown = record.get("teardown")
        if not isinstance(teardown, Mapping):
            raise BrowserRuntimeError(f"media {kind!r} teardown is malformed")
        if teardown.get("ready_state") != 0 or teardown.get("network_state") not in MEDIA_EMPTY_NETWORK_STATES:
            raise BrowserRuntimeError(
                f"media {kind!r} did not return to the empty/no-source state: "
                f"ready_state={teardown.get('ready_state')!r}, network_state={teardown.get('network_state')!r}"
            )
        if teardown.get("current_src") != "" or teardown.get("src_attribute") is not None:
            raise BrowserRuntimeError(f"media {kind!r} retained its source after teardown")
        if record.get("attached_after_remove") is not False:
            raise BrowserRuntimeError(f"media {kind!r} remained attached after teardown")
        validated.append({
            "kind": kind,
            "source": source,
            "event": "error",
            "error_code": error_code,
            "error_message": error_message,
            "teardown": {
                "ready_state": 0,
                "network_state": teardown["network_state"],
                "current_src": "",
                "src_attribute": None,
            },
            "attached_after_remove": False,
        })
    remaining = value.get("remaining_probe_elements")
    if type(remaining) is not int or remaining != 0:
        raise BrowserRuntimeError("media probe elements were not released")
    return {"media": validated, "remaining_probe_elements": remaining}


def capture_media(
    client: WebDriverClient,
    trace: Any,
    label: str,
    *,
    timeout_ms: int = 5_000,
) -> Dict[str, Any]:
    """Record browser media errors and the empty state after source teardown."""
    label = _bounded_text(label, "media label", MAX_MEDIA_REASON_BYTES)
    if not label:
        raise BrowserRuntimeError("media label is empty")
    if type(timeout_ms) is not int or not 1 <= timeout_ms <= MAX_MEDIA_TIMEOUT_MILLISECONDS:
        raise BrowserRuntimeError("media probe timeout is outside its bound")
    measurement = {"label": label, **_validate_result(client.execute_async(MEDIA_SCRIPT, [timeout_ms]))}
    trace.metrics.setdefault("media", []).append(measurement)
    return measurement
