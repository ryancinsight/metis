"""Run the same bounded local checks used for Metis delivery."""
import json
import argparse
import hashlib
import os
import pathlib
import re
import subprocess
import sys
import tomllib
import tempfile
from urllib.parse import unquote, urlsplit

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "output"
OUTPUT.mkdir(exist_ok=True)
LOCK = ROOT / "Cargo.lock"
TOOLCHAIN = tomllib.loads((ROOT / "rust-toolchain.toml").read_text(encoding="utf-8"))["toolchain"]["channel"]


def resolution():
    """Preserve every byte of the standalone lock, rejecting overlay residue."""
    lock = LOCK.read_bytes()
    if "unused" in tomllib.loads(lock.decode("utf-8")).get("patch", {}):
        raise SystemExit("Cargo.lock contains Atlas overlay residue; regenerate standalone before verification")
    return lock


BASELINE = resolution()


def inherited_configuration():
    """Retain ancestor build budgets without importing their source patches."""
    target = None
    profiles = {}
    configs = []

    def leaves(table, prefix=()):
        for key, value in table.items():
            path = (*prefix, key)
            if isinstance(value, dict):
                yield from leaves(value, path)
            else:
                yield path, value

    # Cargo merges ancestor configuration first, with the nearest value winning.
    for directory in reversed((ROOT, *ROOT.parents)):
        candidates = [directory / ".cargo" / name for name in ("config", "config.toml")]
        config = next((path for path in candidates if path.is_file()), None)
        if config is None:
            continue
        configs.append(config)
        content = tomllib.loads(config.read_text(encoding="utf-8"))
        configured = content.get("build", {}).get("target-dir")
        if configured is not None:
            target = (directory / configured).resolve()
        profiles.update(leaves(content.get("profile", {}), ("profile",)))
    if os.environ.get("CARGO_TARGET_DIR"):
        target = pathlib.Path(os.environ["CARGO_TARGET_DIR"]).resolve()
    return target, profiles, configs


def command(arguments, *, resolve=True, tail=()):
    """Select the pinned toolchain and root manifest independently of cwd."""
    return ["rustup", "run", TOOLCHAIN, "cargo", *arguments,
            "--manifest-path", str(ROOT / "Cargo.toml"),
            *(["--locked", "--offline"] if resolve else []), *tail]


def snapshot(update):
    """Require exact renderer output unless snapshot replacement is requested."""
    actual = (OUTPUT / "form.svg").read_bytes()
    expected = ROOT / "docs" / "manual" / "images" / "form.svg"
    if update:
        expected.parent.mkdir(parents=True, exist_ok=True)
        expected.write_bytes(actual)
    elif not expected.is_file() or expected.read_bytes() != actual:
        raise SystemExit("Form snapshot differs; inspect output/form.svg, then use --update-snapshots to accept it")


def manual_links():
    """Check inline Markdown file/image destinations without network requests."""
    for document in sorted((ROOT / "docs" / "manual").rglob("*.md")):
        for match in re.finditer(r"!?\[[^\]\n]*\]\(([^)\n]+)\)", document.read_text(encoding="utf-8")):
            destination = match.group(1).strip().split(maxsplit=1)[0].strip("<>")
            link = urlsplit(destination)
            if link.scheme in ("http", "https", "mailto"):
                continue
            if link.scheme or link.netloc:
                raise SystemExit(f"Unsupported manual link in {document}: {destination}")
            target = (document.parent / unquote(link.path)).resolve() if link.path else document
            if not target.is_relative_to(ROOT) or not target.is_file():
                raise SystemExit(f"Missing or external manual target in {document}: {destination}")


def run(name, args, *, cwd, environment, seconds=300):
    log = OUTPUT / (name + ".log")
    log.write_text("Running: " + " ".join(args) + "\n", encoding="utf-8")
    try:
        result = subprocess.run(args, cwd=cwd, text=True, encoding="utf-8", errors="replace", stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, timeout=seconds, check=False, env=environment)
    except subprocess.TimeoutExpired as error:
        captured = b"".join(value.encode() if isinstance(value, str) else value or b"" for value in (error.stdout, error.stderr))
        log.write_bytes(captured + f"\n{name}: exceeded {seconds}-second budget\n".encode())
        raise SystemExit(f"{name}: exceeded {seconds}-second budget; see {log}") from error
    diagnostic = result.stdout + result.stderr
    (OUTPUT / (name + ".log")).write_text(diagnostic, encoding="utf-8")
    if resolution() != BASELINE:
        raise SystemExit(f"{name}: Cargo changed the locked dependency graph; review and resolve before verification")
    print(f"{name}: exit {result.returncode}", flush=True)
    if result.returncode:
        print(diagnostic[-12000:])
        raise SystemExit(result.returncode)
    return result.stdout

def source_state(metadata, configs):
    inputs = {ROOT / "rust-toolchain.toml", *configs}
    inputs.update((ROOT / "docs" / "manual").rglob("*.md"))
    for package in metadata["packages"]:
        if package["source"] is None:
            manifest = pathlib.Path(package["manifest_path"])
            inputs.add(manifest)
            directory = manifest.parent
            inputs.update(directory.glob("*.rs"))
            inputs.update(directory.glob("*.md"))
            for folder in ("src", "tests", "examples", "scripts", ".config"):
                inputs.update(path for path in (directory / folder).rglob("*") if path.is_file() and "__pycache__" not in path.parts)
    return {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(inputs)}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--update-snapshots", action="store_true",
                        help="Replace the committed form snapshot with this run's renderer output")
    arguments = parser.parse_args()
    target, profiles, configs = inherited_configuration()
    environment = os.environ.copy()
    if any(environment.get(name) for name in ("RUSTC", "RUSTDOC")):
        raise SystemExit("Unset RUSTC/RUSTDOC overrides; verification requires the pinned toolchain")
    environment["RUSTUP_TOOLCHAIN"] = TOOLCHAIN
    environment["RUSTDOCFLAGS"] = environment.get("RUSTDOCFLAGS", "") + " -D warnings"
    if target is not None:
        # Outside-overlay exception: carry the exact inherited target directory,
        # as Atlas's canonical lock runner does; this does not fork the cache.
        environment["CARGO_TARGET_DIR"] = str(target)
    with tempfile.TemporaryDirectory(prefix="metis-verification-") as neutral:
        cargo_directory = pathlib.Path(neutral)
        if any(directory == cargo_directory or directory in cargo_directory.parents for directory in ROOT.parents if (directory / ".cargo").is_dir()):
            raise SystemExit("Temporary directory inherits stack Cargo configuration; choose an external TMPDIR")
        config = cargo_directory / ".cargo" / "config.toml"
        config.parent.mkdir()
        config.write_text("\n".join(
            ".".join(json.dumps(key) for key in path) + " = " + json.dumps(value)
            for path, value in sorted(profiles.items())
        ) + "\n", encoding="utf-8")

        def execute(name, args, seconds=300, *, cwd=cargo_directory):
            return run(name, args, cwd=cwd, environment=environment, seconds=seconds)

        def cargo(name, args, *, resolve=True, tail=()):
            return execute(name, command(args, resolve=resolve, tail=tail))

        host = next(line.split(": ", 1)[1] for line in execute(
            "compiler", ["rustup", "run", TOOLCHAIN, "rustc", "-vV"]
        ).splitlines() if line.startswith("host: "))
        metadata = json.loads(cargo("metadata", ["metadata", "--format-version", "1", "--filter-platform", host]))
        for package in metadata["packages"]:
            if package["source"] is None and package["id"] not in metadata["workspace_members"]:
                raise SystemExit(f"Standalone resolution contains a non-workspace path package: {package['name']}")
        if target is not None and pathlib.Path(metadata["target_directory"]).resolve() != target:
            raise SystemExit("Cargo target directory differs from the inherited shared target")
        source = source_state(metadata, configs)
        external = sorted({p["name"] for p in metadata["packages"] if (p.get("source") or "").startswith("registry+")})
        for package in metadata["packages"]:
            if package["name"].startswith("metis"):
                for dependency in package["dependencies"]:
                    provider = dependency.get("source") or ""
                    if provider and not provider.startswith("git+https://github.com/ryancinsight/"):
                        raise SystemExit(f"Non-Atlas direct dependency: {dependency}")
        (OUTPUT / "provider-dependencies.json").write_text(json.dumps(external, indent=2), encoding="utf-8")
        packages = {p["id"]: p for p in metadata["packages"]}
        nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
        frontend = next(p["id"] for p in metadata["packages"] if p["name"] == "metis-frontend")
        pending, reached = [frontend], set()
        while pending:
            package_id = pending.pop()
            if package_id in reached:
                continue
            reached.add(package_id)
            if packages[package_id]["name"] == "metis-backend":
                raise SystemExit("Frontend dependency closure includes backend authority")
            pending.extend(nodes[package_id]["dependencies"])
        # This exact installed runner applies the committed 30/60-second budgets.
        version = execute("nextest-version", ["rustup", "run", TOOLCHAIN, "cargo", "nextest", "--version"])
        if not version.startswith("cargo-nextest 0.9.143 "):
            raise SystemExit("Install pinned cargo-nextest 0.9.143 before running this gate")
        cargo("format", ["fmt", "--all", "--check"], resolve=False)
        cargo("clippy", ["clippy", "--workspace", "--all-targets"], tail=["--", "-D", "warnings"])
        cargo("build", ["build", "--workspace", "--bins", "--examples"])
        cargo("tests", ["nextest", "run", "--workspace", "--profile", "ci"])
        cargo("release-build", ["build", "--workspace", "--bins", "--release"])
        cargo("release-tests", ["nextest", "run", "--workspace", "--release", "--profile", "ci"])
        cargo("doctests", ["test", "--workspace", "--doc"])
        cargo("docs", ["doc", "--workspace", "--no-deps"])
        example = pathlib.Path(metadata["target_directory"]) / "debug" / "examples" / ("clinical_infusion_workflow" + (".exe" if sys.platform == "win32" else ""))
        execute("example", [str(example)], seconds=60, cwd=ROOT)
        execute("presentation", [str(example.with_name("presentation" + (".exe" if sys.platform == "win32" else "")))], seconds=60, cwd=ROOT)
        snapshot(arguments.update_snapshots)
        manual_links()
        if source_state(metadata, configs) != source:
            raise SystemExit("Source inputs changed during verification; collect against a stable revision")
        evidence = {"mode": "standalone", "host": host, "sources": source,
                    "lock_sha256": hashlib.sha256(BASELINE).hexdigest(),
                    "snapshot_sha256": hashlib.sha256((OUTPUT / "form.svg").read_bytes()).hexdigest()}
        (OUTPUT / "verification.json").write_text(json.dumps(evidence, sort_keys=True, indent=2), encoding="utf-8")
        print(f"Verified Metis with {len(metadata['packages'])} resolved packages; provider transitive graph recorded", flush=True)


if __name__ == "__main__":
    main()
