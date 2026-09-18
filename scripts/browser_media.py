"""Bounded runtime probes for browser media loading, playback and teardown."""
from __future__ import annotations

import math
import urllib.parse
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
MEDIA_PLAYBACK_PATHS = ("assets/metis-tone.wav",)
MEDIA_PLAYBACK_KIND = "audio"
MEDIA_PLAYBACK_REQUIRED_EVENTS = ("loadedmetadata", "canplay", "playing", "pause")
MAX_MEDIA_PLAYBACK_TIMEOUT_MILLISECONDS = 10_000


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


MEDIA_PLAYBACK_SCRIPT = r"""
const done = arguments[arguments.length - 1];
const path = arguments[0];
const timeout = arguments[1];
if (typeof path !== "string" || path.length === 0 || path.length > 128 ||
    !Number.isInteger(timeout) || timeout < 1 || timeout > 10000) {
  done({ok: false, error: "invalid media playback arguments"});
  return;
}
const container = document.createElement("div");
container.setAttribute("data-metis-media-playback", "true");
container.setAttribute("aria-hidden", "true");
container.style.cssText = "position:absolute; width:1px; height:1px; overflow:hidden;";
document.body.appendChild(container);
const media = document.createElement("audio");
media.preload = "auto";
media.controls = true;
media.muted = true;
media.loop = true;
container.appendChild(media);
const boundedReason = (value) => String(value || "media playback failed").slice(0, 256);
const finish = async (record) => {
  if (settled) return;
  settled = true;
  try {
    const emptied = media.currentSrc ? waitFor("emptied") : null;
    media.pause();
    media.removeAttribute("src");
    media.load();
    if (emptied) await emptied;
    const teardown = {
      ready_state: media.readyState,
      network_state: media.networkState,
      current_src: media.currentSrc || "",
      src_attribute: media.getAttribute("src"),
    };
    media.remove();
    container.remove();
    const dialog = document.getElementById("session-dialog");
    if (dialog && dialog.open) dialog.close();
    done({
      ok: true,
      playback: {
        ...record,
        teardown,
        attached_after_remove: media.isConnected === true,
      },
      remaining_probe_elements: document.querySelectorAll("[data-metis-media-playback]").length,
    });
  } catch (error) {
    settled = false;
    fail(error && error.message ? error.message : "media teardown failed");
  }
};
const fail = (reason) => {
  if (settled) return;
  settled = true;
  media.pause();
  media.removeAttribute("src");
  media.load();
  media.remove();
  container.remove();
  const dialog = document.getElementById("session-dialog");
  if (dialog && dialog.open) dialog.close();
  done({ok: false, error: boundedReason(reason)});
};
let timer = 0;
let settled = false;
const events = [];
const waitFor = (eventName) => new Promise((resolve, reject) => {
  if (events.includes(eventName)) {
    resolve();
    return;
  }
  const onEvent = () => {
    window.clearTimeout(timer);
    media.removeEventListener(eventName, onEvent);
    resolve();
  };
  media.addEventListener(eventName, onEvent, {once: true});
  timer = window.setTimeout(() => {
    media.removeEventListener(eventName, onEvent);
    reject(new Error(`media ${eventName} deadline exceeded`));
  }, timeout);
});
(async () => {
  let url;
  try {
    url = new URL(path, document.baseURI);
  } catch (error) {
    fail("media playback URL is malformed");
    return;
  }
  if (url.origin !== window.location.origin) {
    fail("media playback URL is cross-origin");
    return;
  }
  media.addEventListener("loadedmetadata", () => events.push("loadedmetadata"), {once: true});
  media.addEventListener("canplay", () => events.push("canplay"), {once: true});
  media.addEventListener("playing", () => events.push("playing"), {once: true});
  media.addEventListener("pause", () => events.push("pause"), {once: true});
  media.addEventListener("emptied", () => events.push("emptied"), {once: true});
  media.addEventListener("error", () => {
    const error = media.error;
    fail(error && error.message ? error.message : "media source error");
  }, {once: true});
  media.src = url.href;
  media.load();
  await waitFor("loadedmetadata");
  if (media.readyState < 3 && !events.includes("canplay")) await waitFor("canplay");
  const before = {
    controls: media.controls === true,
    paused: media.paused === true,
    ready_state: media.readyState,
    network_state: media.networkState,
    duration_seconds: media.duration,
    source: media.currentSrc || "",
  };
  try {
    const play_result = media.play();
    if (play_result && typeof play_result.catch === "function") {
      play_result.catch((error) => {
        if (!events.includes("playing")) fail(error && error.message ? error.message : "media play rejected");
      });
    }
  } catch (error) {
    fail(error && error.message ? error.message : "media play rejected");
    return;
  }
  if (!events.includes("playing")) await waitFor("playing");
  const playing = {paused: media.paused === true, ready_state: media.readyState};
  media.pause();
  if (!events.includes("pause")) await waitFor("pause");
  const after_pause = {paused: media.paused === true, ready_state: media.readyState};
  await finish({
    kind: "audio",
    path,
    source: url.pathname,
    events: events.filter((event) => event !== "emptied"),
    before,
    playing,
    after_pause,
  });
})().catch((error) => {
  if (settled) return;
  settled = true;
  fail(error && error.message ? error.message : error);
});
"""


def _validate_playback(value: Any) -> Dict[str, Any]:
    if not isinstance(value, Mapping) or value.get("ok") is not True:
        detail = value.get("error") if isinstance(value, Mapping) else value
        raise BrowserRuntimeError(f"media playback probe failed: {detail!r}")
    record = value.get("playback")
    if not isinstance(record, Mapping):
        raise BrowserRuntimeError("media playback probe returned a malformed record")
    if record.get("kind") != MEDIA_PLAYBACK_KIND or record.get("path") != MEDIA_PLAYBACK_PATHS[0]:
        raise BrowserRuntimeError("media playback probe returned the wrong fixture")
    source = _bounded_string(record.get("source"), "playback source", MAX_MEDIA_SOURCE_BYTES)
    if source != "/assets/metis-tone.wav":
        raise BrowserRuntimeError("media playback fixture resolved to an unexpected source")
    events = record.get("events")
    if not isinstance(events, list) or tuple(events) != MEDIA_PLAYBACK_REQUIRED_EVENTS:
        raise BrowserRuntimeError(f"media playback event sequence is invalid: {events!r}")
    before = record.get("before")
    playing = record.get("playing")
    after_pause = record.get("after_pause")
    if not isinstance(before, Mapping) or not isinstance(playing, Mapping) or not isinstance(after_pause, Mapping):
        raise BrowserRuntimeError("media playback state records are malformed")
    if before.get("controls") is not True or before.get("paused") is not True:
        raise BrowserRuntimeError("media playback controls did not start paused and enabled")
    if type(before.get("ready_state")) is not int or before["ready_state"] < 2:
        raise BrowserRuntimeError("media playback did not reach a playable ready state")
    duration = before.get("duration_seconds")
    if not isinstance(duration, (int, float)) or isinstance(duration, bool) or not math.isfinite(duration) or duration <= 0:
        raise BrowserRuntimeError("media playback duration is not finite and positive")
    if playing.get("paused") is not False or after_pause.get("paused") is not True:
        raise BrowserRuntimeError("media playback play/pause controls did not change state")
    teardown = record.get("teardown")
    if not isinstance(teardown, Mapping) or teardown.get("ready_state") != 0 or teardown.get("network_state") not in MEDIA_EMPTY_NETWORK_STATES or teardown.get("src_attribute") is not None:
        raise BrowserRuntimeError("media playback retained its source after teardown")
    current_src = _bounded_string(teardown.get("current_src"), "playback teardown source", MAX_MEDIA_SOURCE_BYTES)
    if current_src:
        parsed = urllib.parse.urlsplit(current_src)
        if parsed.scheme not in {"http", "https"} or parsed.path != "/assets/metis-tone.wav":
            raise BrowserRuntimeError("media playback retained an unexpected resolved source")
    if record.get("attached_after_remove") is not False:
        raise BrowserRuntimeError("media playback element remained attached after teardown")
    remaining = value.get("remaining_probe_elements")
    if type(remaining) is not int or remaining != 0:
        raise BrowserRuntimeError("media playback probe elements were not released")
    return {
        "kind": MEDIA_PLAYBACK_KIND,
        "path": MEDIA_PLAYBACK_PATHS[0],
        "source": source,
        "events": list(MEDIA_PLAYBACK_REQUIRED_EVENTS),
        "before": {
            "controls": True,
            "paused": True,
            "ready_state": before["ready_state"],
            "network_state": before.get("network_state"),
            "duration_seconds": duration,
            "source": source,
        },
        "playing": {"paused": False, "ready_state": playing.get("ready_state")},
        "after_pause": {"paused": True, "ready_state": after_pause.get("ready_state")},
        "teardown": {"ready_state": 0, "network_state": teardown.get("network_state"), "current_src": current_src, "src_attribute": None},
        "attached_after_remove": False,
    }


def capture_media_playback(
    client: WebDriverClient,
    trace: Any,
    label: str,
    *,
    timeout_ms: int = 5_000,
) -> Dict[str, Any]:
    """Exercise a same-origin audio controls cycle and release its source."""
    label = _bounded_text(label, "media playback label", MAX_MEDIA_REASON_BYTES)
    if not label:
        raise BrowserRuntimeError("media playback label is empty")
    if type(timeout_ms) is not int or not 1 <= timeout_ms <= MAX_MEDIA_PLAYBACK_TIMEOUT_MILLISECONDS:
        raise BrowserRuntimeError("media playback timeout is outside its bound")
    client.click(client.find("#open-session-dialog"))
    measurement = {
        "label": label,
        **_validate_playback(client.execute_async(MEDIA_PLAYBACK_SCRIPT, [MEDIA_PLAYBACK_PATHS[0], timeout_ms])),
    }
    trace.metrics.setdefault("media_playback", []).append(measurement)
    return measurement
