"""Validate bounded browser font metrics and fallback-chain observations."""
from __future__ import annotations

import math
from typing import Any, Dict, Mapping

from browser_protocol import BrowserRuntimeError


MAX_TEXT_STYLE_BYTES = 512
MAX_FONT_METRIC_PIXELS = 4_096.0
FONT_FALLBACK_FAMILIES = ("system-ui", "sans-serif", "monospace")
MAX_FONT_FALLBACK_SAMPLES = len(FONT_FALLBACK_FAMILIES)


def bounded_style(value: Any, name: str) -> str:
    """Validate a computed font string without retaining unbounded text."""
    if not isinstance(value, str) or not value or len(value.encode("utf-8")) > MAX_TEXT_STYLE_BYTES:
        raise BrowserRuntimeError(f"text geometry {name} is invalid")
    return value


def bounded_font_metric(value: Any, name: str) -> float:
    """Validate one finite, bounded Canvas or fallback-font measurement."""
    if (
        type(value) not in (int, float)
        or not math.isfinite(float(value))
        or float(value) < 0.0
        or float(value) > MAX_FONT_METRIC_PIXELS
    ):
        raise BrowserRuntimeError(f"text geometry {name} is invalid")
    return float(value)


def normalize_font_fallback(value: Any) -> Dict[str, Any]:
    """Validate a CSS fallback-family chain and its bounded host measurements."""
    if not isinstance(value, Mapping) or value.get("available") is not True or value.get("source") != "CSS font-family fallback":
        raise BrowserRuntimeError("text geometry font fallback source is invalid")
    requested_families = value.get("requested_families")
    if not isinstance(requested_families, list) or tuple(requested_families) != FONT_FALLBACK_FAMILIES:
        raise BrowserRuntimeError("text geometry font fallback families are invalid")
    computed_family = bounded_style(value.get("computed_family"), "font fallback computed family")
    samples = value.get("samples")
    if not isinstance(samples, list) or len(samples) != MAX_FONT_FALLBACK_SAMPLES:
        raise BrowserRuntimeError("text geometry font fallback samples are invalid")
    normalized_samples = []
    for index, sample in enumerate(samples):
        if not isinstance(sample, Mapping) or sample.get("family") != FONT_FALLBACK_FAMILIES[index]:
            raise BrowserRuntimeError("text geometry font fallback family order is invalid")
        sample_computed = bounded_style(sample.get("computed_family"), f"font fallback {index} computed family")
        width = bounded_font_metric(sample.get("width"), f"font fallback {index} width")
        height = bounded_font_metric(sample.get("height"), f"font fallback {index} height")
        if width <= 0.0 or height <= 0.0:
            raise BrowserRuntimeError(f"text geometry font fallback {index} bounds are empty")
        sample_status = bounded_style(sample.get("fonts_status"), f"font fallback {index} status")
        sample_check = sample.get("fonts_check")
        if sample_check is not None and type(sample_check) is not bool:
            raise BrowserRuntimeError(f"text geometry font fallback {index} check is invalid")
        normalized_samples.append(
            {
                "family": FONT_FALLBACK_FAMILIES[index],
                "computed_family": sample_computed,
                "width": width,
                "height": height,
                "fonts_status": sample_status,
                "fonts_check": sample_check,
            }
        )
    return {
        "available": True,
        "source": "CSS font-family fallback",
        "requested_families": list(FONT_FALLBACK_FAMILIES),
        "computed_family": computed_family,
        "samples": normalized_samples,
    }
