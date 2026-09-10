"""Build and exercise the Metis Python wheel in an isolated directory."""

from __future__ import annotations

import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import zipfile
from email.parser import Parser


PHYSICAL_ROOT = pathlib.Path(__file__).resolve().parents[1]
ROOT = pathlib.Path(os.environ.get("METIS_NEUTRAL_ROOT", PHYSICAL_ROOT))
PACKAGE = ROOT / "crates" / "metis-python"
TESTS = PACKAGE / "tests"


def run(
    command: list[str],
    *,
    environment: dict[str, str],
    cwd: pathlib.Path | None = None,
) -> None:
    """Run one bounded binding build or test command."""
    completed = subprocess.run(
        command,
        cwd=ROOT if cwd is None else cwd,
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


def validate_wheel_surface(wheel: pathlib.Path) -> None:
    """Verify the typed package and stable ABI metadata before extraction."""
    with zipfile.ZipFile(wheel) as archive:
        members = set(archive.namelist())
        required = {
            "metis/__init__.py",
            "metis/_metis.pyi",
            "metis/py.typed",
        }
        missing = sorted(required - members)
        if missing:
            raise SystemExit(f"wheel is missing typed package members: {missing}")
        if any("__pycache__/" in member or member.endswith(".pyc") for member in members):
            raise SystemExit("wheel contains generated Python bytecode")

        extensions = sorted(
            member
            for member in members
            if member.startswith("metis/_metis.") and member not in required
        )
        if len(extensions) != 1:
            raise SystemExit(f"expected one native extension, found {extensions}")

        metadata_members = sorted(
            member for member in members if member.endswith(".dist-info/METADATA")
        )
        if len(metadata_members) != 1:
            raise SystemExit(f"expected one wheel METADATA file, found {metadata_members}")
        metadata = Parser().parsestr(
            archive.read(metadata_members[0]).decode("utf-8")
        )
        if metadata.get("Name") != "metis-rs":
            raise SystemExit(f"wheel metadata has unexpected Name: {metadata.get('Name')!r}")
        if metadata.get("Requires-Python") != ">=3.9":
            raise SystemExit(
                "wheel metadata must retain the declared Python floor >=3.9"
            )
        if "Typing :: Typed" not in metadata.get_all("Classifier", []):
            raise SystemExit("wheel metadata is missing the typed classifier")

        wheel_members = sorted(
            member for member in members if member.endswith(".dist-info/WHEEL")
        )
        if len(wheel_members) != 1:
            raise SystemExit(f"expected one wheel metadata file, found {wheel_members}")
        wheel_metadata = Parser().parsestr(
            archive.read(wheel_members[0]).decode("utf-8")
        )
        tags = wheel_metadata.get_all("Tag", [])
        if not tags or not any("abi3" in tag for tag in tags):
            raise SystemExit(f"wheel metadata has no stable abi3 tag: {tags}")

    if "-abi3-" not in wheel.name:
        raise SystemExit(f"wheel filename has no abi3 tag: {wheel.name}")


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
            cwd=root,
        )
        wheels = sorted(distribution.glob("*.whl"))
        if len(wheels) != 1:
            raise SystemExit(f"expected one Metis wheel, found {len(wheels)}")
        validate_wheel_surface(wheels[0])
        extract_wheel(wheels[0], extraction)
        environment["PYTHONPATH"] = os.pathsep.join(
            filter(None, (str(extraction), environment.get("PYTHONPATH", "")))
        )
        run([*pytest, str(TESTS)], environment=environment, cwd=root)


if __name__ == "__main__":
    main()
