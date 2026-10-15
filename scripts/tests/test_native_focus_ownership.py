"""Real Win32 parent/focus ownership; the full WebView journeys own integration.

Predefined native classes and styles:
https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-createwindowexw
https://learn.microsoft.com/en-us/windows/win32/winmsg/window-styles
"""

import ctypes
import hashlib
import itertools
import json
from pathlib import Path
import sys
import tempfile
import time
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_input.windows import focus, focus_observation
from native.pump import run_with_pump


@unittest.skipUnless(sys.platform == "win32", "real Win32 window/focus ownership requires Windows")
class NativeFocusOwnership(unittest.TestCase):
    def setUp(self):
        self.user, _ = focus._focus_api()
        self.child_ids = itertools.count(1)
        self.user.CreateWindowExW.argtypes = [
            ctypes.c_uint32, ctypes.c_wchar_p, ctypes.c_wchar_p, ctypes.c_uint32,
            ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int,
            ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p,
        ]
        self.user.CreateWindowExW.restype = ctypes.c_void_p
        self.user.DestroyWindow.argtypes = [ctypes.c_void_p]
        self.user.DestroyWindow.restype = ctypes.c_int32
        self.user.IsWindow.argtypes = [ctypes.c_void_p]
        self.user.IsWindow.restype = ctypes.c_int32
        previous = int(self.user.GetForegroundWindow() or 0)
        self.addCleanup(self.user.SetForegroundWindow, previous)
        self.top = self.window("STATIC")
        self.target = self.window("STATIC", self.top)
        self.child = self.window("STATIC", self.target)
        self.grandchild = self.window("EDIT", self.child)
        self.sibling = self.window("EDIT", self.top)
        self.outside = self.window("STATIC")
        self.outside_child = self.window("EDIT", self.outside)
        focus.focus_window(self.top)
        self.assertEqual(int(self.user.GetForegroundWindow() or 0), self.top)

    def window(self, window_class, parent=None):
        # WS_VISIBLE and either WS_CHILD or WS_OVERLAPPEDWINDOW. Geometry only
        # places real controls; the ownership oracle depends on the native tree.
        style = 0x10000000 | (0x40000000 if parent else 0x00CF0000)
        child_id = next(self.child_ids) if parent else None
        handle = int(self.user.CreateWindowExW(
            0, window_class, "Metis native focus ownership", style,
            0, 0, 320, 240, parent, child_id, None, None,
        ) or 0)
        if not handle:
            raise ctypes.WinError()
        self.addCleanup(self.destroy, handle)
        return handle

    def destroy(self, handle):
        if not self.user.DestroyWindow(handle):
            raise ctypes.WinError()
        self.assertEqual(self.user.IsWindow(handle), 0)

    def delegate_parent_focus(self, target):
        """Install a real native host route; this does not emulate WebView2."""
        procedure = ctypes.WINFUNCTYPE(
            ctypes.c_ssize_t, ctypes.c_void_p, ctypes.c_uint32,
            ctypes.c_size_t, ctypes.c_ssize_t)
        self.user.SetWindowLongPtrW.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p]
        self.user.SetWindowLongPtrW.restype = ctypes.c_void_p
        self.user.CallWindowProcW.argtypes = [
            ctypes.c_void_p, ctypes.c_void_p, ctypes.c_uint32,
            ctypes.c_size_t, ctypes.c_ssize_t]
        self.user.CallWindowProcW.restype = ctypes.c_ssize_t
        self.delegations = []

        @procedure
        def dispatch(handle, message, word, parameter):
            if message == 0x0007:  # Actual WM_SETFOCUS, produced by SetFocus.
                self.user.SetFocus(target)
                self.delegations.append(int(self.user.GetFocus() or 0))
            return self.user.CallWindowProcW(previous, handle, message, word, parameter)

        self.parent_procedure = dispatch  # Own callback storage through removal.
        previous = self.user.SetWindowLongPtrW(self.top, -4, dispatch)
        if not previous:
            error = ctypes.WinError()
            raise RuntimeError(f"actual native focus procedure installation failed: {error}") from error
        installed = ctypes.cast(dispatch, ctypes.c_void_p).value

        def restore():
            removed = self.user.SetWindowLongPtrW(self.top, -4, previous)
            if not removed:
                error = ctypes.WinError()
                raise RuntimeError(f"actual native focus procedure restoration failed: {error}") from error
            self.assertEqual(removed, installed, "restore removes the exact owned callback")

        self.addCleanup(restore)

    def test_real_parent_entry_follows_delegation_instead_of_an_owned_child_guess(self):
        self.delegate_parent_focus(self.grandchild)
        _, _, guessed = focus.focus_window(self.top)
        prior = int(self.user.GetFocus() or 0)
        self.assertTrue(prior == guessed or self.user.IsChild(guessed, prior))
        self.assertNotEqual(prior, self.grandchild)
        self.assertEqual(self.delegations, [])
        _, foreground, selected = focus.focus_window(
            self.top, entry=focus.FocusEntry.NATIVE_PARENT)
        self.assertTrue(foreground)
        self.assertEqual(selected, self.top)
        self.assertEqual(self.delegations, [self.grandchild])
        self.assertEqual(int(self.user.GetFocus() or 0), self.grandchild)
        self.assertEqual(int(self.user.GetForegroundWindow() or 0), self.top)

    def test_real_parent_entry_rejects_a_host_that_does_not_delegate(self):
        with self.assertRaisesRegex(RuntimeError, "did not delegate") as caught:
            focus.focus_window(self.top, entry=focus.FocusEntry.NATIVE_PARENT)
        self.assertEqual(int(self.user.GetFocus() or 0), self.top)
        report = caught.exception.focus_observation
        self.assertEqual(len(report["activation"]["attempts"]), 1)
        self.assertEqual(report["activation"]["criterion"]["focus_hwnd"], self.top)

    def test_real_recipient_parent_chain_and_foreground_define_admission(self):
        self.assertEqual(self.user.IsChild(self.target, self.target), 0)
        cases = (
            ("target", self.target, True),
            ("child", self.child, True),
            ("grandchild", self.grandchild, True),
            ("sibling", self.sibling, False),
            ("ancestor", self.top, False),
            ("unrelated", self.outside_child, False),
            ("absent", 0, False),
        )
        for name, recipient, expected in cases:
            with self.subTest(recipient=name):
                self.user.SetForegroundWindow(self.top)
                self.user.SetFocus(recipient)
                actual = int(self.user.GetFocus() or 0)
                self.assertEqual(actual, recipient)
                ledger = {"attempts": [], "criterion": None}
                admitted = focus._activation_matches(self.user, self.top, self.target, ledger)
                criterion = ledger["criterion"]
                self.assertEqual(criterion["focus_hwnd"], recipient)
                self.assertEqual(criterion["foreground_hwnd"], int(self.user.GetForegroundWindow() or 0))
                self.assertEqual(criterion["focus_within_target"], expected)
                if name != "unrelated":
                    self.assertEqual(criterion["foreground_hwnd"], self.top)
                self.assertEqual(admitted, expected)
        self.user.SetFocus(self.grandchild)
        self.assertEqual(int(self.user.GetFocus() or 0), self.grandchild)
        ledger = {"attempts": [], "criterion": None}
        self.assertFalse(focus._activation_matches(self.user, self.outside, self.target, ledger))
        self.assertTrue(ledger["criterion"]["focus_within_target"])
        self.assertNotEqual(ledger["criterion"]["foreground_hwnd"], self.outside)
        self.assertEqual(ledger["criterion"]["focus_hwnd"], self.grandchild)
        observed = focus_observation.input_state(self.top)
        self.assertEqual(observed["read_errors"], [])
        self.assertEqual(observed["windows"]["foreground"]["hwnd"], self.top)
        self.assertEqual(observed["foreground_recipient"]["hwnd"], self.grandchild)
        self.assertEqual(observed["foreground_recipient"]["class_name"], "Edit")
        self.assertTrue(observed["recipient_within_top"])
        self.assertEqual(set(observed["modifiers"]), {"control", "shift", "alt"})
        for state in observed["modifiers"].values():
            self.assertEqual(state["high_bit_set"], bool(state["raw"] & 0x8000))
        self.assertFalse(focus._activation_matches(self.user, self.top, 0, ledger))
        self.assertFalse(ledger["criterion"]["focus_within_target"])
        self.assertEqual(ledger["criterion"]["focus_hwnd"], self.grandchild)

    def test_real_uia_actor_retains_exact_reply_and_native_call_progress(self):
        from webview_acceptance.uia import Uia

        retained = Path(__file__).resolve().parents[2] / "output/browser/windows-uia-regression"
        retained.mkdir(parents=True, exist_ok=True)
        output = Path(tempfile.mkdtemp(prefix="metis-uia-患者-🦀-'‘’‚‛-$()`", dir=retained))
        self.user.SetWindowTextW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p]
        self.user.SetWindowTextW.restype = ctypes.c_int32
        self.user.GetWindowTextLengthW.argtypes = [ctypes.c_void_p]
        self.user.GetWindowTextLengthW.restype = ctypes.c_int32
        self.user.GetWindowTextW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p, ctypes.c_int]
        self.user.GetWindowTextW.restype = ctypes.c_int
        value = "患者-🦀-" + str(256 * 257 // 2)
        if not self.user.SetWindowTextW(self.grandchild, value):
            raise ctypes.WinError()
        length = self.user.GetWindowTextLengthW(self.grandchild)
        buffer = ctypes.create_unicode_buffer(length + 1)
        copied = self.user.GetWindowTextW(self.grandchild, buffer, length + 1)
        (output / "fixture.json").write_text(json.dumps({
            "schema": 1, "top": self.top, "target": self.grandchild,
            "expected": value, "native_value": buffer.value,
            "native_units": length, "copied_units": copied,
        }, ensure_ascii=False), encoding="utf-8")
        self.assertEqual(length, len(value.encode("utf-16-le")) // 2)
        self.assertEqual(copied, length)
        self.assertEqual(buffer.value, value)
        actor = Uia(self.top, time.monotonic() + 60, output)
        primary = None
        try:
            nodes = run_with_pump(self.user, actor, actor.request, "snapshot")
            matches = [node for node in nodes if node["value"] == value]
            self.assertEqual(len(matches), 1, json.dumps(nodes, ensure_ascii=True))
            self.assertEqual(matches[0]["value"], value)
            self.assertEqual(matches[0]["role"], "ControlType.Edit")
            self.assertEqual(matches[0]["value_source"], "ValuePattern")
            raw = (output / "uia-response.json").read_bytes()
            self.assertEqual(json.loads(raw), {"schema": 1, "nodes": nodes})
            progress = json.loads((output / "uia-progress.json").read_bytes())
            self.assertEqual(progress["phase"], "response_admitted")
            self.assertEqual(progress["sequence"], "1")
            self.assertEqual(progress["last_response"]["bytes"], len(raw))
            self.assertEqual(progress["last_response"]["sha256"], hashlib.sha256(raw).hexdigest())
        except BaseException as error:
            primary = error
            error.add_note(f"actual UIA regression evidence: {output!r}")
            raise
        finally:
            try:
                actor.close()
            except BaseException as cleanup:
                if primary is None:
                    raise
                primary.add_note(f"actual UIA test cleanup: {type(cleanup).__name__}: {cleanup}")
        final = json.loads((output / "uia-progress.json").read_bytes())
        self.assertEqual(final["phase"], "actor_collected")
        self.assertEqual(final["details"]["returncode"], 0)
        # Actor exit closes both slots after its final stdin marker;
        # observing them earlier would race the last post-response write.
        slots = [json.loads(path.read_bytes()) for path in actor.progress.actor_paths]
        latest = max(slots, key=lambda record: int(record["step"]))
        self.assertEqual(latest["request_sequence"], "1")
        self.assertEqual(latest["phase"], "stdin_read")
        self.assertGreater(int(latest["stopwatch_ticks"]), 0)
        self.assertGreater(int(latest["stopwatch_frequency"]), 0)
        self.assertGreater(latest["process_id"], 0)
        self.assertGreater(latest["managed_thread_id"], 0)
        self.assertIn(latest["apartment"], ("STA", "MTA", "Unknown"))
        self.assertEqual(latest["node_index"], -1)


if __name__ == "__main__":
    unittest.main()
