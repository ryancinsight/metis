"""Run all unchanged real model calculations in one independent process wave."""

from pathlib import Path

from . import dispatch, retention
from .contract import CALCULATIONS, MAX_TRACE_BYTES, parse_headless


def run(executable: Path, root: Path, output: Path, environment: dict[str, str], report: dict) -> list[dict]:
    commands = tuple((str(executable), *case.arguments()) for case in CALCULATIONS)
    wave = dispatch.run(commands, root, environment)
    retained = retention.processes(wave.outcomes, output, tuple(f"model-{index}" for index in range(len(CALCULATIONS))),
                                   byte_limit=MAX_TRACE_BYTES)
    report["model_processes"] = retained.records
    wave.require_success(retained.errors)
    values = []
    for actual, case in zip(wave.outcomes, CALCULATIONS, strict=True):
        combined = actual.stdout + actual.stderr
        values.append(parse_headless(combined, case))
    return values
