"""Compare the semantic text geometry contract across browser engines."""
from __future__ import annotations

import argparse
import json
import pathlib
import sys
from collections.abc import Mapping
from typing import Any, Dict, Sequence, Tuple

from browser_protocol import BrowserRuntimeError
from browser_text_stability import validate_text_geometry_stability
from browser_trace import BrowserEngine


MAX_TRACE_BYTES = 8 * 1024 * 1024
REQUIRED_ENGINES = tuple(engine.value for engine in BrowserEngine)
SEMANTIC_FIELDS = (
    "fixture",
    "utf16_length",
    "grapheme_boundaries",
    "visual_order",
    "line_count",
    "direction",
    "writing_mode",
)


def _parse_trace_spec(value: str) -> Tuple[str, pathlib.Path]:
    """Parse one ``engine=trace`` argument and reject ambiguous paths."""
    if not isinstance(value, str) or "=" not in value:
        raise BrowserRuntimeError("text comparison traces must use engine=path")
    engine, raw_path = value.split("=", 1)
    if engine not in REQUIRED_ENGINES:
        raise BrowserRuntimeError(f"unknown browser engine {engine!r}")
    if not raw_path or len(raw_path.encode("utf-8")) > 1024:
        raise BrowserRuntimeError("text comparison trace path is empty or oversized")
    return engine, pathlib.Path(raw_path)


def _load_trace(engine: str, path: pathlib.Path) -> Dict[str, Any]:
    """Load one bounded runtime trace and validate its lifecycle contract."""
    try:
        payload = path.read_bytes()
    except OSError as error:
        raise BrowserRuntimeError(f"cannot read {engine} trace {path}: {error}") from error
    if len(payload) > MAX_TRACE_BYTES:
        raise BrowserRuntimeError(f"{engine} trace exceeds its {MAX_TRACE_BYTES}-byte bound")
    try:
        document = json.loads(payload.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise BrowserRuntimeError(f"{engine} trace is not valid UTF-8 JSON") from error
    if not isinstance(document, Mapping):
        raise BrowserRuntimeError(f"{engine} trace is not an object")
    if document.get("schema") != 1 or document.get("status") != "passed":
        raise BrowserRuntimeError(f"{engine} trace is not a passed schema-1 document")
    if document.get("engine") != engine:
        raise BrowserRuntimeError(f"{engine} trace identifies another engine")
    metrics = document.get("metrics")
    if not isinstance(metrics, Mapping):
        raise BrowserRuntimeError(f"{engine} trace metrics are malformed")
    observations = metrics.get("text_geometry")
    stability = metrics.get("text_geometry_stability")
    if not isinstance(observations, list) or not isinstance(stability, Mapping):
        raise BrowserRuntimeError(f"{engine} trace lacks text geometry stability evidence")
    try:
        validated = validate_text_geometry_stability(observations)
    except BrowserRuntimeError as error:
        raise BrowserRuntimeError(f"{engine} text geometry is unstable: {error}") from error
    if validated.get("available") is not True:
        raise BrowserRuntimeError(f"{engine} text geometry is unavailable: {validated.get('reason')!r}")
    if stability.get("available") is not True or stability.get("stable") is not True:
        raise BrowserRuntimeError(f"{engine} text geometry stability record is not passed")
    measurement = observations[0]
    if not isinstance(measurement, Mapping):
        raise BrowserRuntimeError(f"{engine} text geometry observation is malformed")
    style = measurement.get("style")
    if not isinstance(style, Mapping):
        raise BrowserRuntimeError(f"{engine} text geometry style is malformed")
    values = {
        "fixture": measurement.get("fixture"),
        "utf16_length": measurement.get("utf16_length"),
        "grapheme_boundaries": measurement.get("grapheme_boundaries"),
        "visual_order": measurement.get("visual_order"),
        "line_count": measurement.get("line_count"),
        "direction": style.get("direction"),
        "writing_mode": style.get("writing_mode"),
    }
    if any(value is None for value in values.values()):
        raise BrowserRuntimeError(f"{engine} text geometry semantic fields are incomplete")
    return {
        "engine": engine,
        "path": path.as_posix(),
        "observations": len(observations),
        "values": values,
        "font_family": style.get("font_family"),
        "font_size_px": style.get("font_size_px"),
        "line_height_px": style.get("line_height_px"),
    }


def compare_text_geometry(
    traces: Sequence[Tuple[str, pathlib.Path]],
    required_engines: Sequence[str] = REQUIRED_ENGINES,
) -> Dict[str, Any]:
    """Require semantic text geometry to agree across the supplied engines."""
    if not 2 <= len(traces) <= len(REQUIRED_ENGINES):
        raise BrowserRuntimeError("text comparison requires between two and three engine traces")
    required = tuple(required_engines)
    if not required or any(engine not in REQUIRED_ENGINES for engine in required):
        raise BrowserRuntimeError("text comparison required engines are invalid")
    names = [engine for engine, _ in traces]
    if len(set(names)) != len(names):
        raise BrowserRuntimeError("text comparison contains a duplicate engine")
    missing = [engine for engine in required if engine not in names]
    if missing:
        raise BrowserRuntimeError(f"text comparison is missing required engines: {', '.join(missing)}")
    records = [_load_trace(engine, path) for engine, path in traces]
    baseline = records[0]["values"]
    for record in records[1:]:
        for field in SEMANTIC_FIELDS:
            if record["values"][field] != baseline[field]:
                raise BrowserRuntimeError(
                    f"text geometry {field} differs between {records[0]['engine']} and {record['engine']}"
                )
    return {
        "schema": 1,
        "status": "passed",
        "engines": [record["engine"] for record in records],
        "observations": {record["engine"]: record["observations"] for record in records},
        "semantic_contract": baseline,
        "font_environment": {
            record["engine"]: {
                "font_family": record["font_family"],
                "font_size_px": record["font_size_px"],
                "line_height_px": record["line_height_px"],
            }
            for record in records
        },
        "paths": {record["engine"]: record["path"] for record in records},
    }


def _arguments() -> argparse.Namespace:
    """Parse the bounded cross-engine comparison command."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--trace", action="append", required=True, metavar="ENGINE=PATH")
    parser.add_argument("--require-engine", action="append", choices=REQUIRED_ENGINES, dest="required_engines")
    parser.add_argument("--output", type=pathlib.Path, required=True)
    return parser.parse_args()


def main() -> int:
    """Compare traces and write a machine-readable result."""
    arguments = _arguments()
    output = arguments.output
    try:
        traces = [_parse_trace_spec(value) for value in arguments.trace]
        document = compare_text_geometry(traces, arguments.required_engines or REQUIRED_ENGINES)
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(json.dumps(document, sort_keys=True))
        return 0
    except (BrowserRuntimeError, OSError, TypeError, ValueError) as error:
        failure = {"schema": 1, "status": "failed", "error": str(error)}
        try:
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_text(json.dumps(failure, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        except OSError as write_error:
            print(f"cannot write text comparison failure: {write_error}", file=sys.stderr)
        print(f"cross-engine text comparison failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
