"""Record bounded opaque bytes from the original native preparation output."""

import hashlib
import json
import os
from pathlib import Path
import stat

from ..native_paths import contained_file


def _limits(value: dict) -> dict:
    expected = {"schema", "portable_path_bytes", "records", "manifest_bytes",
                "module_bytes", "payload_bytes"}
    if set(value) != expected or value["schema"] != 1:
        raise ValueError("prepared evidence requires the canonical Rust budget schema")
    if any(type(item) is not int or item <= 0 for item in value.values()):
        raise ValueError("prepared evidence requires positive integer Rust budgets")
    return value


def _metadata(path: Path) -> os.stat_result:
    """Read native identity and reparse metadata; directory-entry caches omit identity."""
    try:
        return path.stat(follow_symlinks=False)
    except OSError as error:
        error.add_note(f"prepared evidence stat without following links: {path!r}")
        raise


def _nodes(directory: Path, limits: dict):
    pending, count = [directory], 0
    while pending:
        parent = pending.pop()
        try:
            with os.scandir(parent) as entries:
                for entry in entries:
                    count += 1
                    if count > limits["records"]:
                        raise ValueError("prepared evidence exceeds the Rust record budget")
                    path = parent / entry.name
                    relative = path.relative_to(directory).as_posix()
                    if len(relative.encode("utf-8")) > limits["portable_path_bytes"]:
                        raise ValueError(f"prepared evidence exceeds the Rust path budget: {relative!r}")
                    mode = _metadata(path)
                    if (stat.S_ISLNK(mode.st_mode)
                            or getattr(mode, "st_file_attributes", 0) & stat.FILE_ATTRIBUTE_REPARSE_POINT):
                        raise ValueError(f"prepared evidence rejects linked nodes: {path!r}")
                    if stat.S_ISDIR(mode.st_mode):
                        pending.append(path)
                    elif stat.S_ISREG(mode.st_mode):
                        yield path, relative, mode
                    else:
                        raise ValueError(f"prepared evidence requires unlinked regular nodes: {path!r}")
        except OSError as error:
            error.add_note(f"prepared evidence enumerate directory: {parent!r}")
            raise


def _identity(path: Path, limits: dict) -> tuple[int, str]:
    digest, size = hashlib.sha256(), 0
    try:
        with path.open("rb") as source:
            while block := source.read(min(limits["manifest_bytes"], limits["module_bytes"])):
                size += len(block)
                if size > limits["module_bytes"]:
                    raise ValueError(f"prepared evidence exceeds the Rust module budget: {path!r}")
                digest.update(block)
    except OSError as error:
        error.add_note(f"prepared evidence read file: {path!r}")
        raise
    return size, digest.hexdigest()


def record(inventory: Path, directory: Path, budgets: dict) -> Path:
    """Retain a receipt beside the original producer tree without changing it."""
    limits = _limits(budgets)
    admitted = contained_file(inventory, directory)
    try:
        root = directory.resolve(strict=True)
    except OSError as error:
        error.add_note(f"prepared evidence resolve producer directory: {directory!r}")
        raise
    inventory_stat = _metadata(admitted)
    files, matches, total = [], [], 0
    for path, relative, native in _nodes(root, limits):
        size, digest = _identity(path, limits)
        total += size
        if total > limits["payload_bytes"]:
            raise ValueError("prepared evidence exceeds the Rust payload budget")
        entry = {"path": relative, "bytes": size, "sha256": digest}
        files.append(entry)
        if os.path.samestat(native, inventory_stat):
            matches.append(entry)
    if len(matches) != 1:
        raise ValueError("prepared evidence requires one native inventory identity")
    receipt = {"schema": 1, "kind": "original-native-preparation",
               "directory": str(directory), "reported_inventory": str(inventory),
               "inventory": matches[0], "files": sorted(files, key=lambda item: item["path"])}
    encoded = (json.dumps(receipt, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
    if len(encoded) > limits["manifest_bytes"]:
        raise ValueError("prepared evidence exceeds the Rust manifest budget")
    destination = directory.parent / f"{directory.name}.json"
    try:
        with destination.open("xb") as output:
            output.write(encoded)
    except OSError as error:
        error.add_note(f"prepared evidence create receipt: {destination!r}")
        raise
    return destination
