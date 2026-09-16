"""Validate and compare two revision-bound lifecycle provenance records.

The comparator accepts provenance records produced from real application runs.
It requires callers to name the semantic fields that must match before any
resource delta is calculated, and reports both means and combined uncertainty.
It never chooses a faster or smaller framework and never copies input paths or
labels from the source records into its output.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import pathlib
import stat
import sys
from collections.abc import Mapping, Sequence
from typing import Any


MAX_INPUT_BYTES = 2 * 1024 * 1024
MAX_OUTPUT_BYTES = 256 * 1024
MAX_MATCH_KEYS = 32
MAX_MATCH_VALUE_BYTES = 64 * 1024
MAX_NAME_UNITS = 80
DEFAULT_METRICS = (
    "peak_private_bytes",
    "peak_working_set_bytes",
    "duration_ms",
    "startup_observation_ms",
)


class ComparisonError(ValueError):
    """Raised when provenance records cannot be compared safely."""


def _regular_file(path: pathlib.Path, *, role: str) -> pathlib.Path:
    """Return a regular, single-link file without following redirections."""
    path = pathlib.Path(path)
    try:
        attributes = path.lstat()
    except OSError as error:
        raise ComparisonError(f"{role} is not readable") from error
    if path.is_symlink() or bool(
        getattr(attributes, "st_file_attributes", 0) & stat.FILE_ATTRIBUTE_REPARSE_POINT
    ):
        raise ComparisonError(f"{role} is redirected")
    if not path.is_file() or attributes.st_nlink != 1:
        raise ComparisonError(f"{role} is not a single regular file")
    if attributes.st_size > MAX_INPUT_BYTES:
        raise ComparisonError(f"{role} exceeds the {MAX_INPUT_BYTES}-byte bound")
    return path


def _read_record(path: pathlib.Path, *, role: str) -> tuple[Mapping[str, Any], str]:
    """Read one bounded JSON record and return it with its content digest."""
    path = _regular_file(path, role=role)
    try:
        content = path.read_bytes()
        record = json.loads(content.decode("utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError, RecursionError) as error:
        raise ComparisonError(f"{role} is not valid UTF-8 JSON") from error
    if not isinstance(record, Mapping):
        raise ComparisonError(f"{role} root must be an object")
    return record, hashlib.sha256(content).hexdigest()


def _validate_output(path: pathlib.Path) -> pathlib.Path:
    """Reject redirected output paths and create their ordinary parent."""
    path = pathlib.Path(path)
    for candidate in (path, *path.parents):
        try:
            attributes = candidate.lstat()
        except FileNotFoundError:
            continue
        except OSError as error:
            raise ComparisonError("comparison output path is not accessible") from error
        if candidate.is_symlink() or bool(
            getattr(attributes, "st_file_attributes", 0) & stat.FILE_ATTRIBUTE_REPARSE_POINT
        ):
            raise ComparisonError("comparison output path is redirected")
    path = path.resolve()
    if path.exists() and (not path.is_file() or path.stat().st_nlink != 1):
        raise ComparisonError("comparison output path is not a single regular file")
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
    except OSError as error:
        raise ComparisonError("comparison output directory is unavailable") from error
    return path


def _mapping(record: Mapping[str, Any], path: str, *, role: str) -> Any:
    """Resolve a dotted object path without accepting indexes or escapes."""
    value: Any = record
    components = path.split(".")
    if not path or any(not component or component.startswith("_") for component in components):
        raise ComparisonError(f"invalid match key {path!r}")
    for component in components:
        if not isinstance(value, Mapping) or component not in value:
            raise ComparisonError(f"{role} lacks match key {path!r}")
        value = value[component]
    try:
        encoded = _encode_json(value)
    except ComparisonError as error:
        raise ComparisonError(f"match key {path!r} is not JSON-compatible") from error
    if len(encoded.encode("utf-8")) > MAX_MATCH_VALUE_BYTES:
        raise ComparisonError(f"match key {path!r} exceeds its size bound")
    return value


def _canonical(value: Any) -> str:
    """Encode a match value deterministically for equality and output."""
    return _encode_json(value)


def _encode_json(value: Any) -> str:
    """Encode one value with deterministic ordering and strict JSON numbers."""
    try:
        return json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        )
    except (TypeError, ValueError, RecursionError) as error:
        raise ComparisonError("match value is not JSON-compatible") from error


def _finite_number(value: Any, *, field: str, role: str) -> float:
    """Validate one non-negative finite numeric metric value."""
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ComparisonError(f"{role} metric {field!r} is not numeric")
    value = float(value)
    if not math.isfinite(value) or value < 0:
        raise ComparisonError(f"{role} metric {field!r} is not finite and non-negative")
    return value


def _metric(record: Mapping[str, Any], name: str, *, role: str) -> Mapping[str, float | int]:
    """Read one repeated metric and validate its uncertainty contract."""
    resource = record.get("resource")
    if not isinstance(resource, Mapping):
        raise ComparisonError(f"{role} has no resource metrics")
    value = resource.get(name)
    if not isinstance(value, Mapping):
        raise ComparisonError(f"{role} lacks resource metric {name!r}")
    mean = _finite_number(value.get("mean"), field=f"{name}.mean", role=role)
    half_width = _finite_number(
        value.get("approximate_95_half_width"),
        field=f"{name}.approximate_95_half_width",
        role=role,
    )
    count = value.get("count")
    if isinstance(count, bool) or not isinstance(count, int) or count < 2:
        raise ComparisonError(f"{role} metric {name!r} needs at least two samples")
    return {"mean": mean, "approximate_95_half_width": half_width, "count": count}


def _validate_record(record: Mapping[str, Any], *, role: str) -> str:
    """Validate the shared provenance envelope and return its phase."""
    if record.get("schema") != 1 or record.get("status") != "passed":
        raise ComparisonError(f"{role} must be a passed schema-1 record")
    runtime = record.get("runtime")
    if not isinstance(runtime, Mapping) or not isinstance(runtime.get("phase"), str):
        raise ComparisonError(f"{role} lacks a runtime phase")
    phase = runtime["phase"]
    if not phase or len(phase) > 32 or any(ord(character) < 0x20 for character in phase):
        raise ComparisonError(f"{role} has an invalid runtime phase")
    return phase


def _label(value: str, *, role: str) -> str:
    """Validate an output label supplied by the caller."""
    if not isinstance(value, str) or not value or len(value) > MAX_NAME_UNITS:
        raise ComparisonError(f"{role} must be a non-empty label of at most {MAX_NAME_UNITS} characters")
    if any(ord(character) < 0x20 or character in "\\/\r\n\t" for character in value):
        raise ComparisonError(f"{role} contains a control character or path separator")
    return value


def compare(
    left: Mapping[str, Any],
    right: Mapping[str, Any],
    *,
    left_name: str,
    right_name: str,
    match_keys: Sequence[str],
    metric_names: Sequence[str] = DEFAULT_METRICS,
    left_digest: str | None = None,
    right_digest: str | None = None,
) -> Mapping[str, Any]:
    """Compare matched provenance records without selecting a winner."""
    if not match_keys:
        raise ComparisonError("at least one --match key is required")
    if len(match_keys) > MAX_MATCH_KEYS or any(not isinstance(key, str) for key in match_keys):
        raise ComparisonError(f"match key count must be between 1 and {MAX_MATCH_KEYS}")
    if len(set(match_keys)) != len(match_keys):
        raise ComparisonError("match keys must be unique")
    if not metric_names:
        raise ComparisonError("at least one metric is required")
    if len(set(metric_names)) != len(metric_names):
        raise ComparisonError("metrics must be unique")
    left_phase = _validate_record(left, role="left record")
    right_phase = _validate_record(right, role="right record")
    if left_phase != right_phase:
        raise ComparisonError("runtime phases do not match")
    matches: dict[str, Any] = {}
    for key in match_keys:
        left_value = _mapping(left, key, role="left record")
        right_value = _mapping(right, key, role="right record")
        if _canonical(left_value) != _canonical(right_value):
            raise ComparisonError(f"match key {key!r} differs between records")
        matches[key] = left_value

    metrics: dict[str, Any] = {}
    for name in metric_names:
        if not name or name.startswith("_") or "." in name:
            raise ComparisonError(f"invalid metric name {name!r}")
        left_metric = _metric(left, name, role="left record")
        right_metric = _metric(right, name, role="right record")
        delta = right_metric["mean"] - left_metric["mean"]
        combined = math.hypot(
            left_metric["approximate_95_half_width"],
            right_metric["approximate_95_half_width"],
        )
        baseline = left_metric["mean"]
        metrics[name] = {
            "left": left_metric,
            "right": right_metric,
            "delta_right_minus_left": delta,
            "relative_delta": None if baseline == 0 else delta / baseline,
            "combined_approximate_95_half_width": combined,
            "delta_within_combined_interval": abs(delta) <= combined,
        }
    result: dict[str, Any] = {
        "schema": 1,
        "status": "passed",
        "runtime_phase": left_phase,
        "left": {"name": _label(left_name, role="left name")},
        "right": {"name": _label(right_name, role="right name")},
        "matched": matches,
        "metrics": metrics,
        "interpretation": (
            "The records satisfy the requested semantic match keys. "
            "Deltas are reported in right-minus-left order with combined "
            "approximate 95% half-widths; this record does not rank frameworks."
        ),
    }
    if left_digest is not None or right_digest is not None:
        if not isinstance(left_digest, str) or not isinstance(right_digest, str):
            raise ComparisonError("both source digests are required together")
        result["sources"] = {
            "left_sha256": left_digest,
            "right_sha256": right_digest,
        }
    return result


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--left", required=True, type=pathlib.Path, help="left provenance JSON")
    parser.add_argument("--right", required=True, type=pathlib.Path, help="right provenance JSON")
    parser.add_argument("--left-name", required=True, help="bounded output label for the left record")
    parser.add_argument("--right-name", required=True, help="bounded output label for the right record")
    parser.add_argument("--match", action="append", required=True,
                        help="dotted JSON field that must match; repeat for each semantic field")
    parser.add_argument("--metric", action="append", dest="metrics",
                        help="resource metric to compare; defaults to the standard lifecycle metrics")
    parser.add_argument("--output", required=True, type=pathlib.Path, help="comparison JSON output")
    return parser


def main(arguments: Sequence[str] | None = None) -> int:
    """Validate two records, compare requested metrics and write one report."""
    args = _parser().parse_args(arguments)
    left, left_digest = _read_record(args.left, role="left record")
    right, right_digest = _read_record(args.right, role="right record")
    result = compare(
        left,
        right,
        left_name=args.left_name,
        right_name=args.right_name,
        match_keys=tuple(args.match),
        metric_names=tuple(args.metrics or DEFAULT_METRICS),
        left_digest=left_digest,
        right_digest=right_digest,
    )
    output = json.dumps(result, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    encoded = output.encode("utf-8")
    if len(encoded) > MAX_OUTPUT_BYTES:
        raise ComparisonError("comparison output exceeds its size bound")
    path = _validate_output(args.output)
    try:
        path.write_bytes(encoded)
    except OSError as error:
        raise ComparisonError("comparison output could not be written") from error
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ComparisonError as error:
        print(f"resource comparison: {error}", file=sys.stderr)
        raise SystemExit(1) from error
