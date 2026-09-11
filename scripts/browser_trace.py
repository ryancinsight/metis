"""Shared browser-engine and bounded trace artifacts."""
from __future__ import annotations

import enum
import hashlib
import pathlib
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
