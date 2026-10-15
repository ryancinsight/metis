"""Byte-exact clipboard verification for trusted input journeys.

The session clipboard is read through one bounded PowerShell call after the
journey closes; verification fails closed unless the clipboard carries the
exact expected text.
"""
from __future__ import annotations
import hashlib
import subprocess
from typing import Any


CLIPBOARD_READ_TIMEOUT_SECONDS = 15.0


def read_clipboard_text() -> str:
    """Reads the session clipboard through one bounded PowerShell call.

    PowerShell's console pipeline re-encodes to the OEM code page by default,
    so the child's output encoding is pinned to UTF-8 and the raw bytes are
    decoded strictly here; a mojibake read can never pass verification.
    """
    read = subprocess.run(
        [
            "powershell.exe",
            "-NoProfile",
            "-Command",
            "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; "
            "Get-Clipboard -Raw",
        ],
        capture_output=True,
        timeout=CLIPBOARD_READ_TIMEOUT_SECONDS,
    )
    if read.returncode != 0:
        raise RuntimeError(f"the clipboard read failed: {read.stderr.decode(errors='replace').strip()}")
    return read.stdout.decode("utf-8").rstrip("\r\n")


def verify_expected_clipboard(expected: str | None) -> dict[str, Any] | None:
    """Fails closed unless the clipboard carries the exact expected text."""
    if expected is None:
        return None
    actual = read_clipboard_text()
    record = {
        "reader": "powershell Get-Clipboard -Raw",
        "expected_sha256": hashlib.sha256(expected.encode("utf-8")).hexdigest(),
        "read_sha256": hashlib.sha256(actual.encode("utf-8")).hexdigest(),
        "matches": actual == expected,
    }
    if not record["matches"]:
        raise RuntimeError(
            "the clipboard does not carry the expected text: "
            f"expected {expected!r}, read {actual!r}"
        )
    return record
