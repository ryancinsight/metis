"""Run standard unittest discovery with verbose monotonic case observations.

The stdlib owns discovery, ordering, fixtures, subtests, every outcome and the
process exit policy. In its verbose ``showAll`` mode, each actual
startTest/stopTest pair emits its test.id and native monotonic nanoseconds.
For start s and end e, duration=e-s >= 0; differences between adjacent case
records also expose elapsed time outside cases. They do not assign that time
to any fixture or import. Cumulative counts come from the original result.

Start records precede the stdlib's unterminated verbose case line; completion
records follow its outcome line. Human output stays intact. Every record is
flushed so the existing outer process-group deadline retains the last started
case and its start time, even when no completion exists. No child process,
environment or argument instrumentation changes the workload. Quiet and dot
modes keep stdlib output without reading the observer clock or test identity.
"""
import json
import time
import unittest


class TimedTextTestResult(unittest.TextTestResult):
    """Observe actual case boundaries without replacing stdlib result callbacks."""

    def _record(self, value):
        self.stream.writeln("unittest-timing: " + json.dumps(value, sort_keys=True))
        self.stream.flush()

    def startTest(self, test):
        if self.showAll:
            self._case_start_ns = time.monotonic_ns()
            self._record({"event": "case_started", "test": test.id(),
                          "monotonic_ns": self._case_start_ns})
        super().startTest(test)

    def stopTest(self, test):
        super().stopTest(test)
        if not self.showAll:
            return
        ended = time.monotonic_ns()
        self._record({
            "event": "case_finished", "test": test.id(), "monotonic_ns": ended,
            "duration_ns": ended - self._case_start_ns,
            "result_counts": {"tests_run": self.testsRun, "failures": len(self.failures),
                              "errors": len(self.errors), "skips": len(self.skipped),
                              "expected_failures": len(self.expectedFailures),
                              "unexpected_successes": len(self.unexpectedSuccesses)},
        })


class TimedTextTestRunner(unittest.TextTestRunner):
    """Keep standard runner construction and execution with observed results."""

    resultclass = TimedTextTestResult


if __name__ == "__main__":
    unittest.main(module=None, testRunner=TimedTextTestRunner)
