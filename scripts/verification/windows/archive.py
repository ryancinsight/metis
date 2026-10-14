"""Nextest owns compilation/extraction; execution receives no preparation environment."""

from dataclasses import dataclass
import hashlib
import json
from pathlib import Path

from webview_acceptance.fixtures import FIXTURES


@dataclass(frozen=True)
class NativeArchive:
    root: Path
    extracted: Path
    fixture: Path
    identity: dict

    def arguments(self) -> list[str]:
        metadata = self.extracted / "target" / "nextest"
        return ["--cargo-metadata", str(metadata / "cargo-metadata.json"),
                "--binaries-metadata", str(metadata / "binaries-metadata.json"),
                "--workspace-remap", str(self.root),
                "--target-dir-remap", str(self.extracted / "target")]


def create(cargo, execute, root: Path, temporary: Path, output: Path,
           toolchain: str, *, release: bool) -> NativeArchive:
    label = "release" if release else "debug"
    temporary.mkdir(parents=True, exist_ok=True)
    archive = temporary / f"{label}.tar.zst"
    extracted = temporary / label
    cargo("windows-" + label + "-archive", [
        "nextest", "archive", "--workspace", "--profile", "ci", "--archive-file", str(archive),
        *(["--release"] if release else []),
    ])
    # nextest canonicalizes --extract-to before unpacking, so this must be an
    # actual exclusively created empty directory, not a nonexistent pathname.
    extracted.mkdir(exist_ok=False)
    native = ["rustup", "run", toolchain, "cargo", "nextest"]
    listing = execute("windows-" + label + "-archive-list", [
        *native, "list", "--archive-file", str(archive), "--extract-to", str(extracted),
        "--workspace-remap", str(root), "--config-file", str(root / ".config" / "nextest.toml"),
        "--message-format", "json", "--run-ignored", "all",
    ])
    listed = json.loads(listing)
    suites = [suite for suite in listed.get("rust-suites", {}).values()
              if suite.get("package-name") == "metis-app" and suite.get("binary-name") == "metis-app"]
    if len(suites) != 1:
        raise ValueError("actual nextest archive has no unique Metis application test binary")
    fixture = Path(suites[0]["binary-path"]).resolve(strict=True)
    if not fixture.is_file() or not fixture.is_relative_to(extracted.resolve(strict=True)):
        raise ValueError("actual nextest fixture executable is outside its extracted archive")
    tests = suites[0].get("testcases", {})
    if any(tests.get(name, {}).get("ignored") is not True for name in FIXTURES.values()):
        raise ValueError("actual nextest archive omits a required explicit installed-runtime fixture")
    if type(listed.get("test-count")) is not int or listed["test-count"] <= len(FIXTURES):
        raise ValueError("actual nextest archive does not retain the workspace's regular test domains")
    with archive.open("rb") as source:
        archive_digest = hashlib.file_digest(source, "sha256").hexdigest()
    record = {"profile": label, "archive_sha256": archive_digest,
              "test_count": listed["test-count"], "fixture_executable": str(fixture)}
    (output / f"windows-{label}-archive-metadata.json").write_text(listing, encoding="utf-8")
    return NativeArchive(root, extracted, fixture, record)


def run(execute, archive: NativeArchive, toolchain: str, *, release: bool) -> None:
    # Reused metadata makes this a pure test execution command: no Cargo build
    # receives or propagates the inventory variable to the actual test children.
    execute("release-tests" if release else "tests", [
        "rustup", "run", toolchain, "cargo", "nextest", "run", *archive.arguments(),
        # Retain actual native filesystem observations from passing tests too.
        "--profile", "ci", "--no-tests", "fail", "--success-output", "immediate",
        "--config-file", str(archive.root / ".config" / "nextest.toml"),
    ])
