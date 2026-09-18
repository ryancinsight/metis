"""Run Cargo outside an inherited Atlas stack configuration.

Cargo merges configuration from the invocation directory upward, so a member
checked out inside the Atlas stack inherits the stack root's `[patch]` tables.
Those patches resolve first-party crates to local trees, which cannot agree with
the committed `Cargo.lock`, so every `--locked` invocation inside the stack
fails before it compiles anything:

    error: cannot update the lock file Cargo.lock because --locked was passed

The committed lock is the reproducible artifact, so the build must run outside
that configuration chain. `isolated_cargo` yields a directory outside it
together with the environment Cargo needs, while carrying the inherited
`build.target-dir` explicitly so the build still lands in the shared cache the
stack's build-budget policy asks for. The inherited source patches are the only
setting deliberately dropped, and a `--locked` build cannot use them anyway.
"""
from __future__ import annotations

import contextlib
import os
import pathlib
import tempfile
import tomllib
from typing import Iterator


CONFIGURATION_NAMES = ("config", "config.toml")


def configuration_directories(root: pathlib.Path) -> list[pathlib.Path]:
    """Return the ancestor Cargo configuration files that apply to `root`."""
    return [
        directory / ".cargo" / name
        for directory in reversed((root, *root.parents))
        for name in CONFIGURATION_NAMES
        if (directory / ".cargo" / name).is_file()
    ]


def inherited_target_directory(root: pathlib.Path) -> pathlib.Path | None:
    """Resolve the effective shared Cargo target directory, if one is configured."""
    configured = os.environ.get("CARGO_TARGET_DIR")
    if configured:
        return pathlib.Path(configured).resolve()
    target = None
    # Nearest configuration wins, so the last match walking outwards is the one
    # Cargo would have used.
    for config in configuration_directories(root):
        value = tomllib.loads(config.read_text(encoding="utf-8")).get("build", {}).get("target-dir")
        if isinstance(value, str) and value:
            target = (config.parent.parent / value).resolve()
    return target


@contextlib.contextmanager
def isolated_cargo(root: pathlib.Path) -> Iterator[tuple[pathlib.Path, dict[str, str]]]:
    """Yield the working directory and environment for a lock-authoritative build.

    Without an inherited configuration this is an ordinary in-place build. With
    one, the yielded directory sits outside the configuration chain so the
    committed lock stays authoritative. Callers pass `--manifest-path` because
    the workspace is no longer discovered from the working directory.
    """
    environment = os.environ.copy()
    target = inherited_target_directory(root)
    if target is not None:
        environment["CARGO_TARGET_DIR"] = str(target)
    if not configuration_directories(root):
        yield root, environment
        return
    with tempfile.TemporaryDirectory(prefix="metis-neutral-cargo-") as neutral:
        directory = pathlib.Path(neutral)
        # A temporary directory inside the stack would inherit the very
        # configuration this redirect exists to avoid.
        if configuration_directories(directory):
            raise SystemExit(f"temporary directory inherits stack Cargo configuration: {directory}")
        yield directory, environment
