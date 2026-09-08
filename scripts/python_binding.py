"""Build and exercise the Metis Python wheel in an isolated directory."""

from __future__ import annotations

import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import zipfile


ROOT = pathlib.Path(__file__).resolve().parents[1]
PACKAGE = ROOT / "crates" / "metis-python"
TESTS = PACKAGE / "tests"


def run(command: list[str], *, environment: dict[str, str]) -> None:
    """Run one bounded binding build or test command."""
    completed = subprocess.run(
        command,
        cwd=ROOT,
        env=environment,
        check=False,
        text=True,
    )
    if completed.returncode != 0:
        raise SystemExit(completed.returncode)


def extract_wheel(wheel: pathlib.Path, destination: pathlib.Path) -> None:
    """Extract a generated wheel only when every member stays in destination."""
    destination = destination.resolve()
    with zipfile.ZipFile(wheel) as archive:
        for member in archive.infolist():
            target = (destination / member.filename).resolve()
            if not target.is_relative_to(destination):
                raise SystemExit(f"wheel member escapes extraction root: {member.filename}")
        archive.extractall(destination)


def main() -> None:
    """Build one locked wheel and run its provider-owned pytest suite."""
    maturin = shutil.which("maturin")
    if maturin is None:
        raise SystemExit("maturin is required; install the pinned gate tool before verification")
    pytest = [sys.executable, "-m", "pytest", "--import-mode=importlib", "--disable-warnings", "--maxfail=1"]
    environment = os.environ.copy()
    environment["CARGO_NET_OFFLINE"] = "true"
    with tempfile.TemporaryDirectory(prefix="metis-python-wheel-") as temporary:
        root = pathlib.Path(temporary)
        distribution = root / "dist"
        extraction = root / "site"
        distribution.mkdir()
        extraction.mkdir()
        run(
            [
                maturin,
                "build",
                "--release",
                "--locked",
                "--manifest-path",
                str(PACKAGE / "Cargo.toml"),
                "--out",
                str(distribution),
            ],
            environment=environment,
        )
        wheels = sorted(distribution.glob("*.whl"))
        if len(wheels) != 1:
            raise SystemExit(f"expected one Metis wheel, found {len(wheels)}")
        extract_wheel(wheels[0], extraction)
        environment["PYTHONPATH"] = os.pathsep.join(
            filter(None, (str(extraction), environment.get("PYTHONPATH", "")))
        )
        run([*pytest, str(TESTS)], environment=environment)


if __name__ == "__main__":
    main()
