"""Real process proofs for captured failure status, context, and full streams."""

import json
import os
import pathlib
import signal
import subprocess
import sys
import tempfile
import unittest


SCRIPTS = pathlib.Path(__file__).resolve().parents[1]
ACTOR = r'''
import json
import pathlib
import subprocess
import signal
import sys
sys.path.insert(0, sys.argv[1])
import browser
mode, status = sys.argv[3], int(sys.argv[4])
child = """
import os
import signal
import sys
sys.stdout.buffer.write(bytes.fromhex(sys.argv[1]))
sys.stderr.buffer.write(bytes.fromhex(sys.argv[2]))
sys.stdout.buffer.flush()
sys.stderr.buffer.flush()
status = int(sys.argv[3])
if status < 0:
    number = -status
    if number not in (signal.SIGKILL, signal.SIGSTOP):
        signal.signal(number, signal.SIG_DFL)
    os.kill(os.getpid(), number)
else:
    raise SystemExit(status)
"""
command = [sys.executable, "-c", child, sys.argv[5], sys.argv[6], str(status)]
try:
    result = browser.run(command, cwd=pathlib.Path(sys.argv[2]), timeout=10,
                         capture_output=True, text=mode == "text")
    code, stdout, stderr, args = result.returncode, result.stdout, result.stderr, result.args
    caught = False
except subprocess.CalledProcessError as error:
    code, stdout, stderr, args = error.returncode, error.stdout, error.stderr, error.cmd
    caught = True
if mode == "bytes":
    stdout, stderr = stdout.hex(), stderr.hex()
print(json.dumps({"returncode": code, "stdout": stdout, "stderr": stderr,
                  "command": args, "caught": caught}), flush=True)
if code == -signal.SIGTERM:
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
elif code == -signal.SIGINT:
    signal.pthread_sigmask(signal.SIG_BLOCK, {signal.SIGINT})
browser.terminate(code)
'''


class CapturedDiagnostics(unittest.TestCase):
    def invoke(self, mode, status, stdout, stderr):
        with tempfile.TemporaryDirectory() as directory:
            environment = os.environ.copy()
            # Diagnostic writes must retain Unicode even on an ASCII console.
            environment["PYTHONIOENCODING"] = "ascii"
            result = subprocess.run(
                [sys.executable, "-c", ACTOR, str(SCRIPTS), directory, mode,
                 str(status), stdout.hex(), stderr.hex()],
                env=environment, capture_output=True, timeout=30,
            )
            value = json.loads(result.stdout)
            self.assertEqual(value["returncode"], status)
            self.assertEqual(result.returncode, status)
            self.assertEqual(value["command"][-3:], [stdout.hex(), stderr.hex(), str(status)])
            if status:
                first, diagnostic = result.stderr.split(b"\n", 1)
                context = json.loads(first.removeprefix(b"browser subprocess failed: "))
                self.assertEqual(context, {"command": value["command"], "cwd": directory,
                                           "returncode": status})
                self.assertEqual(diagnostic, b"stdout:\n" + stdout + b"\nstderr:\n" + stderr + b"\n")
            else:
                self.assertEqual(result.stderr, b"")
            return value

    def test_real_nonzero_child_retains_unicode_streams_and_context(self):
        cases = [
            (7, "output λ 患者\nlast output", "error 🙂 caméra\nlast error\n"),
            (23, "", "拒否 geolocation\n"),
        ]
        if os.name == "posix":
            cases.extend(((-signal.SIGTERM, "signal λ\n", "terminated 患者\n"),
                          (-signal.SIGINT, "interrupt 🙂\n", "interrupted caméra\n")))
        for status, stdout, stderr in cases:
            with self.subTest(status=status):
                actual = self.invoke("text", status, stdout.encode("utf-8"), stderr.encode("utf-8"))
                self.assertEqual(actual["stdout"], stdout)
                self.assertEqual(actual["stderr"], stderr)
                self.assertEqual(actual["caught"], True)

    def test_real_nonzero_child_retains_every_binary_byte(self):
        stdout, stderr = bytes(range(256)), bytes(reversed(range(256)))
        statuses = [31, -signal.SIGKILL] if os.name == "posix" else [31]
        for status in statuses:
            with self.subTest(status=status):
                actual = self.invoke("bytes", status, stdout, stderr)
                self.assertEqual(actual["stdout"], stdout.hex())
                self.assertEqual(actual["stderr"], stderr.hex())
                self.assertEqual(actual["caught"], True)

    def test_success_preserves_returned_streams_without_failure_emission(self):
        stdout, stderr = "result λ\n", "warning 患者\n"
        actual = self.invoke("text", 0, stdout.encode("utf-8"), stderr.encode("utf-8"))
        self.assertEqual(actual["stdout"], stdout)
        self.assertEqual(actual["stderr"], stderr)
        self.assertEqual(actual["caught"], False)


if __name__ == "__main__":
    unittest.main()
