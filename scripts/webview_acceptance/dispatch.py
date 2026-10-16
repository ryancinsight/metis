"""Drain bounded real process waves in declared order before propagating errors.

Every submitted command owns its process tree through the existing runner.
Independent commands overlap; no exception cancels or abandons another owner.
Callers retain all returned streams before raising the original failure object.
"""

from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from pathlib import Path
import subprocess
import time

from process_tree import run as run_process_tree

from .contract import FIXTURE_SECONDS


@dataclass(frozen=True)
class Outcome:
    command: tuple[str, ...]
    returncode: int | None
    stdout: str | None
    stderr: str | None
    seconds: float
    error: BaseException | None

@dataclass(frozen=True)
class Wave:
    outcomes: tuple[Outcome, ...]
    interruption: BaseException | None = None

    def require_success(self, additional_errors: tuple[BaseException, ...] = ()) -> None:
        failures = [value.error if value.error is not None else subprocess.CalledProcessError(
            value.returncode, value.command, output=value.stdout, stderr=value.stderr)
            for value in self.outcomes if value.error is not None or value.returncode != 0]
        if self.interruption is not None:
            failures.insert(0, self.interruption)
        failures.extend(additional_errors)
        if failures:
            primary, *remaining = failures
            for error in remaining:
                primary.add_note(f"additional owned command failure: {type(error).__name__}: {error}")
            raise primary


def _execute(command: tuple[str, ...], root: Path, environment: dict[str, str]) -> Outcome:
    started = time.monotonic()
    try:
        actual = run_process_tree(command, cwd=root, env=environment, timeout=FIXTURE_SECONDS)
        return Outcome(command, actual.returncode, actual.stdout, actual.stderr,
                       time.monotonic() - started, None)
    except BaseException as error:
        return Outcome(command, getattr(error, "returncode", None), getattr(error, "stdout", None),
                       getattr(error, "stderr", None), time.monotonic() - started, error)


def run(commands: tuple[tuple[str, ...], ...], root: Path, environment: dict[str, str]) -> Wave:
    """Start every real owner, drain every future, and retain interruption identity."""
    if not commands:
        raise ValueError("actual process wave must contain a command")
    started = time.monotonic()
    results = [None] * len(commands)
    interruption = None
    with ThreadPoolExecutor(max_workers=len(commands)) as pool:
        futures = []
        for index, command in enumerate(commands):
            try:
                futures.append(pool.submit(_execute, command, root, environment))
            except BaseException as error:
                results[index] = Outcome(command, None, None, None, time.monotonic() - started, error)
                futures.append(None)
        for index, future in enumerate(futures):
            if future is None:
                continue
            while True:
                try:
                    results[index] = future.result()
                    break
                except BaseException as error:
                    if future.done() and future.exception() is error:
                        results[index] = Outcome(commands[index], None, None, None,
                                                 time.monotonic() - started, error)
                        break
                    if interruption is None:
                        interruption = error
                    else:
                        interruption.add_note(f"additional collection interruption: {error}")
    return Wave(tuple(results), interruption)
