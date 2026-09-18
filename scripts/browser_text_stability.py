"""Validate browser text-layout invariants across lifecycle observations."""
from __future__ import annotations

from collections.abc import Mapping
from typing import Any, Dict

from browser_protocol import BrowserRuntimeError


MAX_TEXT_GEOMETRY_OBSERVATIONS = 8


def _contract(measurement: Mapping[str, Any]) -> Dict[str, Any]:
    """Extract the layout identity that must survive a stop/remount cycle."""
    style = measurement.get("style")
    font_metrics = measurement.get("font_metrics")
    fallback = measurement.get("font_fallback")
    if not isinstance(style, Mapping) or not isinstance(font_metrics, Mapping) or not isinstance(fallback, Mapping):
        raise BrowserRuntimeError("text geometry stability contract is malformed")
    samples = font_metrics.get("samples")
    fallback_samples = fallback.get("samples")
    if not isinstance(samples, list) or not isinstance(fallback_samples, list):
        raise BrowserRuntimeError("text geometry stability font samples are malformed")
    return {
        "fixture": measurement.get("fixture"),
        "utf16_length": measurement.get("utf16_length"),
        "grapheme_boundaries": measurement.get("grapheme_boundaries"),
        "visual_order": measurement.get("visual_order"),
        "line_count": measurement.get("line_count"),
        "style": {
            "font_family": style.get("font_family"),
            "font_size_px": style.get("font_size_px"),
            "line_height_px": style.get("line_height_px"),
            "direction": style.get("direction"),
            "writing_mode": style.get("writing_mode"),
        },
        "font_metrics": {
            "available": font_metrics.get("available"),
            "source": font_metrics.get("source"),
            "font": font_metrics.get("font"),
            "fonts_status": font_metrics.get("fonts_status"),
            "fonts_check": font_metrics.get("fonts_check"),
            "samples": [
                {"label": sample.get("label"), "text": sample.get("text")}
                for sample in samples
                if isinstance(sample, Mapping)
            ],
        },
        "font_fallback": {
            "available": fallback.get("available"),
            "source": fallback.get("source"),
            "requested_families": fallback.get("requested_families"),
            "computed_family": fallback.get("computed_family"),
            "samples": [
                {
                    "family": sample.get("family"),
                    "computed_family": sample.get("computed_family"),
                    "fonts_status": sample.get("fonts_status"),
                    "fonts_check": sample.get("fonts_check"),
                }
                for sample in fallback_samples
                if isinstance(sample, Mapping)
            ],
        },
    }


def validate_text_geometry_stability(measurements: Any) -> Dict[str, Any]:
    """Require one stable text-layout contract across lifecycle observations."""
    if not isinstance(measurements, list) or not 2 <= len(measurements) <= MAX_TEXT_GEOMETRY_OBSERVATIONS:
        raise BrowserRuntimeError("text geometry stability needs between 2 and 8 observations")
    if any(not isinstance(measurement, Mapping) for measurement in measurements):
        raise BrowserRuntimeError("text geometry stability observations are malformed")
    availability = [measurement.get("available") is True for measurement in measurements]
    if any(measurement.get("available") not in (True, False) for measurement in measurements):
        raise BrowserRuntimeError("text geometry stability availability is invalid")
    if len(set(availability)) != 1:
        raise BrowserRuntimeError("text geometry availability changed across lifecycle observations")
    if not availability[0]:
        reasons = [measurement.get("reason") for measurement in measurements]
        if any(not isinstance(reason, str) or not reason for reason in reasons) or len(set(reasons)) != 1:
            raise BrowserRuntimeError("text geometry unavailable reasons changed across lifecycle observations")
        return {"available": False, "observations": len(measurements), "reason": reasons[0]}

    baseline = _contract(measurements[0])
    for index, measurement in enumerate(measurements[1:], start=2):
        if _contract(measurement) != baseline:
            raise BrowserRuntimeError(f"text geometry contract changed at observation {index}")
    return {
        "available": True,
        "stable": True,
        "observations": len(measurements),
        "grapheme_clusters": len(baseline["grapheme_boundaries"]) - 1,
        "line_count": baseline["line_count"],
        "visual_order": baseline["visual_order"],
        "direction": baseline["style"]["direction"],
        "writing_mode": baseline["style"]["writing_mode"],
        "font_family": baseline["style"]["font_family"],
    }
