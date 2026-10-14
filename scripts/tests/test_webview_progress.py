"""Real filesystem and computed-value laws for bounded UIA progress storage."""

import errno
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
from webview_acceptance.contract import MAX_UIA_BYTES
from webview_acceptance.progress import Progress, _encoded

# Exclusive opening of an existing directory returns EACCES through Windows
# _wopen and EEXIST through POSIX open; retain each native class and error code.
COLLISION_ERROR, COLLISION_ERRNO = ((PermissionError, errno.EACCES) if os.name == "nt"
                                    else (FileExistsError, errno.EEXIST))


class WebviewProgress(unittest.TestCase):
    def state(self, output):
        return json.loads((output / "uia-progress.json").read_text(encoding="utf-8"))

    def response(self, length):
        value = {"patient": "患者-🦀", "sum": 256 * 257 // 2, "padding": ""}
        base = json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode() + b"\r\n"
        value["padding"] = "a" * (length - len(base))
        raw = json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode() + b"\r\n"
        self.assertEqual(len(raw), length)
        return raw

    def test_sequence_context_and_computed_unicode_response_remain_exact(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            progress = Progress(output)
            initial = self.state(output)
            self.assertEqual((progress.sequence, initial["sequence"], initial["phase"]),
                             (0, "0", "initialized"))
            self.assertEqual(progress.actor_paths, tuple(output.resolve() / f"uia-actor-{i}.json" for i in (0, 1)))
            progress.operation("wait result 患者", ("result", "calculation-submit"))
            first = progress.begin("snapshot", "result")
            raw = json.dumps({"sum": sum(range(257)), "patient": "患者-🦀"}, ensure_ascii=False).encode() + b"\n"
            progress.received(raw)
            observed = self.state(output)
            self.assertEqual(first, 1)
            self.assertEqual(observed["request"], {"action": "snapshot", "id": "result"})
            self.assertEqual(observed["operation"], "wait result 患者")
            self.assertEqual(observed["identifiers"], ["result", "calculation-submit"])
            self.assertEqual(observed["last_response"], {"sequence": "1", "bytes": len(raw),
                                                       "sha256": hashlib.sha256(raw).hexdigest()})
            self.assertEqual((output / "uia-response.json").read_bytes(), raw)
            self.assertEqual(json.loads(raw)["sum"], 256 * 257 // 2)
            progress.mark("predicate", node={"id": "result", "text": "患者-🦀"}, accepted=False,
                          schema=257, sequence="diagnostic detail")
            current = self.state(output)
            self.assertEqual(current["details"], {"node": {"id": "result", "text": "患者-🦀"},
                                                 "accepted": False, "schema": 257,
                                                 "sequence": "diagnostic detail"})
            self.assertEqual((current["schema"], current["sequence"]), (1, "1"))
            self.assertGreaterEqual(int(current["monotonic_ns"]), int(observed["monotonic_ns"]))
            self.assertEqual(progress.begin("focus", None), 2)
            self.assertEqual((progress.sequence, self.state(output)["sequence"]), (2, "2"))

    def test_raw_inclusive_bound_rejects_before_changing_either_file(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            progress = Progress(output)
            progress.begin("snapshot", "result")
            for length in (MAX_UIA_BYTES - 1, MAX_UIA_BYTES):
                with self.subTest(length=length):
                    raw = self.response(length)
                    progress.received(raw)
                    self.assertEqual((output / "uia-response.json").read_bytes(), raw)
                    self.assertEqual(self.state(output)["last_response"]["bytes"], length)
            before = {path.name: path.read_bytes() for path in output.iterdir()}
            with self.assertRaisesRegex(ValueError, "response exceeds its byte budget"):
                progress.received(self.response(MAX_UIA_BYTES + 1))
            self.assertEqual({path.name: path.read_bytes() for path in output.iterdir()}, before)

    def test_state_encoder_preserves_values_at_each_exact_byte_boundary(self):
        original = {"schema": 1, "sequence": "257", "monotonic_ns": str(time.monotonic_ns()),
                    "details": {"sum": sum(range(257)), "patient": "患者-🦀", "padding": ""}}
        base = len(_encoded(original))
        for length in (MAX_UIA_BYTES - 1, MAX_UIA_BYTES, MAX_UIA_BYTES + 1):
            with self.subTest(length=length):
                value = {**original, "details": {**original["details"], "padding": "b" * (length - base)}}
                if length > MAX_UIA_BYTES:
                    with self.assertRaisesRegex(ValueError, "state exceeds its byte budget"):
                        _encoded(value)
                else:
                    raw = _encoded(value)
                    self.assertEqual(len(raw), length)
                    self.assertEqual(json.loads(raw), value)
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            progress = Progress(output)
            before = (output / "uia-progress.json").read_bytes()
            with self.assertRaisesRegex(ValueError, "state exceeds its byte budget"):
                progress.mark("too large", computed="患者" * MAX_UIA_BYTES)
            self.assertEqual((output / "uia-progress.json").read_bytes(), before)
            self.assertEqual(sorted(path.name for path in output.iterdir()), ["uia-progress.json"])
            candidate = {**self.state(output), "phase": "operation", "operation": ""}
            available = MAX_UIA_BYTES - len(_encoded(candidate)) - 32
            progress.operation("x" * available)
            self.assertGreater((output / "uia-progress.json").stat().st_size, MAX_UIA_BYTES - 64)
            before = (output / "uia-progress.json").read_bytes()
            with self.assertRaisesRegex(ValueError, "state exceeds its byte budget"):
                progress.received(self.response(257))
            self.assertEqual((output / "uia-progress.json").read_bytes(), before)
            self.assertEqual(sorted(path.name for path in output.iterdir()), ["uia-progress.json"])

    def test_replacement_changes_file_identity_and_never_decodes_response(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            progress = Progress(output)
            path = output / "uia-progress.json"
            old = path.stat()
            progress.mark("replacement", computed=sum(range(257)))
            new = path.stat()
            self.assertNotEqual((old.st_dev, old.st_ino), (new.st_dev, new.st_ino))
            self.assertEqual(self.state(output)["details"]["computed"], 256 * 257 // 2)
            for raw in (b"", b'{"duplicate":1,"duplicate":2}\r\n', bytes(range(256))):
                progress.received(raw)
                self.assertEqual((output / "uia-response.json").read_bytes(), raw)
                self.assertEqual(self.state(output)["last_response"]["sha256"], hashlib.sha256(raw).hexdigest())
            self.assertEqual(sorted(path.name for path in output.iterdir()),
                             ["uia-progress.json", "uia-response.json"])

    def test_native_pending_collision_preserves_old_bytes_and_primary_identity(self):
        with tempfile.TemporaryDirectory(prefix="progress-患者-") as directory:
            output = Path(directory)
            progress = Progress(output)
            pending = output.resolve() / "uia-progress.json.pending"
            pending.mkdir()
            before = (output / "uia-progress.json").read_bytes()
            with self.assertRaises(COLLISION_ERROR) as caught:
                progress.mark("native collision")
            self.assertEqual((type(caught.exception), caught.exception.errno),
                             (COLLISION_ERROR, COLLISION_ERRNO))
            self.assertEqual(Path(caught.exception.filename), pending)
            self.assertIn(f"{pending!r}", caught.exception.__notes__[0])
            computed = sum(range(257))
            try:
                computed // (computed - computed)
            except ZeroDivisionError as primary:
                original = primary
                original_message, original_arguments = str(primary), primary.args
                progress.mark("retain primary", primary=primary, computed=computed)
                self.assertIs(primary, original)
                self.assertEqual((str(primary), primary.args), (original_message, original_arguments))
                self.assertIn(f"{COLLISION_ERROR.__name__}: {caught.exception}", primary.__notes__[0])
                self.assertIn(f"{pending!r}", primary.__notes__[0])
            self.assertEqual((output / "uia-progress.json").read_bytes(), before)
            self.assertTrue(pending.is_dir())
            self.assertEqual(progress.sequence, 0)

    def test_received_state_failure_retains_actual_raw_and_later_truthful_summary(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            progress = Progress(output)
            progress.begin("snapshot", "result")
            progress.received(b"previous\n")
            old = (output / "uia-progress.json").read_bytes()
            pending = output.resolve() / "uia-progress.json.pending"
            pending.mkdir()
            raw = self.response(257)
            with self.assertRaises(COLLISION_ERROR) as caught:
                progress.received(raw)
            self.assertEqual((type(caught.exception), caught.exception.errno),
                             (COLLISION_ERROR, COLLISION_ERRNO))
            self.assertEqual((output / "uia-response.json").read_bytes(), raw)
            self.assertEqual((output / "uia-progress.json").read_bytes(), old)
            self.assertEqual(Path(caught.exception.filename), pending)
            self.assertIn(f"{pending!r}", caught.exception.__notes__[0])
            original = caught.exception
            original_values = (original.args, str(original), original.__notes__.copy())
            pending.rmdir()
            progress.mark("record primary", primary=caught.exception)
            self.assertIs(caught.exception, original)
            self.assertEqual((original.args, str(original), original.__notes__), original_values)
            self.assertEqual(self.state(output)["last_response"], {"sequence": "1", "bytes": len(raw),
                                                                  "sha256": hashlib.sha256(raw).hexdigest()})
            self.assertEqual(sorted(path.name for path in output.iterdir()),
                             ["uia-progress.json", "uia-response.json"])

    def test_actual_child_termination_keeps_the_last_complete_operation_and_raw(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            program = (
                "import os,sys; from pathlib import Path; sys.path.insert(0,sys.argv[1]); "
                "from webview_acceptance.progress import Progress; p=Progress(Path(sys.argv[2])); "
                "p.operation('real child wait',('result',)); p.begin('snapshot','result'); "
                "p.received('actual response 患者-🦀\\n'.encode()); "
                "p.mark('before_provider_call',computed=sum(range(257))); "
                "Path(sys.argv[2],'ready').mkdir(); os.read(0,1)"
            )
            child = subprocess.Popen([sys.executable, "-c", program, str(SCRIPTS), str(output)],
                                     stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            try:
                deadline = time.monotonic() + 5
                while time.monotonic() < deadline:
                    if child.poll() is not None:
                        self.fail(f"actual child exited before its retained checkpoint: {child.communicate()!r}")
                    if (output / "ready").is_dir():
                        break
                    time.sleep(0.005)
                else:
                    self.fail("actual child did not publish its checkpoint within five seconds")
                self.assertEqual(self.state(output)["phase"], "before_provider_call")
                before = {path.name: path.read_bytes() for path in output.iterdir() if path.is_file()}
                child.terminate()
                stdout, stderr = child.communicate(timeout=5)
                expected = -signal.SIGTERM if os.name == "posix" else 1
                self.assertEqual((child.returncode, stdout, stderr), (expected, b"", b""))
                self.assertEqual({path.name: path.read_bytes() for path in output.iterdir() if path.is_file()}, before)
                self.assertEqual(self.state(output)["details"]["computed"], 256 * 257 // 2)
                self.assertEqual((output / "uia-response.json").read_bytes(), "actual response 患者-🦀\n".encode())
            finally:
                if child.poll() is None:
                    child.kill()
                    child.communicate(timeout=5)


if __name__ == "__main__":
    unittest.main()
