"""Measure bounded browser text layout for the format-neutral workbench."""
from __future__ import annotations

import math
from typing import Any, Dict, Mapping

from browser_protocol import BrowserRuntimeError, _bounded_text
from browser_trace import Trace


MAX_TEXT_GEOMETRY_CLUSTERS = 256
MAX_TEXT_GEOMETRY_FRAGMENTS = 8
MAX_TEXT_GEOMETRY_STRING_BYTES = 512
MAX_TEXT_GEOMETRY_STYLE_BYTES = 512


TEXT_GEOMETRY_SCRIPT = r"""
const textarea = document.getElementById("text-specimen");
if (!textarea) return {available: false, reason: "text specimen is not mounted"};
if (!window.Intl || typeof window.Intl.Segmenter !== "function") {
  return {available: false, reason: "Intl.Segmenter unavailable"};
}
const fixture = "A\u030A 影像 — שלום 👩‍🔬\nline two / 東京";
const segmenter = new Intl.Segmenter("und", {granularity: "grapheme"});
const segments = Array.from(segmenter.segment(fixture));
const boundaries = [0];
for (const segment of segments) {
  boundaries.push(segment.index + segment.segment.length);
}
const finiteRect = (rect) => ({
  left: rect.left,
  top: rect.top,
  width: rect.width,
  height: rect.height,
});
const style = window.getComputedStyle(textarea);
const probe = document.createElement("div");
probe.setAttribute("aria-hidden", "true");
probe.style.position = "fixed";
probe.style.left = "-100000px";
probe.style.top = "0";
probe.style.width = `${Math.max(16, Math.min(4096, textarea.clientWidth))}px`;
probe.style.boxSizing = "border-box";
probe.style.padding = style.padding;
probe.style.border = style.border;
probe.style.fontFamily = style.fontFamily;
probe.style.fontSize = style.fontSize;
probe.style.fontWeight = style.fontWeight;
probe.style.fontStyle = style.fontStyle;
probe.style.fontStretch = style.fontStretch;
probe.style.letterSpacing = style.letterSpacing;
probe.style.lineHeight = style.lineHeight;
probe.style.direction = style.direction;
probe.style.writingMode = style.writingMode;
probe.style.textAlign = style.textAlign;
probe.style.whiteSpace = "pre-wrap";
probe.style.overflowWrap = "anywhere";
probe.style.wordBreak = "normal";
probe.textContent = fixture;
document.body.appendChild(probe);
try {
  const node = probe.firstChild;
  if (!node || node.nodeType !== Node.TEXT_NODE) {
    return {available: false, reason: "text geometry node did not mount"};
  }
  const range = document.createRange();
  range.selectNodeContents(probe);
  const lineRects = Array.from(range.getClientRects()).map(finiteRect);
  const clusterRects = [];
  for (let index = 0; index + 1 < boundaries.length; index += 1) {
    const clusterRange = document.createRange();
    clusterRange.setStart(node, boundaries[index]);
    clusterRange.setEnd(node, boundaries[index + 1]);
    const fragments = Array.from(clusterRange.getClientRects()).map(finiteRect);
    const bounding = finiteRect(clusterRange.getBoundingClientRect());
    clusterRects.push({
      start: boundaries[index],
      end: boundaries[index + 1],
      fragments: fragments.length > 0 ? fragments.slice(0, 8) : [bounding],
    });
  }
  const lineTops = [];
  for (const rect of lineRects) {
    const top = Math.round(rect.top * 2) / 2;
    if (!lineTops.includes(top)) lineTops.push(top);
  }
  lineTops.sort((left, right) => left - right);
  const visualClusters = clusterRects.map((cluster) => {
    const left = cluster.fragments.reduce((value, rect) => Math.min(value, rect.left), Number.POSITIVE_INFINITY);
    const right = cluster.fragments.reduce((value, rect) => Math.max(value, rect.left + rect.width), Number.NEGATIVE_INFINITY);
    const top = cluster.fragments.reduce((value, rect) => Math.min(value, rect.top), Number.POSITIVE_INFINITY);
    const bottom = cluster.fragments.reduce((value, rect) => Math.max(value, rect.top + rect.height), Number.NEGATIVE_INFINITY);
    let lineIndex = 0;
    let lineDistance = Number.POSITIVE_INFINITY;
    lineTops.forEach((lineTop, index) => {
      const distance = Math.abs(lineTop - top);
      if (distance < lineDistance) {
        lineDistance = distance;
        lineIndex = index;
      }
    });
    return {start: cluster.start, end: cluster.end, line_index: lineIndex, left, right, top, bottom};
  });
  const visualOrder = [...visualClusters]
    .sort((left, right) => left.line_index - right.line_index || left.left - right.left || left.start - right.start)
    .map((cluster) => cluster.start);
  const numericPx = (value) => {
    const parsed = Number.parseFloat(value);
    return Number.isFinite(parsed) ? parsed : null;
  };
  return {
    available: true,
    source: "Range.getClientRects",
    fixture,
    utf16_length: fixture.length,
    grapheme_boundaries: boundaries,
    cluster_rects: clusterRects,
    visual_clusters: visualClusters,
    visual_order: visualOrder,
    line_rects: lineRects,
    line_tops: lineTops,
    line_count: lineTops.length,
    element_rect: finiteRect(probe.getBoundingClientRect()),
    style: {
      font_family: style.fontFamily,
      font_size_px: numericPx(style.fontSize),
      line_height_px: numericPx(style.lineHeight),
      direction: style.direction,
      writing_mode: style.writingMode,
    },
    textarea: {
      value_length: textarea.value.length,
      selection_start: textarea.selectionStart,
      selection_end: textarea.selectionEnd,
      client_width: textarea.clientWidth,
      client_height: textarea.clientHeight,
      scroll_height: textarea.scrollHeight,
    },
  };
} finally {
  probe.remove();
}
"""


def _number(value: Any, name: str, *, minimum: float | None = 0.0) -> float:
    """Validate one finite layout number."""
    if (
        type(value) not in (int, float)
        or not math.isfinite(float(value))
        or minimum is not None
        and float(value) < minimum
    ):
        raise BrowserRuntimeError(f"text geometry {name} is invalid")
    return float(value)


def _rect(value: Any, name: str) -> Dict[str, float]:
    """Validate a DOMRect-like mapping with bounded nonnegative dimensions."""
    if not isinstance(value, Mapping):
        raise BrowserRuntimeError(f"text geometry {name} is not an object")
    return {
        "left": _number(value.get("left"), f"{name}.left", minimum=None),
        "top": _number(value.get("top"), f"{name}.top", minimum=None),
        "width": _number(value.get("width"), f"{name}.width"),
        "height": _number(value.get("height"), f"{name}.height"),
    }


def _bounded_style(value: Any, name: str) -> str:
    """Validate a computed style string without preserving unbounded text."""
    if not isinstance(value, str) or not value or len(value.encode("utf-8")) > MAX_TEXT_GEOMETRY_STYLE_BYTES:
        raise BrowserRuntimeError(f"text geometry {name} is invalid")
    return value


def _validate_measurement(value: Any) -> Dict[str, Any]:
    """Validate host geometry while keeping browser-specific metrics explicit."""
    if not isinstance(value, Mapping):
        raise BrowserRuntimeError("text geometry probe returned a non-object")
    if value.get("available") is False:
        reason = value.get("reason")
        if not isinstance(reason, str) or not reason:
            raise BrowserRuntimeError("text geometry unavailable reason is invalid")
        _bounded_text(reason, "text geometry unavailable reason", MAX_TEXT_GEOMETRY_STYLE_BYTES)
        return {"available": False, "reason": reason}
    if value.get("available") is not True or value.get("source") != "Range.getClientRects":
        raise BrowserRuntimeError("text geometry probe returned an invalid source")
    fixture = value.get("fixture")
    if not isinstance(fixture, str) or not fixture or len(fixture.encode("utf-8")) > MAX_TEXT_GEOMETRY_STRING_BYTES:
        raise BrowserRuntimeError("text geometry fixture is invalid")
    utf16_length = value.get("utf16_length")
    if type(utf16_length) is not int or not 1 <= utf16_length <= MAX_TEXT_GEOMETRY_STRING_BYTES:
        raise BrowserRuntimeError("text geometry UTF-16 length is invalid")
    if utf16_length != len(fixture.encode("utf-16-le")) // 2:
        raise BrowserRuntimeError("text geometry UTF-16 length disagrees with fixture")
    boundaries = value.get("grapheme_boundaries")
    if (
        not isinstance(boundaries, list)
        or not 2 <= len(boundaries) <= MAX_TEXT_GEOMETRY_CLUSTERS + 1
        or boundaries[0] != 0
        or boundaries[-1] != utf16_length
        or any(type(boundary) is not int for boundary in boundaries)
        or any(right <= left for left, right in zip(boundaries, boundaries[1:]))
    ):
        raise BrowserRuntimeError("text geometry grapheme boundaries are invalid")
    cluster_rects = value.get("cluster_rects")
    if not isinstance(cluster_rects, list) or len(cluster_rects) != len(boundaries) - 1:
        raise BrowserRuntimeError("text geometry cluster count disagrees with boundaries")
    normalized_clusters = []
    for index, cluster in enumerate(cluster_rects):
        if not isinstance(cluster, Mapping) or cluster.get("start") != boundaries[index] or cluster.get("end") != boundaries[index + 1]:
            raise BrowserRuntimeError("text geometry cluster boundaries are not contiguous")
        fragments = cluster.get("fragments")
        if not isinstance(fragments, list) or not 1 <= len(fragments) <= MAX_TEXT_GEOMETRY_FRAGMENTS:
            raise BrowserRuntimeError("text geometry cluster fragments are invalid")
        normalized_fragments = [_rect(rect, f"cluster {index} fragment") for rect in fragments]
        if any(rect["height"] <= 0.0 for rect in normalized_fragments):
            raise BrowserRuntimeError("text geometry cluster fragment is empty")
        normalized_clusters.append(
            {
                "start": boundaries[index],
                "end": boundaries[index + 1],
                "fragments": normalized_fragments,
            }
        )
    line_rects = value.get("line_rects")
    if not isinstance(line_rects, list) or not 2 <= len(line_rects) <= MAX_TEXT_GEOMETRY_CLUSTERS:
        raise BrowserRuntimeError("text geometry line rectangles are invalid")
    normalized_lines = [_rect(rect, f"line {index}") for index, rect in enumerate(line_rects)]
    if any(rect["height"] <= 0.0 for rect in normalized_lines):
        raise BrowserRuntimeError("text geometry line rectangle is empty")
    line_tops = value.get("line_tops")
    if (
        not isinstance(line_tops, list)
        or not 2 <= len(line_tops) <= len(line_rects)
        or any(type(top) not in (int, float) or not math.isfinite(float(top)) for top in line_tops)
        or any(right <= left for left, right in zip(line_tops, line_tops[1:]))
    ):
        raise BrowserRuntimeError("text geometry line tops are invalid")
    derived_tops = sorted({math.floor(rect["top"] * 2.0 + 0.5) / 2.0 for rect in normalized_lines})
    if [float(top) for top in line_tops] != derived_tops:
        raise BrowserRuntimeError("text geometry line tops disagree with line rectangles")
    line_count = value.get("line_count")
    if type(line_count) is not int or line_count != len(line_tops) or line_count < 2:
        raise BrowserRuntimeError("text geometry line count is invalid")
    visual_clusters = value.get("visual_clusters")
    if not isinstance(visual_clusters, list) or len(visual_clusters) != len(boundaries) - 1:
        raise BrowserRuntimeError("text geometry visual cluster count disagrees with boundaries")
    normalized_visual_clusters = []
    for index, cluster in enumerate(visual_clusters):
        if (
            not isinstance(cluster, Mapping)
            or cluster.get("start") != boundaries[index]
            or cluster.get("end") != boundaries[index + 1]
            or type(cluster.get("line_index")) is not int
            or not 0 <= cluster["line_index"] < line_count
        ):
            raise BrowserRuntimeError("text geometry visual cluster boundaries are invalid")
        left = _number(cluster.get("left"), f"visual cluster {index}.left", minimum=None)
        right = _number(cluster.get("right"), f"visual cluster {index}.right", minimum=None)
        top = _number(cluster.get("top"), f"visual cluster {index}.top", minimum=None)
        bottom = _number(cluster.get("bottom"), f"visual cluster {index}.bottom", minimum=None)
        if right < left or bottom <= top:
            raise BrowserRuntimeError("text geometry visual cluster bounds are invalid")
        normalized_visual_clusters.append(
            {
                "start": boundaries[index],
                "end": boundaries[index + 1],
                "line_index": cluster["line_index"],
                "left": left,
                "right": right,
                "top": top,
                "bottom": bottom,
            }
        )
    visual_order = value.get("visual_order")
    expected_order = boundaries[:-1]
    if (
        not isinstance(visual_order, list)
        or len(visual_order) != len(expected_order)
        or any(type(offset) is not int for offset in visual_order)
        or sorted(visual_order) != expected_order
    ):
        raise BrowserRuntimeError("text geometry visual order is not a cluster permutation")
    element_rect = _rect(value.get("element_rect"), "element")
    if element_rect["width"] <= 0.0 or element_rect["height"] <= 0.0:
        raise BrowserRuntimeError("text geometry element rectangle is empty")
    style = value.get("style")
    if not isinstance(style, Mapping):
        raise BrowserRuntimeError("text geometry style is not an object")
    normalized_style: Dict[str, Any] = {
        "font_family": _bounded_style(style.get("font_family"), "font family"),
        "direction": _bounded_style(style.get("direction"), "direction"),
        "writing_mode": _bounded_style(style.get("writing_mode"), "writing mode"),
    }
    for name in ("font_size_px", "line_height_px"):
        raw = style.get(name)
        normalized_style[name] = None if raw is None else _number(raw, name)
    textarea = value.get("textarea")
    if not isinstance(textarea, Mapping):
        raise BrowserRuntimeError("text geometry textarea metrics are not an object")
    selection_start = textarea.get("selection_start")
    selection_end = textarea.get("selection_end")
    value_length = textarea.get("value_length")
    if (
        type(selection_start) is not int
        or type(selection_end) is not int
        or type(value_length) is not int
        or not 0 <= value_length <= MAX_TEXT_GEOMETRY_STRING_BYTES
        or selection_start < 0
        or selection_end < selection_start
        or selection_end > value_length
        or type(textarea.get("client_width")) is not int
        or type(textarea.get("client_height")) is not int
        or type(textarea.get("scroll_height")) is not int
        or textarea["client_width"] < 1
        or textarea["client_height"] < 1
        or textarea["scroll_height"] < textarea["client_height"]
    ):
        raise BrowserRuntimeError("text geometry textarea metrics are invalid")
    return {
        "available": True,
        "source": "Range.getClientRects",
        "fixture": fixture,
        "utf16_length": utf16_length,
        "grapheme_boundaries": boundaries,
        "cluster_rects": normalized_clusters,
        "visual_clusters": normalized_visual_clusters,
        "visual_order": visual_order,
        "line_rects": normalized_lines,
        "line_tops": [float(top) for top in line_tops],
        "line_count": line_count,
        "element_rect": element_rect,
        "style": normalized_style,
        "textarea": {
            "selection_start": selection_start,
            "selection_end": selection_end,
            "value_length": value_length,
            "client_width": textarea["client_width"],
            "client_height": textarea["client_height"],
            "scroll_height": textarea["scroll_height"],
        },
    }


def capture_text_geometry(client: Any, trace: Trace, label: str) -> Dict[str, Any]:
    """Record one bounded browser Range measurement from the editing specimen."""
    _bounded_text(label, "text geometry label", MAX_TEXT_GEOMETRY_STYLE_BYTES)
    value = client.execute(TEXT_GEOMETRY_SCRIPT)
    measurement = {"label": label, **_validate_measurement(value)}
    trace.metrics.setdefault("text_geometry", []).append(measurement)
    return measurement
