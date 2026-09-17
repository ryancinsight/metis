"""Bounded runtime probes for the browser's local image decoders."""
from __future__ import annotations

from typing import Any, Dict, Mapping

from browser_protocol import BrowserRuntimeError, WebDriverClient, _bounded_text


MAX_ASSET_COUNT = 2
MAX_ASSET_PATH_BYTES = 128
MAX_ASSET_REASON_BYTES = 256
MAX_ASSET_DIMENSION = 4096
MAX_ASSET_TIMEOUT_MILLISECONDS = 10_000
ASSET_PATHS = ("assets/metis-mark.svg", "assets/metis-mark.png")


ASSET_SCRIPT = r"""
const done = arguments[arguments.length - 1];
const paths = arguments[0];
const timeout = arguments[1];
if (!Array.isArray(paths) || paths.length === 0 || paths.length > 2 ||
    !Number.isInteger(timeout) || timeout < 1 || timeout > 10000) {
  done({ok: false, error: "invalid asset probe arguments"});
  return;
}
const container = document.createElement("div");
container.setAttribute("data-metis-asset-probe", "true");
container.setAttribute("aria-hidden", "true");
container.style.cssText = "position:absolute; width:1px; height:1px; overflow:hidden;";
document.body.appendChild(container);
const boundedReason = (value) => String(value || "asset decode failed").slice(0, 256);
const load = (path) => new Promise((resolve) => {
  let settled = false;
  let timer = 0;
  const image = new Image();
  image.decoding = "async";
  container.appendChild(image);
  const finish = (value) => {
    if (settled) return;
    settled = true;
    window.clearTimeout(timer);
    resolve(value);
  };
  let url;
  try {
    url = new URL(path, document.baseURI);
  } catch (error) {
    finish({path, available: false, reason: "asset URL is malformed"});
    return;
  }
  if (url.origin !== window.location.origin) {
    finish({path, available: false, reason: "asset URL is cross-origin"});
    return;
  }
  timer = window.setTimeout(() => finish({
    path,
    available: false,
    reason: "asset decode deadline exceeded",
  }), timeout);
  if (typeof image.decode !== "function") {
    finish({path, available: false, reason: "HTMLImageElement.decode unavailable"});
    return;
  }
  image.src = url.href;
  image.decode().then(() => {
    const current = new URL(image.currentSrc || url.href, document.baseURI);
    if (current.origin !== window.location.origin) {
      finish({path, available: false, reason: "decoded asset became cross-origin"});
      return;
    }
    finish({
      path,
      source: current.pathname,
      available: true,
      decoder: "HTMLImageElement.decode",
      complete: image.complete === true,
      natural_width: image.naturalWidth,
      natural_height: image.naturalHeight,
      same_origin: true,
    });
  }).catch((error) => finish({
    path,
    available: false,
    reason: boundedReason(error && error.message),
  }));
});
(async () => {
  const assets = [];
  for (const path of paths) {
    if (typeof path !== "string" || path.length === 0 || path.length > 128) {
      assets.push({path, available: false, reason: "asset path is invalid"});
      continue;
    }
    assets.push(await load(path));
  }
  container.remove();
  done({
    ok: true,
    assets,
    remaining_probe_elements: document.querySelectorAll("[data-metis-asset-probe]").length,
  });
})().catch((error) => {
  container.remove();
  done({ok: false, error: boundedReason(error && error.message)});
});
"""


def _bounded_string(value: Any, label: str, limit: int) -> str:
    if not isinstance(value, str):
        raise BrowserRuntimeError(f"asset {label} is not text")
    return _bounded_text(value, f"asset {label}", limit)


def _bounded_dimension(value: Any, label: str) -> int:
    if type(value) is not int or not 1 <= value <= MAX_ASSET_DIMENSION:
        raise BrowserRuntimeError(f"asset {label} is outside its bound")
    return value


def _validate_result(value: Any) -> Dict[str, Any]:
    if not isinstance(value, Mapping) or value.get("ok") is not True:
        detail = value.get("error") if isinstance(value, Mapping) else value
        raise BrowserRuntimeError(f"asset probe failed: {detail!r}")
    assets = value.get("assets")
    if not isinstance(assets, list) or len(assets) != MAX_ASSET_COUNT:
        raise BrowserRuntimeError("asset probe returned an unexpected asset count")
    records = []
    for expected_path, record in zip(ASSET_PATHS, assets):
        if not isinstance(record, Mapping):
            raise BrowserRuntimeError("asset probe returned a malformed record")
        path = _bounded_string(record.get("path"), "path", MAX_ASSET_PATH_BYTES)
        if path != expected_path:
            raise BrowserRuntimeError("asset probe returned assets in the wrong order")
        if record.get("available") is not True:
            reason = _bounded_string(record.get("reason"), "unavailable reason", MAX_ASSET_REASON_BYTES)
            raise BrowserRuntimeError(f"asset {path!r} was not decoded: {reason}")
        source = _bounded_string(record.get("source"), "source", MAX_ASSET_PATH_BYTES)
        if source != f"/{expected_path}":
            raise BrowserRuntimeError(f"asset {path!r} decoded from an unexpected source")
        if record.get("decoder") != "HTMLImageElement.decode":
            raise BrowserRuntimeError(f"asset {path!r} used an unexpected decoder")
        if record.get("complete") is not True or record.get("same_origin") is not True:
            raise BrowserRuntimeError(f"asset {path!r} did not complete as a same-origin resource")
        width = _bounded_dimension(record.get("natural_width"), f"{path} width")
        height = _bounded_dimension(record.get("natural_height"), f"{path} height")
        records.append({
            "path": path,
            "source": source,
            "decoder": "HTMLImageElement.decode",
            "same_origin": True,
            "natural_width": width,
            "natural_height": height,
        })
    remaining = value.get("remaining_probe_elements")
    if type(remaining) is not int or remaining != 0:
        raise BrowserRuntimeError("asset probe elements were not released")
    return {"assets": records, "remaining_probe_elements": remaining}


def capture_assets(
    client: WebDriverClient,
    trace: Any,
    label: str,
    *,
    timeout_ms: int = 5_000,
) -> Dict[str, Any]:
    """Decode the shipped local marks and record their intrinsic dimensions."""
    label = _bounded_text(label, "asset label", MAX_ASSET_REASON_BYTES)
    if not label:
        raise BrowserRuntimeError("asset label is empty")
    if type(timeout_ms) is not int or not 1 <= timeout_ms <= MAX_ASSET_TIMEOUT_MILLISECONDS:
        raise BrowserRuntimeError("asset probe timeout is outside its bound")
    measurement = {"label": label, **_validate_result(client.execute_async(ASSET_SCRIPT, [list(ASSET_PATHS), timeout_ms]))}
    trace.metrics.setdefault("assets", []).append(measurement)
    return measurement
