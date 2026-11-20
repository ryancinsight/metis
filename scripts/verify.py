"""Run the same bounded local checks used for Metis delivery."""
import json
import argparse
import contextlib
import hashlib
import os
import pathlib
import shutil
import subprocess
import sys
import stat
import tomllib
import tempfile

from verification import documentation, source, workspace

PHYSICAL_ROOT = pathlib.Path(__file__).resolve().parents[1]
ROOT = PHYSICAL_ROOT
OUTPUT = ROOT / "output"
LOCK = ROOT / "Cargo.lock"
TOOLCHAIN = None
CARGO_DENY_VERSION = "0.20.2"
BUILD_LINK_PACKAGES = {
    "ring": "Moirai cryptographic provider build contract",
    "pyo3": "PyO3 interpreter ABI build contract",
    "pyo3-ffi": "PyO3 interpreter FFI build contract",
    "wasm-bindgen-shared": "wasm-bindgen browser ABI build contract",
}

BASELINE = None
EVIDENCE = {"schema": 1, "status": "running", "stages": {}, "commands": {}}


@contextlib.contextmanager
def neutral_workspace():
    """Expose the workspace without inheriting an ancestor stack overlay."""
    if os.name != "nt":
        yield PHYSICAL_ROOT
        return

    has_ancestor_configuration = any(
        (directory / ".cargo" / name).is_file()
        for directory in (PHYSICAL_ROOT, *PHYSICAL_ROOT.parents)
        for name in ("config", "config.toml")
    )
    if not has_ancestor_configuration:
        yield PHYSICAL_ROOT
        return

    import ctypes

    used_drives = ctypes.windll.kernel32.GetLogicalDrives()
    drive = next(
        (
            f"{chr(code)}:"
            for code in range(ord("Z"), ord("D") - 1, -1)
            if not used_drives & (1 << (code - ord("A")))
        ),
        None,
    )
    if drive is None:
        raise RuntimeError("no unused drive letter is available for neutral verification")

    mapping = subprocess.run(
        ["subst.exe", drive, str(PHYSICAL_ROOT)],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )
    if mapping.returncode != 0:
        diagnostic = (mapping.stderr or mapping.stdout).strip()
        raise RuntimeError(f"cannot create neutral verification drive {drive}: {diagnostic}")

    try:
        yield pathlib.Path(f"{drive}\\")
    finally:
        cleanup = subprocess.run(
            ["subst.exe", drive, "/D"],
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            check=False,
        )
        if cleanup.returncode != 0:
            diagnostic = (cleanup.stderr or cleanup.stdout).strip()
            raise RuntimeError(f"cannot remove neutral verification drive {drive}: {diagnostic}")


def output_path(name):
    """Keep fixed gate artifacts inside the repository, without following links."""
    path = OUTPUT / name
    linked = any(part.is_symlink() or (part.exists() and
                 getattr(part.lstat(), "st_file_attributes", 0) & stat.FILE_ATTRIBUTE_REPARSE_POINT)
                 for part in (OUTPUT, path))
    if (not path.resolve().is_relative_to(PHYSICAL_ROOT)
            or linked
            or (path.is_file() and path.stat().st_nlink != 1)):
        raise ValueError(f"Unsafe gate output path: {path}")
    OUTPUT.mkdir(exist_ok=True)
    return path


def evidence():
    """The current report describes this run, including failure before capture."""
    output_path("verification.json").write_text(json.dumps(EVIDENCE, sort_keys=True, indent=2), encoding="utf-8")


def command(arguments, *, resolve=True, tail=()):
    """Select the pinned toolchain and root manifest independently of cwd."""
    return ["rustup", "run", TOOLCHAIN, "cargo", *arguments,
            "--manifest-path", str(ROOT / "Cargo.toml"),
            *(["--locked", "--offline"] if resolve else []), *tail]


def run(name, args, *, cwd, environment, seconds=300, expected_exit=0, required_diagnostic=None):
    from process_tree import ProcessTreeTimeout, run as run_process_tree

    EVIDENCE["stages"][name] = "running"
    EVIDENCE["commands"][name] = {"args": args, "cwd": str(cwd), "timeout_seconds": seconds,
                                "expected_exit": expected_exit, "required_diagnostic": required_diagnostic}
    evidence()
    log = output_path(name + ".log")
    log.write_text("Running: " + " ".join(args) + "\n", encoding="utf-8")
    try:
        result = run_process_tree(args, cwd=cwd, env=environment, timeout=seconds)
    except ProcessTreeTimeout as error:
        captured = b"".join(value.encode() if isinstance(value, str) else value or b"" for value in (error.stdout, error.stderr))
        cleanup = (
            f"\n{name}: process-tree cleanup failed: {error.cleanup_error}\n".encode()
            if error.cleanup_error else b""
        )
        output_path(name + ".log").write_bytes(
            captured + cleanup + f"\n{name}: exceeded {seconds}-second budget\n".encode()
        )
        raise SystemExit(f"{name}: exceeded {seconds}-second budget; see {log}") from error
    diagnostic = result.stdout + result.stderr
    output_path(name + ".log").write_text(diagnostic, encoding="utf-8")
    if source.resolution(LOCK) != BASELINE:
        raise SystemExit(f"{name}: Cargo changed the locked dependency graph; review and resolve before verification")
    print(f"{name}: exit {result.returncode}", flush=True)
    accepted = result.returncode == expected_exit and (required_diagnostic is None or required_diagnostic in diagnostic)
    EVIDENCE["stages"][name] = "passed" if accepted else "failed"
    evidence()
    if not accepted:
        print(diagnostic[-12000:])
        raise SystemExit(f"{name}: expected exit {expected_exit} and diagnostic {required_diagnostic!r}; got exit {result.returncode}")
    return result.stdout

def run_gate():
    global BASELINE, TOOLCHAIN
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--update-snapshots", action="store_true",
                        help="Replace reviewed visual baselines with this run's validated captures")
    parser.add_argument("--install", action="store_true",
                        help="Also install/run/uninstall the generated per-user MSI in an isolated directory")
    parser.add_argument("--base",
                        help="Limit a pull-request run to package owners and their workspace consumers")
    parser.add_argument("--phase", choices=("all", "native", "release"), default="all",
                        help="Run all checks or one dependency-ordered hosted full-gate phase")
    arguments = parser.parse_args()
    evidence()
    # Import and configuration failures must invalidate the previous success too.
    from visual import begin_run, source_digest
    run_nonce = begin_run(OUTPUT)
    TOOLCHAIN = tomllib.loads((ROOT / "rust-toolchain.toml").read_text(encoding="utf-8"))["toolchain"]["channel"]
    BASELINE = source.resolution(LOCK)
    # Windows consoles may use a legacy code page; never lose the failing-test
    # diagnostic to a UnicodeEncodeError. Full UTF-8 output remains in the log.
    sys.stdout.reconfigure(errors="backslashreplace")
    target, profiles, configs = source.inherited_configuration(PHYSICAL_ROOT, os.environ)
    environment = os.environ.copy()
    if any(environment.get(name) for name in ("RUSTC", "RUSTDOC")):
        raise SystemExit("Unset RUSTC/RUSTDOC overrides; verification requires the pinned toolchain")
    environment["RUSTUP_TOOLCHAIN"] = TOOLCHAIN
    environment["RUSTDOCFLAGS"] = environment.get("RUSTDOCFLAGS", "") + " -D warnings"
    environment["METIS_NEUTRAL_ROOT"] = str(ROOT)
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

        def execute(name, args, seconds=300, *, cwd=cargo_directory, expected_exit=0, required_diagnostic=None):
            return run(name, args, cwd=cwd, environment=environment, seconds=seconds,
                       expected_exit=expected_exit, required_diagnostic=required_diagnostic)

        def cargo(name, args, *, resolve=True, tail=()):
            return execute(name, command(args, resolve=resolve, tail=tail))

        def covers(*names):
            return selected_packages is None or bool(set(names).intersection(selected_packages))

        def skip(*names):
            for name in names:
                EVIDENCE["stages"][name] = "skipped: outside selected gate profile"
            evidence()

        host = next(line.split(": ", 1)[1] for line in execute(
            "compiler", ["rustup", "run", TOOLCHAIN, "rustc", "-vV"]
        ).splitlines() if line.startswith("host: "))
        metadata = json.loads(cargo("metadata", ["metadata", "--format-version", "1", "--filter-platform", host]))
        for package in metadata["packages"]:
            if package["source"] is None and package["id"] not in metadata["workspace_members"]:
                raise SystemExit(f"Standalone resolution contains a non-workspace path package: {package['name']}")
        if target is not None and pathlib.Path(metadata["target_directory"]).resolve() != target:
            raise SystemExit("Cargo target directory differs from the inherited shared target")
        workspace.application_targets(metadata)
        workspace.format_neutral_workspace(metadata)
        native_directory = workspace.native_build_directory(
            metadata, host, environment, explicit_target=sys.platform == "win32"
        )
        source_inputs = source.state(ROOT, metadata, configs)
        revision = execute("revision", ["git", "rev-parse", "HEAD"], seconds=30, cwd=ROOT).strip()
        selected_packages = None
        if arguments.base:
            changed = execute("scope-diff",
                              ["git", "diff", "--name-only", "--no-renames", "-z",
                               f"{arguments.base}...HEAD"], seconds=30, cwd=ROOT).split("\0")
            selected_packages = workspace.pull_request_packages(
                metadata, ROOT, [path for path in changed if path]
            )
        provenance = {"mode": "standalone", "host": host, "sources": source_inputs, "run_nonce": run_nonce,
                      "revision": revision, "compiler": TOOLCHAIN,
                      "profile": {"kind": "full" if selected_packages is None else "packages",
                                  "phase": arguments.phase, "base": arguments.base,
                                  "packages": list(selected_packages or ())},
                      "host_capabilities": {"focus_order": "unsupported", "accessibility_tree": "unsupported",
                                            "pointer_dispatch": "unsupported", "responsive_cancellation": "unsupported"},
                      "source_sha256": hashlib.sha256(json.dumps(source_inputs, sort_keys=True).encode()).hexdigest(),
                      "lock_sha256": source_digest(LOCK)}
        EVIDENCE.update(provenance)
        evidence()
        external = sorted({p["name"] for p in metadata["packages"] if (p.get("source") or "").startswith("registry+")})
        for package in metadata["packages"]:
            if package["name"].startswith("metis"):
                for dependency in package["dependencies"]:
                    provider = dependency.get("source") or ""
                    registry_boundary = workspace.registry_boundary(package["name"], dependency["name"], provider)
                    if provider and not provider.startswith("git+https://github.com/ryancinsight/") and not registry_boundary:
                        raise SystemExit(f"Non-Atlas direct dependency: {dependency}")
        output_path("provider-dependencies.json").write_text(json.dumps(external, indent=2), encoding="utf-8")
        EVIDENCE["stages"]["build-links"] = "running"
        evidence()
        linked = sorted(
            [
                {
                    "name": package["name"],
                    "version": package["version"],
                    "source": package.get("source"),
                    "links": package["links"],
                    "reason": BUILD_LINK_PACKAGES.get(package["name"]),
                }
                for package in metadata["packages"]
                if package.get("links")
            ],
            key=lambda entry: (entry["name"], entry["version"], entry["source"] or ""),
        )
        output_path("build-links.json").write_text(json.dumps(linked, indent=2), encoding="utf-8")
        unknown_links = [entry["name"] for entry in linked if entry["name"] not in BUILD_LINK_PACKAGES]
        if unknown_links:
            EVIDENCE["stages"]["build-links"] = "failed"
            evidence()
            raise SystemExit(f"Unreviewed Cargo build-link packages: {', '.join(unknown_links)}")
        EVIDENCE["stages"]["build-links"] = "passed"
        evidence()
        packages = {p["id"]: p for p in metadata["packages"]}
        nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
        frontend = next(p["id"] for p in metadata["packages"] if p["name"] == "metis-frontend")
        pending, reached = [frontend], set()
        while pending:
            package_id = pending.pop()
            if package_id in reached:
                continue
            reached.add(package_id)
            if packages[package_id]["name"] in {"metis-backend", "metis-app", "metis-cli"}:
                raise SystemExit("Frontend dependency closure includes backend authority, application composition or distribution tooling")
            pending.extend(nodes[package_id]["dependencies"])
        # This exact installed runner applies the committed 30/60-second budgets.
        version = execute("nextest-version", ["rustup", "run", TOOLCHAIN, "cargo", "nextest", "--version"])
        if not version.startswith("cargo-nextest 0.9.143 "):
            raise SystemExit("Install pinned cargo-nextest 0.9.143 before running this gate")
        deny = shutil.which("cargo-deny")
        if deny is None:
            raise SystemExit(f"Install pinned cargo-deny {CARGO_DENY_VERSION} before running this gate")
        execute("supply-chain", [deny, "--manifest-path", str(ROOT / "Cargo.toml"),
                                  "--config", str(ROOT / "deny.toml"), "--locked", "--offline",
                                  "check", "advisories", "bans", "licenses", "sources"],
                seconds=180, cwd=ROOT)
        cargo("format", ["fmt", "--all", "--check"], resolve=False)
        execute(
            "visual-tests",
            [sys.executable, str(ROOT / "scripts" / "visual_tests.py")],
            seconds=60,
            cwd=ROOT,
        )
        execute("plan", [sys.executable, str(ROOT / "scripts" / "plan.py"), "check"], seconds=60, cwd=ROOT)
        scope_arguments = (["--workspace"] if selected_packages is None
                           else workspace.package_flags(selected_packages))
        run_native = arguments.phase != "release"
        run_followup = arguments.phase != "native"
        run_full_release = run_followup and selected_packages is None
        # Compile the browser host in the same locked gate; runtime evidence is
        # collected by scripts/browser.py and the manual browser trace.
        wasm_packages = ("metis-core", "metis-platform", "metis-ui-lang", "metis-web", "metis-starter")
        selected_wasm = (wasm_packages if selected_packages is None else
                         tuple(package for package in wasm_packages if package in selected_packages))
        if run_native and selected_wasm:
            cargo("wasm-libraries", ["build", "--lib", "--target", "wasm32-unknown-unknown",
                                     *workspace.package_flags(selected_wasm)])
        else:
            skip("wasm-libraries")
        if run_native and covers("metis-web", "metis-starter"):
            execute("browser-assets", [sys.executable, str(ROOT / "scripts" / "browser.py"), "build"],
                    seconds=300, cwd=ROOT)
        else:
            skip("browser-assets")
        if run_native:
            cargo("clippy", ["clippy", *scope_arguments, "--all-targets"],
                  tail=["--", "-D", "warnings"])
            cargo("build", ["build", *scope_arguments, "--bins", "--examples"])
        else:
            skip("clippy", "build")
        native_image = native_directory / "debug" / "examples" / (
            "native_image" + (".exe" if sys.platform == "win32" else "")
        )
        if run_native and covers("metis-platform"):
            execute("native-image", [str(native_image)], seconds=60, cwd=ROOT)
        else:
            skip("native-image")
        native_capture = native_directory / "debug" / "examples" / (
            "native_host_capture" + (".exe" if sys.platform == "win32" else "")
        )
        if run_native and sys.platform == "win32" and covers("metis-platform"):
            native_output = output_path("native-host")
            execute(
                "native-host-capture",
                [
                    str(native_capture),
                    "--output",
                    str(native_output),
                    "--source-revision",
                    revision,
                ],
                seconds=60,
                cwd=ROOT,
            )
            native_trace_path = native_output / "native-host-trace.json"
            if not native_trace_path.is_file():
                raise SystemExit("native-host-capture did not emit its execution trace")
            native_trace = json.loads(native_trace_path.read_text(encoding="utf-8"))
            if native_trace.get("source_revision") != revision or native_trace.get("host_result") != "ok":
                raise SystemExit("native-host-capture trace is not tied to the verified revision")
            EVIDENCE["native_host"] = native_trace
        elif sys.platform != "win32":
            EVIDENCE["stages"]["native-host-capture"] = "skipped: Windows-only host"
        else:
            skip("native-host-capture")
        if run_native:
            cargo("tests", ["nextest", "run", *scope_arguments, "--profile", "ci"])
        else:
            skip("tests")
        if run_native and covers("metis-backend", "metis-ipc"):
            cargo("quic-minimal", ["nextest", "run", "--no-default-features",
                                    "-p", "metis-backend", "-p", "metis-ipc", "--profile", "ci"])
        else:
            skip("quic-minimal")
        if run_native and covers("metis-python"):
            execute("python-binding", [sys.executable, str(ROOT / "scripts" / "python_binding.py")],
                    seconds=720, cwd=ROOT)
        else:
            skip("python-binding")
        if run_full_release:
            cargo("release-build", ["build", "--workspace", "--bins", "--release"])
            distribution_tool = native_directory / "release" / "metis.exe"
            execute("distribution", [sys.executable, str(ROOT / "scripts" / "distribution.py"),
                                     "--tool", str(distribution_tool), "--output", str(OUTPUT / "distribution"),
                                     *(["--install"] if arguments.install else [])], seconds=720)
            cargo("release-tests", ["nextest", "run", "--workspace", "--release", "--profile", "ci"])
        else:
            skip("release-build", "distribution", "release-tests")
        # Benchmarks are local measurement instruments, so the gate proves only
        # that every bench builds and executes one iteration. Timing runs and
        # their stored baselines stay on the controlled local machine class.
        if run_followup and covers("metis-platform"):
            cargo("bench-smoke", ["bench", "-p", "metis-platform", "--bench", "rasterizer"],
                  tail=("--", "--test"))
        else:
            skip("bench-smoke")
        if run_followup:
            cargo("doctests", ["test", *scope_arguments, "--doc"])
            cargo("docs", ["doc", *scope_arguments, "--no-deps"])
        else:
            skip("doctests", "docs")
        example = native_directory / "debug" / "examples" / ("clinical_infusion_workflow" + (".exe" if sys.platform == "win32" else ""))
        image = example.with_name("image" + (".exe" if sys.platform == "win32" else ""))
        presentation = example.with_name("presentation" + (".exe" if sys.platform == "win32" else ""))
        if run_followup and covers("metis-ui"):
            if arguments.phase == "release":
                cargo("examples-build", ["build", *scope_arguments, "--examples"])
            execute("example", [str(example)], seconds=60, cwd=ROOT)
            execute("image", [str(image)], seconds=60, cwd=ROOT)
            with tempfile.TemporaryDirectory(prefix="metis-capture-failure-") as failure_root:
                # A real directory at the CSV file path forces the OS write failure
                # while the backend awaits requests. The process must collect it.
                (pathlib.Path(failure_root) / "output" / "form.csv").mkdir(parents=True)
                execute("capture-failure", [str(presentation)], seconds=60, cwd=failure_root,
                        expected_exit=1,
                        required_diagnostic=("PermissionDenied" if sys.platform == "win32"
                                             else "IsADirectory"))
            execute("presentation", [str(presentation)], seconds=60, cwd=ROOT)
            provenance_path = output_path("visual-provenance.json")
            provenance_path.write_text(json.dumps(provenance, sort_keys=True), encoding="utf-8")
            execute("visual", [sys.executable, str(ROOT / "scripts" / "visual.py"),
                               "--root", str(ROOT), "--output", str(OUTPUT),
                               "--provenance", str(provenance_path),
                               *(["--update"] if arguments.update_snapshots else [])], seconds=60, cwd=ROOT)
            EVIDENCE["visual"] = json.loads(
                (OUTPUT / "visual" / "latest" / "report.json").read_text(encoding="utf-8")
            )
        else:
            skip("example", "image", "capture-failure", "presentation", "visual")
        documentation.manual_links(ROOT, PHYSICAL_ROOT)
        documentation.citations(ROOT)
        if source.state(ROOT, metadata, configs) != source_inputs:
            raise SystemExit("Source inputs changed during verification; collect against a stable revision")
        EVIDENCE["status"] = "passed"
        evidence()
        print(f"Verified Metis with {len(metadata['packages'])} resolved packages; provider transitive graph recorded", flush=True)


def main():
    """Run the gate from a workspace path that cannot inherit stack patches."""
    global LOCK, OUTPUT, ROOT
    previous = ROOT, OUTPUT, LOCK
    try:
        with neutral_workspace() as root:
            ROOT = root
            OUTPUT = ROOT / "output"
            LOCK = ROOT / "Cargo.lock"
            run_gate()
    finally:
        ROOT, OUTPUT, LOCK = previous


if __name__ == "__main__":
    try:
        main()
    except BaseException as error:
        if isinstance(error, SystemExit) and error.code == 0:
            raise
        EVIDENCE["status"] = "failed"
        EVIDENCE["error"] = str(error)
        try:
            evidence()
        except (OSError, ValueError) as report_error:
            print(f"Cannot write failure evidence: {report_error}", file=sys.stderr)
        raise
