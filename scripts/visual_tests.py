"""Run the Python visual suite with timeout-safe case attribution."""

from __future__ import annotations

import pathlib
import sys
import time
import unittest
from collections.abc import Callable
from typing import TextIO


TESTS = pathlib.Path(__file__).resolve().parent / "tests"


class ProgressResult(unittest.TextTestResult):
    """Record each active case and its elapsed time before process exit."""

    def __init__(
        self,
        stream: TextIO,
        descriptions: bool,
        verbosity: int,
        *,
        durations: int | None = None,
        clock: Callable[[], float] = time.perf_counter,
    ) -> None:
        super().__init__(stream, descriptions, verbosity, durations=durations)
        self._clock = clock
        self._started_at: float | None = None

    def startTest(self, test: unittest.case.TestCase) -> None:
        super().startTest(test)
        self._started_at = self._clock()
        self.stream.write(f"test-start: {test.id()}\n")
        self.stream.flush()

    def stopTest(self, test: unittest.case.TestCase) -> None:
        started_at = self._started_at
        if started_at is None:
            raise RuntimeError("test result stopped without a matching start")
        elapsed = self._clock() - started_at
        self.stream.write(f"test-finish: {elapsed:.6f}s {test.id()}\n")
        self.stream.flush()
        self._started_at = None
        super().stopTest(test)


def main() -> int:
    """Discover and run every scripts test under the gate's existing policy."""
    suite = unittest.defaultTestLoader.discover(str(TESTS))
    runner = unittest.TextTestRunner(
        verbosity=0,
        resultclass=ProgressResult,
        durations=0,
    )
    return 0 if runner.run(suite).wasSuccessful() else 1


if __name__ == "__main__":
    sys.exit(main())
