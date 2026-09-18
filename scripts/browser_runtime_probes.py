"""Dispatch the optional browser runtime probes for one lifecycle observation."""
from __future__ import annotations

from typing import Any

from browser_assets import capture_assets
from browser_font import capture_font
from browser_media import capture_media, capture_media_playback
from browser_protocol import WebDriverClient
from browser_text_geometry import capture_text_geometry
from browser_trace import Trace


def capture_runtime_features(
    client: WebDriverClient,
    trace: Trace,
    label: str,
    *,
    asset_probe: bool,
    media_probe: bool,
    media_playback_probe: bool,
    font_probe: bool,
    text_geometry_probe: bool,
) -> None:
    """Run the enabled asset, media and text probes for one lifecycle label."""
    if asset_probe:
        capture_assets(client, trace, label)
    if media_probe:
        capture_media(client, trace, label)
    if media_playback_probe:
        capture_media_playback(client, trace, label)
    if font_probe:
        capture_font(client, trace, label)
    if text_geometry_probe:
        capture_text_geometry(client, trace, label)
