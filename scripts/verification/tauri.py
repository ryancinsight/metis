"""Validate the reviewed Tauri native capability inventory and its owners.

For each native pair, comparator admission implies an open or bounded Metis
claim. Restriction is valid exactly when the pinned comparator says none.
Bounded means a real source boundary exists; it never proves complete parity.
"""
from __future__ import annotations

import json
import pathlib
import re

PLATFORMS = {"windows", "linux", "macos", "android", "ios"}
FEATURES = {
    "autostart", "barcode-scanner", "biometric", "cli", "clipboard-manager",
    "deep-link", "dialog", "fs", "geolocation", "global-shortcut", "haptics",
    "http", "localhost", "log", "nfc", "notification", "opener", "os",
    "persisted-scope", "positioner", "process", "shell", "single-instance",
    "sql", "store", "stronghold", "updater", "upload", "websocket",
    "window-state", "system-tray", "window-customization",
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def validate(data: dict, root: pathlib.Path) -> dict[str, int]:
    require(data.get("schema") == 1, "Unknown inventory schema")
    require(set(data.get("platforms", [])) == PLATFORMS, "Incomplete native platform inventory")
    for name, repository in [
        ("plugin_source", "https://github.com/tauri-apps/plugins-workspace"),
        ("documentation_source", "https://github.com/tauri-apps/tauri-docs"),
    ]:
        source = data.get(name, {})
        require(source.get("repository") == repository, f"Invalid {name} repository")
        require(re.fullmatch(r"[0-9a-f]{40}", source.get("revision", "")) is not None,
                f"Unpinned {name}")
    rows = data.get("features", [])
    require(len(rows) == len(FEATURES) and {row.get("name") for row in rows} == FEATURES,
            "Incomplete or duplicate feature inventory")
    board = (root / "backlog.md").read_text(encoding="utf-8")
    result = {"features": len(rows), "admitted_pairs": 0, "bounded_pairs": 0,
              "open_pairs": 0, "restricted_pairs": 0}
    for row in rows:
        name = row["name"]
        require(set(row.get("tauri", {})) == PLATFORMS and
                set(row.get("metis", {})) == PLATFORMS, f"Incomplete platforms: {name}")
        owner = row.get("owner", "")
        require(bool(owner) and f'id="{owner}"' in board, f"Missing live owner: {name}")
        require(isinstance(row.get("remaining"), str) and bool(row["remaining"].strip()),
                f"Missing remaining acceptance: {name}")
        boundary = row.get("boundary")
        if boundary is not None:
            path = pathlib.PurePosixPath(boundary)
            require(not path.is_absolute() and ".." not in path.parts and
                    path.suffix == ".rs" and (root / path).is_file(),
                    f"Missing Rust boundary: {name}")
        for platform in PLATFORMS:
            admitted = row["tauri"][platform]
            claim = row["metis"][platform]
            require(admitted in {"full", "partial", "none"}, f"Invalid comparator claim: {name}")
            require(claim in {"bounded", "open", "restriction"}, f"Invalid Metis claim: {name}")
            if admitted == "none":
                require(claim == "restriction", f"Comparator restriction lost: {name}/{platform}")
                result["restricted_pairs"] += 1
            else:
                require(claim != "restriction", f"Restriction hides admitted pair: {name}/{platform}")
                require(claim != "bounded" or boundary is not None,
                        f"Bounded claim lacks implementation: {name}/{platform}")
                result["admitted_pairs"] += 1
                result[f"{claim}_pairs"] += 1
    return result


if __name__ == "__main__":
    repository_root = pathlib.Path(__file__).resolve().parents[2]
    inventory = json.loads((repository_root / "docs/conformance/tauri.json").read_text())
    print(json.dumps(validate(inventory, repository_root), sort_keys=True))
