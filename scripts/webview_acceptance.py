"""Verify actual Windows WebView2 Rust pages through native components and OS input."""

import argparse
import json
import os
from pathlib import Path
import sys
import tomllib

from webview_acceptance import cases, dispatch, evidence, model, retention
from webview_acceptance.contract import FIXTURE_SECONDS


def arguments(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("executable", "fixture-executable", "inventory", "root", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--worker", choices=cases.ROLES)
    parser.add_argument("--observe-startup", action="store_true")
    args = parser.parse_args(argv)
    if args.observe_startup and args.worker != "form":
        parser.error("--observe-startup is allowed only with --worker form")
    return args


def native_wave(wave, args, environment, report):
    commands = tuple((sys.executable, str(Path(__file__).resolve()), *case.arguments(),
                      "--executable", str(args.executable),
                      "--fixture-executable", str(args.fixture_executable),
                      "--inventory", str(args.inventory), "--root", str(args.root),
                      "--output", str(args.output / case.identifier), "--revision", args.revision)
                     for case in wave)
    actual_wave = dispatch.run(commands, args.root, environment)
    retained = retention.processes(actual_wave.outcomes, args.output, tuple(case.identifier for case in wave))
    admission_errors = list(retained.errors)
    for case, result, record in zip(wave, actual_wave.outcomes, retained.records, strict=True):
        stage = args.output / case.identifier
        report["stages"][case.identifier] = record
        result_path = stage / "evidence.json"
        try:
            if not result_path.is_file():
                if stage.exists():
                    record["partial_evidence_directory"] = str(stage)
                raise RuntimeError(f"actual native worker exits without evidence: {case.identifier}; "
                                   f"see {args.output / (case.identifier + '.log')}")
            actual = json.loads(result_path.read_text(encoding="utf-8"))
            record.update({"evidence_path": str(result_path), "evidence_sha256": evidence.digest(result_path),
                           "worker_status": actual.get("status"),
                           "status": "failed" if record.get("retention_errors") or result.error is not None or result.returncode != 0
                           else actual.get("status")})
            case.admit(actual)
            if actual.get("status") != "passed":
                raise RuntimeError(f"actual native acceptance fails: {case.identifier}; {actual.get('error')}")
        except BaseException as error:
            record["status"] = "failed"
            record["admission_error"] = f"{type(error).__name__}: {error}"
            admission_errors.append(error)
    actual_wave.require_success(tuple(admission_errors))


def main() -> None:
    args = arguments()
    report = {"schema": 1, "status": "running", "host": sys.platform,
              "revision": args.revision, "fixture_timeout_seconds": FIXTURE_SECONDS,
              "schedule_bound_seconds": cases.SCHEDULE_SECONDS, "stages": {}}
    args.output.mkdir(parents=True, exist_ok=True)
    caught = None
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
                args.root, args.output, module_digest, observe_startup=args.observe_startup)
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
        environment.pop(cases.SELECTION_ENV, None)
        report["unchanged_model"] = model.run(args.executable, args.root, args.output, environment, report)
        for wave in cases.WAVES:
            native_wave(wave, args, environment, report)
        if evidence.digest(lock) != lock_digest:
            raise ValueError("Cargo dependency lock changes during actual native acceptance")
        report["status"] = "passed"
    except BaseException as error:
        caught = error
        report["status"] = "failed"
        report["error"] = f"{type(error).__name__}: {error}"
        raise
    finally:
        if not args.worker:
            retention.report(args.output / "evidence.json", report, caught)


if __name__ == "__main__":
    main()
