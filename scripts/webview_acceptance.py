"""Verify actual Windows WebView2 Rust pages through native components and OS input."""

import argparse
import json
import os
from pathlib import Path
import sys
import time
import tomllib

from process_tree import ProcessTreeTimeout, run as run_process_tree
from webview_acceptance import evidence, fixtures
from webview_acceptance.contract import CALCULATIONS, FIXTURE_SECONDS, MAX_TRACE_BYTES, parse_headless


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("executable", "fixture-executable", "inventory", "root", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--worker", choices=("form", "permissions", "permissions_capture", *fixtures.FIXTURES))
    args = parser.parse_args()
    report = {"schema": 1, "status": "running", "host": sys.platform,
              "revision": args.revision, "fixture_timeout_seconds": FIXTURE_SECONDS,
              "stages": {}}
    args.output.mkdir(parents=True, exist_ok=True)
    try:
        if sys.platform != "win32":
            raise RuntimeError("actual Windows acceptance requires Windows, installed WebView2 and OS input")
        for name in ("executable", "fixture_executable", "inventory", "root"):
            path = getattr(args, name)
            if not path.is_absolute():
                raise ValueError(f"actual acceptance {name} must be absolute")
            setattr(args, name, path.resolve(strict=True))
        if not all(path.is_file() for path in (args.executable, args.fixture_executable, args.inventory)):
            raise ValueError("actual compiler-produced executable/inventory input is absent")
        module_digest = evidence.digest(args.inventory)
        if args.worker:
            from webview_acceptance.scenario import run
            # The coordinator creates only the parent; each actual worker owns
            # its previously nonexistent scenario evidence directory.
            args.output.rmdir()
            run(args.worker, args.executable, args.fixture_executable,
                args.root, args.output, module_digest)
            return
        lock = args.root / "Cargo.lock"
        lock_digest = evidence.digest(lock)
        report.update({"executable_sha256": evidence.digest(args.executable),
                       "fixture_executable_sha256": evidence.digest(args.fixture_executable),
                       "inventory_sha256": module_digest, "lock_sha256": lock_digest})
        providers = tomllib.loads(lock.read_text(encoding="utf-8"))["package"]
        report["providers"] = [{key: row.get(key) for key in ("name", "version", "source")}
                               for row in providers if row["name"].startswith("moirai-")]
        environment = os.environ.copy()
        environment.pop("METIS_WEBVIEW_ASSET_INVENTORY", None)
        before = []
        for index, case in enumerate(CALCULATIONS):
            result = run_process_tree([str(args.executable), *case.arguments()],
                                      cwd=args.root, env=environment, timeout=FIXTURE_SECONDS)
            combined = result.stdout + result.stderr
            if len(combined.encode("utf-8")) > MAX_TRACE_BYTES:
                raise ValueError("actual unchanged-model evidence exceeds its byte budget")
            (args.output / f"model-{index}.log").write_text(combined, encoding="utf-8")
            if result.returncode != 0:
                raise RuntimeError(f"actual unchanged model exits with {result.returncode}")
            before.append(parse_headless(combined, case))
        report["unchanged_model"] = before
        for name in ("form", "permissions", "permissions_capture", *fixtures.FIXTURES):
            stage = args.output / name
            command = [sys.executable, str(Path(__file__).resolve()),
                       "--worker", name, "--executable", str(args.executable),
                       "--fixture-executable", str(args.fixture_executable),
                       "--inventory", str(args.inventory), "--root", str(args.root),
                       "--output", str(stage), "--revision", args.revision]
            started = time.monotonic()
            try:
                result = run_process_tree(command, cwd=args.root, env=environment, timeout=FIXTURE_SECONDS)
            except ProcessTreeTimeout as error:
                report["stages"][name] = {"status": "failed", "seconds": time.monotonic() - started,
                                          "cleanup_error": error.cleanup_error, "error": str(error)}
                (args.output / f"{name}.log").write_text(error.stdout + error.stderr, encoding="utf-8")
                if stage.exists():
                    report["stages"][name]["partial_evidence_directory"] = str(stage)
                raise
            report["stages"][name] = {"returncode": result.returncode,
                                      "seconds": time.monotonic() - started}
            (args.output / f"{name}.log").write_text(result.stdout + result.stderr, encoding="utf-8")
            result_path = stage / "evidence.json"
            if not result_path.is_file():
                raise RuntimeError(f"actual native worker exits without evidence: {name}; see {args.output / (name + '.log')}")
            actual = json.loads(result_path.read_text(encoding="utf-8"))
            report["stages"][name].update({"evidence_path": str(result_path),
                                          "evidence_sha256": evidence.digest(result_path),
                                          "status": actual.get("status")})
            if result.returncode != 0 or actual.get("status") != "passed":
                raise RuntimeError(f"actual native acceptance fails: {name}; {actual.get('error')}")
        if evidence.digest(lock) != lock_digest:
            raise ValueError("Cargo dependency lock changes during actual native acceptance")
        report["status"] = "passed"
    except BaseException as error:
        report["status"] = "failed"
        report["error"] = f"{type(error).__name__}: {error}"
        raise
    finally:
        if not args.worker:
            (args.output / "evidence.json").write_text(json.dumps(report, indent=2, ensure_ascii=False), encoding="utf-8")


if __name__ == "__main__":
    main()
