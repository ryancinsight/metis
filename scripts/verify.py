"""Run the same bounded local checks used for Metis delivery."""
import json
import argparse
import hashlib
import os
import pathlib
import subprocess
import sys
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "output"
OUTPUT.mkdir(exist_ok=True)
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--stack", action="store_true", help="Use Atlas local providers; enforce unchanged resolved lock packages while allowing Cargo to reorder unused patches")
STACK = parser.parse_args().stack
LOCK = ROOT / "Cargo.lock"

def resolution():
    """Compare all lock content except order-only unused patch records."""
    lock = tomllib.loads(LOCK.read_text(encoding="utf-8"))
    patch = lock.get("patch", {})
    if "unused" in patch:
        patch["unused"] = sorted(patch["unused"], key=lambda item: json.dumps(item, sort_keys=True))
    return lock

BASELINE = resolution()

def run(name, args, seconds=300):
    environment = os.environ.copy()
    environment["RUSTDOCFLAGS"] = environment.get("RUSTDOCFLAGS", "") + " -D warnings"
    log = OUTPUT / (name + ".log")
    log.write_text("Running: " + " ".join(args) + "\n", encoding="utf-8")
    try:
        result = subprocess.run(args, cwd=ROOT, text=True, stdout=subprocess.PIPE,
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

def cargo(name, args):
    return run(name, ["cargo", *args, *([] if STACK else ["--locked"]), "--offline"])

host = next(line.split(": ", 1)[1] for line in run("compiler", ["rustc", "-vV"]).splitlines() if line.startswith("host: "))
metadata = json.loads(cargo("metadata", ["metadata", "--format-version", "1", "--filter-platform", host]))

def source_state():
    inputs = set()
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

SOURCE = source_state()
external = sorted({p["name"] for p in metadata["packages"] if p["source"] is not None and p["source"].startswith("registry+")})
for package in metadata["packages"]:
    if package["name"].startswith("metis"):
        for dependency in package["dependencies"]:
            source = dependency.get("source") or ""
            if source and not source.startswith("git+https://github.com/ryancinsight/"):
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
# This exact installed runner applies the committed 30/60-second test budgets.
version = run("nextest-version", ["cargo", "nextest", "--version"])
if not version.startswith("cargo-nextest 0.9.143 "):
    raise SystemExit("Install pinned cargo-nextest 0.9.143 before running this gate")
run("format", ["cargo", "fmt", "--all", "--check"])
run("clippy", ["cargo", "clippy", "--workspace", "--all-targets", *([] if STACK else ["--locked"]), "--offline", "--", "-D", "warnings"])
cargo("build", ["build", "--workspace", "--bins", "--examples"])
cargo("tests", ["nextest", "run", "--workspace", "--profile", "ci"])
cargo("release-build", ["build", "--workspace", "--bins", "--release"])
cargo("release-tests", ["nextest", "run", "--workspace", "--release", "--profile", "ci"])
cargo("doctests", ["test", "--workspace", "--doc"])
cargo("docs", ["doc", "--workspace", "--no-deps"])
example = pathlib.Path(metadata["target_directory"]) / "debug" / "examples" / ("clinical_infusion_workflow" + (".exe" if sys.platform == "win32" else ""))
run("example", [str(example)], seconds=60)
run("presentation", [str(example.with_name("presentation" + (".exe" if sys.platform == "win32" else "")))], seconds=60)
if source_state() != SOURCE:
    raise SystemExit("Source inputs changed during verification; collect against a stable revision")
evidence = {"mode": "stack" if STACK else "standalone", "host": host, "sources": SOURCE, "lock": BASELINE}
(OUTPUT / "verification.json").write_text(json.dumps(evidence, sort_keys=True, indent=2), encoding="utf-8")
print(f"Verified Metis with {len(metadata['packages'])} resolved packages; provider transitive graph recorded", flush=True)
