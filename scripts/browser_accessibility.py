"""Bounded runtime probes for the browser accessibility presentation."""
from __future__ import annotations

import math
from typing import Any, Dict, Mapping, Optional

from browser_protocol import BrowserRuntimeError, WebDriverClient, _bounded_text


MAX_ACCESSIBILITY_ELEMENTS = 64
MAX_SEMANTIC_ELEMENTS = 96
MAX_ACCESSIBILITY_TEXT_BYTES = 256
MAX_ACCESSIBILITY_DIMENSION = 65_536


ACCESSIBILITY_SCRIPT = r"""
const root = document.getElementById("metis-app");
if (!root) return {ok: false, error: "metis-app is absent"};
const media = {
  reduced_motion: window.matchMedia("(prefers-reduced-motion: reduce)").matches,
  forced_colors: window.matchMedia("(forced-colors: active)").matches,
  contrast_more: window.matchMedia("(prefers-contrast: more)").matches,
};
const viewport = {
  width: window.innerWidth,
  height: window.innerHeight,
  device_pixel_ratio: window.devicePixelRatio,
};
const visual = window.visualViewport;
const visual_viewport = visual ? {
  scale: visual.scale,
  width: visual.width,
  height: visual.height,
} : null;
const document_element = document.documentElement;
const is_visible = (element) => {
  const style = window.getComputedStyle(element);
  return style.display !== "none" &&
    style.visibility !== "hidden" &&
    !element.closest("[hidden], [inert], [aria-hidden=\"true\"], dialog:not([open])");
};
const focus_candidates = Array.from(root.querySelectorAll(
  "a[href],button,input,select,textarea,[tabindex]"
)).filter((element) =>
  element.id &&
  element.getAttribute("tabindex") !== "-1" &&
  element.matches(":disabled") !== true &&
  is_visible(element)
);
if (focus_candidates.length === 0) {
  return {ok: false, error: "no visible enabled focusable controls"};
}
if (focus_candidates.length > 64) {
  return {ok: false, error: "focusable control count exceeds 64"};
}
const focusable = focus_candidates;
const accessible_name = (element) => {
  const aria = element.getAttribute("aria-label");
  if (aria) return aria.trim().slice(0, 256);
  const labelledby = element.getAttribute("aria-labelledby");
  if (labelledby) {
    return labelledby.split(/\s+/).map((id) => {
      const label = document.getElementById(id);
      return label ? (label.textContent || "") : "";
    }).join(" ").trim().slice(0, 256);
  }
  const label = element.labels && element.labels[0];
  if (label) return (label.textContent || "").trim().slice(0, 256);
  return element.id;
};
const focus_order = focusable.map((element) => ({
  id: element.id,
  role: element.getAttribute("role") || element.tagName.toLowerCase(),
  name: accessible_name(element),
}));
const active_before = document.activeElement && document.activeElement.id
  ? document.activeElement.id : null;
const focus_sequence = [];
for (const element of focusable) {
  try { element.focus({preventScroll: true}); }
  catch (_) { element.focus(); }
  focus_sequence.push(document.activeElement && document.activeElement.id
    ? document.activeElement.id : null);
}
if (document.activeElement && typeof document.activeElement.blur === "function") {
  document.activeElement.blur();
}
const geometry_elements = [root, ...focusable];
const geometry = Object.fromEntries(geometry_elements.map((element) => {
  const rect = element.getBoundingClientRect();
  return [element.id, {
    left: rect.left,
    top: rect.top,
    width: rect.width,
    height: rect.height,
    right: rect.right,
    bottom: rect.bottom,
  }];
}));
const semantic_candidates = [root, ...Array.from(root.querySelectorAll(
  "main,header,nav,form,fieldset,section,dialog,table,[role],[aria-live],[aria-busy],button,input,select,textarea"
))].filter((element) =>
  element.id && !element.closest("[hidden], [aria-hidden=\"true\"]")
);
if (semantic_candidates.length === 0 || semantic_candidates.length > 96) {
  return {ok: false, error: "semantic element count is outside its bound"};
}
const semantic = semantic_candidates.map((element) => ({
  id: element.id,
  role: element.getAttribute("role") || element.tagName.toLowerCase(),
  name: accessible_name(element),
  states: {
    disabled: element.matches(":disabled"),
    aria_busy: element.getAttribute("aria-busy"),
    aria_live: element.getAttribute("aria-live"),
    aria_atomic: element.getAttribute("aria-atomic"),
    aria_expanded: element.getAttribute("aria-expanded"),
    aria_haspopup: element.getAttribute("aria-haspopup"),
    open: element.tagName.toLowerCase() === "dialog" ? element.open === true : null,
  },
}));
return {
  ok: true,
  media,
  viewport,
  visual_viewport,
  document: {
    client_width: document_element.clientWidth,
    scroll_width: document_element.scrollWidth,
    client_height: document_element.clientHeight,
    scroll_height: document_element.scrollHeight,
  },
  active_before,
  active_after: document.activeElement && document.activeElement.id
    ? document.activeElement.id : null,
  focus_order,
  focus_sequence,
  geometry,
  semantics: semantic,
};
"""


def _bounded_string(
    value: Any,
    label: str,
    limit: int = MAX_ACCESSIBILITY_TEXT_BYTES,
    *,
    required: bool = True,
) -> Optional[str]:
    if not isinstance(value, str):
        if required:
            raise BrowserRuntimeError(f"accessibility {label} is not text")
        return None
    text = _bounded_text(value, f"accessibility {label}", limit)
    if required and not text:
        raise BrowserRuntimeError(f"accessibility {label} is empty")
    return text


def _bounded_integer(value: Any, label: str, *, minimum: int = 0) -> int:
    if type(value) is not int or not minimum <= value <= MAX_ACCESSIBILITY_DIMENSION:
        raise BrowserRuntimeError(f"accessibility {label} is outside its bound")
    return value


def _finite_number(value: Any, label: str, *, minimum: float = 0.0, maximum: float = 16.0) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise BrowserRuntimeError(f"accessibility {label} is not numeric")
    number = float(value)
    if not math.isfinite(number) or not minimum <= number <= maximum:
        raise BrowserRuntimeError(f"accessibility {label} is outside its bound")
    return number


def _validate_rect(value: Any, element_id: str, viewport_width: int) -> Dict[str, float]:
    if not isinstance(value, Mapping):
        raise BrowserRuntimeError(f"accessibility geometry for {element_id!r} is not an object")
    rect = {
        name: _finite_number(value.get(name), f"geometry {element_id}.{name}", minimum=-MAX_ACCESSIBILITY_DIMENSION, maximum=MAX_ACCESSIBILITY_DIMENSION)
        for name in ("left", "top", "width", "height", "right", "bottom")
    }
    if rect["width"] <= 0.0 or rect["height"] <= 0.0:
        raise BrowserRuntimeError(f"accessibility geometry for {element_id!r} is empty")
    if rect["left"] < -1.0 or rect["right"] > viewport_width + 1.0:
        raise BrowserRuntimeError(f"accessibility geometry for {element_id!r} overflows the viewport")
    return rect


def _validate_snapshot(value: Any) -> Dict[str, Any]:
    if not isinstance(value, Mapping) or value.get("ok") is not True:
        detail = value.get("error") if isinstance(value, Mapping) else value
        raise BrowserRuntimeError(f"accessibility probe failed: {detail!r}")
    media = value.get("media")
    if not isinstance(media, Mapping) or any(type(media.get(name)) is not bool for name in ("reduced_motion", "forced_colors", "contrast_more")):
        raise BrowserRuntimeError("accessibility media preferences are malformed")
    viewport = value.get("viewport")
    if not isinstance(viewport, Mapping):
        raise BrowserRuntimeError("accessibility viewport is not an object")
    width = _bounded_integer(viewport.get("width"), "viewport width", minimum=1)
    height = _bounded_integer(viewport.get("height"), "viewport height", minimum=1)
    ratio = _finite_number(viewport.get("device_pixel_ratio"), "device pixel ratio", minimum=0.1)
    visual_viewport = value.get("visual_viewport")
    visual: Optional[Dict[str, Any]] = None
    if visual_viewport is not None:
        if not isinstance(visual_viewport, Mapping):
            raise BrowserRuntimeError("accessibility visual viewport is malformed")
        visual = {
            "scale": _finite_number(visual_viewport.get("scale"), "visual viewport scale", minimum=0.1),
            "width": _finite_number(visual_viewport.get("width"), "visual viewport width", minimum=1.0, maximum=MAX_ACCESSIBILITY_DIMENSION),
            "height": _finite_number(visual_viewport.get("height"), "visual viewport height", minimum=1.0, maximum=MAX_ACCESSIBILITY_DIMENSION),
        }
    document = value.get("document")
    if not isinstance(document, Mapping):
        raise BrowserRuntimeError("accessibility document geometry is not an object")
    document_metrics = {
        name: _bounded_integer(document.get(name), f"document {name}", minimum=1)
        for name in ("client_width", "scroll_width", "client_height", "scroll_height")
    }
    if document_metrics["scroll_width"] > document_metrics["client_width"] + 1:
        raise BrowserRuntimeError("accessibility document has horizontal overflow")
    focus_order = value.get("focus_order")
    if not isinstance(focus_order, list) or not 1 <= len(focus_order) <= MAX_ACCESSIBILITY_ELEMENTS:
        raise BrowserRuntimeError("accessibility focus order is outside its bound")
    focus_items = []
    for item in focus_order:
        if not isinstance(item, Mapping):
            raise BrowserRuntimeError("accessibility focus entry is not an object")
        focus_items.append({
            "id": _bounded_string(item.get("id"), "focus id"),
            "role": _bounded_string(item.get("role"), "focus role"),
            "name": _bounded_string(item.get("name"), "focus name"),
        })
    focus_ids = [item["id"] for item in focus_items]
    if len(set(focus_ids)) != len(focus_ids):
        raise BrowserRuntimeError("accessibility focus order contains duplicate ids")
    focus_sequence = value.get("focus_sequence")
    if not isinstance(focus_sequence, list) or len(focus_sequence) != len(focus_items):
        raise BrowserRuntimeError("accessibility focus sequence does not match its order")
    sequence = [_bounded_string(item, "focus sequence id") for item in focus_sequence]
    if sequence != focus_ids:
        raise BrowserRuntimeError("accessibility focus sequence diverges from DOM order")
    geometry = value.get("geometry")
    if not isinstance(geometry, Mapping) or set(geometry) != {"metis-app", *focus_ids}:
        raise BrowserRuntimeError("accessibility geometry does not cover the focus order")
    rects = {
        element_id: _validate_rect(rect, element_id, width)
        for element_id, rect in geometry.items()
    }
    active_before = _bounded_string(value.get("active_before"), "active-before", required=False)
    active_after = _bounded_string(value.get("active_after"), "active-after", required=False)
    if active_after is not None:
        raise BrowserRuntimeError("accessibility probe left focus active")
    semantics = value.get("semantics")
    if not isinstance(semantics, list) or not 1 <= len(semantics) <= MAX_SEMANTIC_ELEMENTS:
        raise BrowserRuntimeError("accessibility semantic element count is outside its bound")
    semantic_records = []
    semantic_ids = set()
    for item in semantics:
        if not isinstance(item, Mapping):
            raise BrowserRuntimeError("accessibility semantic entry is not an object")
        semantic_id = _bounded_string(item.get("id"), "semantic id")
        if semantic_id in semantic_ids:
            raise BrowserRuntimeError("accessibility semantic ids are duplicated")
        semantic_ids.add(semantic_id)
        states = item.get("states")
        if not isinstance(states, Mapping):
            raise BrowserRuntimeError(f"accessibility states for {semantic_id!r} are malformed")
        parsed_states = {}
        for name in ("disabled", "open"):
            if type(states.get(name)) not in (bool, type(None)):
                raise BrowserRuntimeError(f"accessibility state {semantic_id}.{name} is malformed")
            parsed_states[name] = states.get(name)
        for name in ("aria_busy", "aria_live", "aria_atomic", "aria_expanded", "aria_haspopup"):
            parsed_states[name] = _bounded_string(states.get(name), f"{semantic_id}.{name}", MAX_ACCESSIBILITY_TEXT_BYTES, required=False)
        semantic_records.append({
            "id": semantic_id,
            "role": _bounded_string(item.get("role"), "semantic role"),
            "name": _bounded_string(item.get("name"), "semantic name"),
            "states": parsed_states,
        })
    required_semantics = {
        "metis-app": "main",
        "metis-form": "form",
        "session-dialog": "dialog",
        "submit-calculation": "button",
        "file-input": "input",
        "text-specimen": "textarea",
        "explorer-table": "table",
    }
    by_id = {item["id"]: item for item in semantic_records}
    for semantic_id, role in required_semantics.items():
        item = by_id.get(semantic_id)
        if item is None or item["role"] != role:
            raise BrowserRuntimeError(f"accessibility semantic contract is missing {semantic_id!r} as {role!r}")
    return {
        "media": dict(media),
        "viewport": {"width": width, "height": height, "device_pixel_ratio": ratio},
        "visual_viewport": visual,
        "document": document_metrics,
        "active_before": active_before,
        "active_after": active_after,
        "focus_order": focus_items,
        "focus_sequence": sequence,
        "geometry": rects,
        "semantics": semantic_records,
    }


def capture_accessibility(
    client: WebDriverClient,
    trace: Any,
    label: str,
    *,
    baseline: Optional[Mapping[str, Any]] = None,
    require_reduced_motion: bool = False,
    require_forced_colors: bool = False,
) -> Dict[str, Any]:
    """Record media preferences, focus order and zoom geometry for one page state."""
    label = _bounded_text(label, "accessibility label", MAX_ACCESSIBILITY_TEXT_BYTES)
    if not label:
        raise BrowserRuntimeError("accessibility label is empty")
    snapshot = _validate_snapshot(client.execute(ACCESSIBILITY_SCRIPT))
    if require_reduced_motion and not snapshot["media"]["reduced_motion"]:
        raise BrowserRuntimeError("browser did not report prefers-reduced-motion: reduce")
    if require_forced_colors and not snapshot["media"]["forced_colors"]:
        raise BrowserRuntimeError("browser did not report forced-colors: active")
    if baseline is not None and snapshot["focus_order"] != baseline.get("focus_order"):
        raise BrowserRuntimeError("accessibility focus order changed after input")
    if baseline is not None and [item["id"] for item in snapshot["semantics"]] != [item["id"] for item in baseline.get("semantics", [])]:
        raise BrowserRuntimeError("accessibility semantic identity changed after input")
    record = {"label": label, **snapshot}
    trace.metrics.setdefault("accessibility", []).append(record)
    return snapshot
