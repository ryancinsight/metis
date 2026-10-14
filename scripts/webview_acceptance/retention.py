"""Retain every drained process outcome without replacing its original failure.

The optional combined UTF-8 stream bound is inclusive and rejects before file
creation. Every admitted stdout/stderr/combined write and every later outcome
is attempted independently. Original process exceptions precede these actual
persistence errors in Wave.require_success; no failed write constitutes success.
Final JSON persistence adds native error context to an existing failure instead
of replacing it during stack unwinding. There is no shortened stream or retry.
"""

from dataclasses import dataclass
from contextlib import contextmanager
import json
from pathlib import Path

from .dispatch import Outcome


@dataclass(frozen=True)
class Retention:
    records: tuple[dict, ...]
    errors: tuple[BaseException, ...]


def processes(outcomes: tuple[Outcome, ...], output: Path, names: tuple[str, ...], *,
              byte_limit: int | None = None) -> Retention:
    records, errors = [], []
    for actual, name in zip(outcomes, names, strict=True):
        record = {"command": actual.command, "returncode": actual.returncode,
                  "seconds": actual.seconds, "stdout_available": actual.stdout is not None,
                  "stderr_available": actual.stderr is not None}
        records.append(record)
        if actual.error is not None:
            record.update({"status": "failed", "error": f"{type(actual.error).__name__}: {actual.error}",
                           "cleanup_error": getattr(actual.error, "cleanup_error", None)})
        combined = actual.stdout + actual.stderr if actual.stdout is not None and actual.stderr is not None else None
        size = sum(len(text.encode("utf-8")) for text in (actual.stdout, actual.stderr) if text is not None)
        if byte_limit is not None and size > byte_limit:
            error = ValueError("actual process evidence exceeds its byte budget")
            errors.append(error)
            record.update({"status": "failed", "bytes": size,
                           "limit_bytes": byte_limit, "retention_errors": [str(error)]})
            continue
        for extension, text in (("stdout", actual.stdout), ("stderr", actual.stderr), ("log", combined)):
            if text is None:
                continue
            path = output / f"{name}.{extension}"
            try:
                path.write_text(text, encoding="utf-8")
            except BaseException as error:
                error.add_note(f"retaining actual owned process stream: {path!r}")
                errors.append(error)
                record["status"] = "failed"
                record.setdefault("retention_errors", []).append(f"{type(error).__name__}: {error}")
    return Retention(tuple(records), tuple(errors))


def report(path: Path, value: dict, primary: BaseException | None) -> None:
    """The caller continues unwinding primary after any secondary write failure."""
    try:
        path.write_text(json.dumps(value, indent=2, ensure_ascii=False), encoding="utf-8")
    except BaseException as error:
        if primary is None:
            raise
        primary.add_note(f"retaining actual evidence report {path!r}: {type(error).__name__}: {error}")


@contextmanager
def streams(paths: tuple[Path, ...]):
    """Close every actual native log while preserving a body/acquisition error."""
    opened, failures = [], []
    primary = None
    try:
        for path in paths:
            opened.append(path.open("wb"))
        yield tuple(opened)
    except BaseException as error:
        primary = error
        raise
    finally:
        for stream in reversed(opened):
            try:
                stream.close()
            except BaseException as error:
                error.add_note(f"closing actual native log: {stream.name!r}")
                failures.append(error)
        if primary is not None:
            for error in failures:
                primary.add_note(f"additional actual native log close failure: {type(error).__name__}: {error}")
        elif failures:
            primary, *remaining = failures
            for error in remaining:
                primary.add_note(f"additional actual native log close failure: {type(error).__name__}: {error}")
            raise primary
