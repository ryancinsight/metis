"""Test timeout-safe visual-suite case attribution."""

from __future__ import annotations

import io
import pathlib
import sys
import unittest


sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
from visual_tests import ProgressResult  # noqa: E402


class _Stream(io.StringIO):
    def __init__(self) -> None:
        super().__init__()
        self.flushes = 0

    def flush(self) -> None:
        self.flushes += 1
        super().flush()


class _Case(unittest.TestCase):
    def check_value(self) -> None:
        self.assertEqual(2 + 2, 4)


class ProgressResultTests(unittest.TestCase):
    def test_start_and_finish_are_flushed_with_elapsed_time(self) -> None:
        times = iter((10.0, 10.25))
        stream = _Stream()
        result = ProgressResult(stream, descriptions=False, verbosity=0, clock=lambda: next(times))
        case = _Case("check_value")

        result.startTest(case)
        self.assertEqual(stream.getvalue(), f"test-start: {case.id()}\n")
        self.assertEqual(stream.flushes, 1)

        result.stopTest(case)
        self.assertEqual(
            stream.getvalue(),
            f"test-start: {case.id()}\ntest-finish: 0.250000s {case.id()}\n",
        )
        self.assertEqual(stream.flushes, 2)

    def test_stop_without_start_is_rejected(self) -> None:
        result = ProgressResult(_Stream(), descriptions=False, verbosity=0)
        with self.assertRaisesRegex(RuntimeError, "without a matching start"):
            result.stopTest(_Case("check_value"))

    def test_skip_is_flushed_with_its_reason(self) -> None:
        stream = _Stream()
        result = ProgressResult(stream, descriptions=False, verbosity=0)
        case = _Case("check_value")
        result.addSkip(case, "platform decoder unavailable")
        self.assertEqual(
            stream.getvalue(),
            f"test-skip: {case.id()}: platform decoder unavailable\n",
        )
        self.assertEqual(result.skipped, [(case, "platform decoder unavailable")])
        self.assertEqual(stream.flushes, 1)


if __name__ == "__main__":
    unittest.main()
