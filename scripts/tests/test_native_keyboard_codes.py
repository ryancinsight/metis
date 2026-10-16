"""Real SendInput scan metadata delivered to one owned Win32 EDIT window.

The receiver records actual keyboard messages and forwards every native message
to the original window procedure. Normal translation and dispatch stay active;
the fixture never posts synthetic messages or installs a hook. One five-second
deadline covers setup and all input cases.
Pure mapping rejection vectors belong to the portable keyboard-record tests;
this test also rejects an actual SDK-unmapped virtual key before delivery.

https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-keybdinput
https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-mapvirtualkeyexw
https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-callwindowprocw
"""

import ctypes
import math
from pathlib import Path
import sys
import tempfile
import time
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_input.windows import abi, focus, keyboard


@unittest.skipUnless(sys.platform == "win32", "real keyboard scan delivery requires Windows")
class NativeKeyboardCodes(unittest.TestCase):
    SIMPLE = (0x44, 0x0D, 0x27)  # D, Return, logical Right.
    EXTENDED = (0xA3,)  # Side-specific Right Control has the E0 1D scan.
    CHORD = (0x12, 0x10, 0x44)  # Alt, Shift, D.
    DOMAIN = (0x11, 0x0D)  # Named Control+Enter input operation.
    BODY = (0x44,)  # One held D whose real filesystem body fails.

    def setUp(self):
        from python_native_accessibility import WEBVIEW2_FOCUS_PRIMER_TABS, _prepare_surface

        self.deadline = time.monotonic() + 5
        self.events, self.callback_errors = [], []
        self.prepare_surface = _prepare_surface
        self.primer_tabs = WEBVIEW2_FOCUS_PRIMER_TABS
        self.maximum_events = 2 * (len(self.SIMPLE) + len(self.EXTENDED) + len(self.CHORD)
                                   + len(self.DOMAIN) + len(self.BODY) + self.primer_tabs)
        self.user = ctypes.WinDLL("user32", use_last_error=True)
        self.bind()
        previous = int(self.user.GetForegroundWindow() or 0)
        self.addCleanup(self.user.SetForegroundWindow, previous)
        self.top = self.window("STATIC", None)
        self.edit = self.window("EDIT", self.top)
        focus.focus_window(self.top)
        self.user.SetFocus(self.edit)
        self.assertEqual(int(self.user.GetForegroundWindow() or 0), self.top)
        self.assertEqual(int(self.user.GetFocus() or 0), self.edit)
        self.pump()
        self.install_receiver()
        self.remaining()

    def bind(self):
        self.user.CreateWindowExW.argtypes = [
            ctypes.c_uint32, ctypes.c_wchar_p, ctypes.c_wchar_p, ctypes.c_uint32,
            ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int,
            ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p,
        ]
        self.user.CreateWindowExW.restype = ctypes.c_void_p
        for name in ("DestroyWindow", "IsWindow"):
            method = getattr(self.user, name)
            method.argtypes, method.restype = [ctypes.c_void_p], ctypes.c_int32
        for name in ("GetForegroundWindow", "GetFocus"):
            method = getattr(self.user, name)
            method.argtypes, method.restype = [], ctypes.c_void_p
        self.user.SetForegroundWindow.argtypes = [ctypes.c_void_p]
        self.user.SetForegroundWindow.restype = ctypes.c_int32
        self.user.SetFocus.argtypes, self.user.SetFocus.restype = [ctypes.c_void_p], ctypes.c_void_p
        self.user.GetKeyState.argtypes, self.user.GetKeyState.restype = [ctypes.c_int], ctypes.c_int16
        self.user.GetWindowThreadProcessId.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint32)]
        self.user.GetWindowThreadProcessId.restype = ctypes.c_uint32
        self.user.GetKeyboardLayout.argtypes, self.user.GetKeyboardLayout.restype = [ctypes.c_uint32], ctypes.c_void_p
        self.user.MapVirtualKeyExW.argtypes = [ctypes.c_uint32, ctypes.c_uint32, ctypes.c_void_p]
        self.user.MapVirtualKeyExW.restype = ctypes.c_uint32
        self.user.PeekMessageW.argtypes = [ctypes.POINTER(abi.WindowMessage), ctypes.c_void_p,
                                          ctypes.c_uint32, ctypes.c_uint32, ctypes.c_uint32]
        self.user.PeekMessageW.restype = ctypes.c_int32
        self.user.TranslateMessage.argtypes = [ctypes.POINTER(abi.WindowMessage)]
        self.user.TranslateMessage.restype = ctypes.c_int32
        self.user.DispatchMessageW.argtypes = [ctypes.POINTER(abi.WindowMessage)]
        self.user.DispatchMessageW.restype = ctypes.c_ssize_t
        self.user.MsgWaitForMultipleObjectsEx.argtypes = [ctypes.c_uint32, ctypes.c_void_p,
                                                        ctypes.c_uint32, ctypes.c_uint32, ctypes.c_uint32]
        self.user.MsgWaitForMultipleObjectsEx.restype = ctypes.c_uint32

    def window(self, name, parent):
        style = 0x10000000 | (0x40010000 if parent else 0x00CF0000)
        handle = int(self.user.CreateWindowExW(
            0, name, "Metis real keyboard metadata", style,
            0, 0, 320, 120, parent, 1 if parent else None, None, None,
        ) or 0)
        if not handle:
            raise ctypes.WinError(ctypes.get_last_error())
        self.addCleanup(self.destroy, handle)
        return handle

    def destroy(self, handle):
        if not self.user.DestroyWindow(handle):
            raise ctypes.WinError(ctypes.get_last_error())
        self.assertEqual(self.user.IsWindow(handle), 0)

    def install_receiver(self):
        # GWLP_WNDPROC is pointer-sized; Win32 exports the LONG spelling.
        suffix = "LongPtrW" if ctypes.sizeof(ctypes.c_void_p) == 8 else "LongW"
        self.get_procedure = getattr(self.user, "GetWindow" + suffix)
        self.get_procedure.argtypes = [ctypes.c_void_p, ctypes.c_int]
        self.get_procedure.restype = ctypes.c_ssize_t
        self.set_procedure = getattr(self.user, "SetWindow" + suffix)
        self.set_procedure.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_ssize_t]
        self.set_procedure.restype = ctypes.c_ssize_t
        self.user.CallWindowProcW.argtypes = [ctypes.c_void_p, ctypes.c_void_p,
                                             ctypes.c_uint32, ctypes.c_size_t, ctypes.c_ssize_t]
        self.user.CallWindowProcW.restype = ctypes.c_ssize_t
        self.original = self.get_procedure(self.edit, -4)
        if not self.original:
            raise ctypes.WinError(ctypes.get_last_error())
        signature = ctypes.WINFUNCTYPE(ctypes.c_ssize_t, ctypes.c_void_p,
                                      ctypes.c_uint32, ctypes.c_size_t, ctypes.c_ssize_t)
        self.procedure = signature(self.receive)
        address = ctypes.cast(self.procedure, ctypes.c_void_p).value
        previous = self.set_procedure(self.edit, -4, address)
        if not previous:
            raise ctypes.WinError(ctypes.get_last_error())
        self.addCleanup(self.restore_receiver)
        self.assertEqual(previous, self.original)
        self.assertEqual(self.get_procedure(self.edit, -4), address)

    def restore_receiver(self):
        # self.procedure remains strongly owned through restoration and window
        # destruction, including a native restoration failure.
        previous = self.set_procedure(self.edit, -4, self.original)
        if not previous:
            raise ctypes.WinError(ctypes.get_last_error())
        self.assertEqual(self.get_procedure(self.edit, -4), self.original)

    def receive(self, handle, message, word, parameter):
        if handle == self.edit and message in (0x100, 0x101, 0x104, 0x105):
            try:
                if len(self.events) == self.maximum_events:
                    raise ValueError("actual keyboard receiver exceeds its derived event count")
                self.events.append({
                    "window": handle, "message": message, "virtual_key": word,
                    "scan": (parameter >> 16) & 0xFF, "extended": bool(parameter & (1 << 24)),
                    "released": bool(parameter & (1 << 31)),
                    "previously_down": bool(parameter & (1 << 30)),
                    "alt_context": bool(parameter & (1 << 29)),
                    "modifiers": {name: bool(self.user.GetKeyState(key) & 0x8000)
                                  for name, key in (("alt", 0x12), ("shift", 0x10), ("control", 0x11))},
                })
            except BaseException as error:
                if not self.callback_errors:
                    self.callback_errors.append(error)
        return self.user.CallWindowProcW(self.original, handle, message, word, parameter)

    def remaining(self):
        seconds = self.deadline - time.monotonic()
        if seconds <= 0:
            raise TimeoutError(f"actual native keyboard fixture exceeds five seconds: {self.events!r}")
        return seconds

    def pump(self):
        message = abi.WindowMessage()
        while self.user.PeekMessageW(ctypes.byref(message), None, 0, 0, 1):
            self.remaining()
            self.user.TranslateMessage(ctypes.byref(message))
            self.user.DispatchMessageW(ctypes.byref(message))
        if self.callback_errors:
            raise self.callback_errors[0]

    def collect(self, count):
        while len(self.events) < count:
            self.pump()
            if len(self.events) >= count:
                break
            milliseconds = max(1, math.ceil(self.remaining() * 1000))
            # QS_ALLINPUT, MWMO_INPUTAVAILABLE: wait only within the original
            # fixture deadline, including messages that PeekMessage observed.
            result = self.user.MsgWaitForMultipleObjectsEx(0, None, milliseconds, 0x04FF, 0x0004)
            if result == 0xFFFFFFFF:
                raise ctypes.WinError(ctypes.get_last_error())
            self.remaining()
        self.assertEqual(len(self.events), count, self.events)

    def deliver(self, strokes):
        held, primary = [], None
        try:
            for stroke in strokes:
                stroke.send(key_up=False)
                held.append(stroke)
        except BaseException as error:
            primary = error
            raise
        finally:
            failures = []
            for stroke in reversed(held):
                try:
                    stroke.send(key_up=True)
                except BaseException as error:
                    failures.append(error)
            if failures:
                if primary is None:
                    primary = failures.pop(0)
                    for error in failures:
                        primary.add_note(f"releasing actual held key: {type(error).__name__}: {error}")
                    raise primary
                for error in failures:
                    primary.add_note(f"releasing actual held key: {type(error).__name__}: {error}")

    def assert_pair(self, events, key, mapping):
        down, up = events
        expected = {"window": self.edit, "virtual_key": key,
                    "scan": mapping & 0xFF, "extended": mapping >> 8 == 0xE0}
        self.assertNotEqual(expected["scan"], 0)
        for event in events:
            self.assertEqual({name: event[name] for name in expected}, expected)
            self.assertEqual(event["modifiers"], {"alt": False, "shift": False, "control": False})
        self.assertEqual((down["message"], up["message"]), (0x100, 0x101))
        self.assertEqual((down["released"], up["released"]), (False, True))
        self.assertEqual((down["previously_down"], up["previously_down"]), (False, True))

    def check_chord(self, strokes, native):
        before = len(self.events)
        self.deliver(tuple(strokes[key] for key in self.CHORD))
        self.collect(before + 2 * len(self.CHORD))
        chord = self.events[before:]
        self.assertEqual([event["virtual_key"] for event in chord], list(self.CHORD + tuple(reversed(self.CHORD))))
        for key in self.CHORD:
            pair = [event for event in chord if event["virtual_key"] == key]
            self.assertEqual([event["scan"] for event in pair], [native[key] & 0xFF] * 2)
            self.assertEqual([event["released"] for event in pair], [False, True])
        down, up = [event for event in chord if event["virtual_key"] == 0x44]
        self.assertEqual((down["message"], up["message"]), (0x104, 0x105))
        for event in (down, up):
            self.assertTrue(event["alt_context"])
            self.assertEqual(event["modifiers"], {"alt": True, "shift": True, "control": False})

    def check_extended_control(self, strokes, native, layout):
        key = self.EXTENDED[0]
        mapping = native[key]
        roundtrip = self.user.MapVirtualKeyExW(mapping, 3, layout)
        with self.subTest(requested_virtual_key=key, native_map4=mapping, native_map3=roundtrip,
                          stroke=strokes[key], actual_events=self.events):
            self.assertEqual(mapping, 0xE01D)
            self.assertEqual(roundtrip, key)
            before = len(self.events)
            self.deliver((strokes[key],))
            self.collect(before + 2 * len(self.EXTENDED))
            down, up = self.events[before:]
            expected = {"window": self.edit, "virtual_key": 0x11, "scan": 0x1D, "extended": True}
            for event in (down, up):
                self.assertEqual({name: event[name] for name in expected}, expected)
            self.assertEqual((down["message"], up["message"]), (0x100, 0x101))
            self.assertEqual((down["released"], up["released"]), (False, True))
            self.assertEqual((down["previously_down"], up["previously_down"]), (False, True))
            self.assertEqual([event["modifiers"] for event in (down, up)],
                             [{"alt": False, "shift": False, "control": True},
                              {"alt": False, "shift": False, "control": False}])

    def check_named_shortcut(self, layout):
        before = len(self.events)
        keyboard.send_shortcut("control-enter")
        self.collect(before + 2 * len(self.DOMAIN))
        events = self.events[before:]
        self.assertEqual([e["virtual_key"] for e in events], [0x11, 0x0D, 0x0D, 0x11])
        self.assertEqual([e["message"] for e in events], [0x100, 0x100, 0x101, 0x101])
        self.assertEqual([e["released"] for e in events], [False, False, True, True])
        self.assertEqual([e["modifiers"]["control"] for e in events], [True, True, True, False])
        for key in self.DOMAIN:
            mapping = self.user.MapVirtualKeyExW(key, 4, layout)
            pair = [e for e in events if e["virtual_key"] == key]
            self.assertEqual([e["scan"] for e in pair], [mapping & 0xFF] * 2)
            self.assertEqual([e["extended"] for e in pair], [mapping >> 8 == 0xE0] * 2)
        self.assertEqual({name: bool(self.user.GetKeyState(key) & 0x8000)
                          for name, key in (("alt", 0x12), ("shift", 0x10), ("control", 0x11))},
                         {"alt": False, "shift": False, "control": False})

    def check_surface_primer(self, layout):
        before = len(self.events)
        self.assertEqual(self.prepare_surface(self.top, "native"), 0)
        self.pump()
        self.assertEqual(len(self.events), before)
        self.assertEqual(self.prepare_surface(self.top, "webview2"), self.primer_tabs)
        self.collect(before + 2 * self.primer_tabs)
        mapping = self.user.MapVirtualKeyExW(0x09, 4, layout)
        events = self.events[before:]
        for index in range(self.primer_tabs):
            self.assert_pair(events[2 * index:2 * index + 2], 0x09, mapping)
        self.assertEqual(int(self.user.GetFocus() or 0), self.edit)
        self.assertEqual(int(self.user.GetForegroundWindow() or 0), self.top)

    def check_unfocused_mapping(self):
        self.remaining()
        try:
            self.user.SetFocus(None)
            self.assertEqual(int(self.user.GetFocus() or 0), 0)
            self.assertEqual(int(self.user.GetForegroundWindow() or 0), self.top)
            with self.assertRaisesRegex(RuntimeError, "no focused physical key recipient"):
                keyboard.physical_keys((0x44,))
            thread = self.user.GetWindowThreadProcessId(self.top, None)
            self.assertGreater(thread, 0)
            layout = self.user.GetKeyboardLayout(thread)
            self.assertIsNotNone(layout)
            mapping = self.user.MapVirtualKeyExW(0x12, 4, layout)
            (alt,) = keyboard.physical_keys((0x12,), recipient=keyboard.Recipient.FOREGROUND_QUEUE)
            self.assertEqual((alt.virtual_key, alt.scan_code, alt.extended),
                             (0x12, mapping & 0xFF, mapping >> 8 == 0xE0))
            self.assertNotEqual(alt.scan_code, 0)
        finally:
            self.user.SetFocus(self.edit)
            self.assertEqual(int(self.user.GetFocus() or 0), self.edit)
        self.assertEqual(int(self.user.GetForegroundWindow() or 0), self.top)
        self.remaining()

    def check_held_failure(self, strokes, native):
        before, primary = len(self.events), None
        with tempfile.TemporaryDirectory(prefix="native-keyboard-") as directory:
            missing = Path(directory) / "missing-body-input"
            try:
                with strokes[self.BODY[0]].held():
                    try:
                        missing.read_bytes()
                    except FileNotFoundError as error:
                        primary = error
                        raise
            except FileNotFoundError as actual:
                self.assertIs(actual, primary)
                self.assertEqual(actual.filename, str(missing))
            else:
                self.fail("the owned absent input must raise its native filesystem error")
        self.collect(before + 2 * len(self.BODY))
        self.assert_pair(self.events[before:], self.BODY[0], native[self.BODY[0]])

    def test_real_physical_keys_deliver_scan_extended_and_modifier_metadata(self):
        process = ctypes.c_uint32()
        thread = self.user.GetWindowThreadProcessId(self.edit, ctypes.byref(process))
        self.assertGreater(thread, 0)
        layout = self.user.GetKeyboardLayout(thread)
        self.assertNotEqual(layout, None)
        virtuals = tuple(dict.fromkeys(self.CHORD + self.SIMPLE + self.EXTENDED))
        native = {key: self.user.MapVirtualKeyExW(key, 4, layout) for key in virtuals}
        strokes = {}
        with self.subTest(hkl=int(layout), requested_virtual_keys=virtuals,
                          native_map4=native, strokes=strokes, actual_events=self.events):
            strokes.update(zip(virtuals, keyboard.physical_keys(virtuals), strict=True))
            repeated = keyboard.physical_keys((0x44, 0x44))
            self.assertIs(repeated[0], repeated[1])
            self.assertEqual((repeated[0].virtual_key, repeated[0].scan_code, repeated[0].extended),
                             (0x44, native[0x44] & 0xFF, native[0x44] >> 8 == 0xE0))
            for key in self.SIMPLE:
                with self.subTest(virtual_key=key):
                    before = len(self.events)
                    self.deliver((strokes[key],))
                    self.collect(before + 2)
                    self.assert_pair(self.events[before:], key, native[key])
            self.check_extended_control(strokes, native, layout)
            self.check_chord(strokes, native)
            self.check_named_shortcut(layout)
            self.check_surface_primer(layout)
            self.check_held_failure(strokes, native)
            # VK_LBUTTON is a real SDK virtual key without a keyboard scan mapping.
            self.assertEqual(self.user.MapVirtualKeyExW(0x01, 4, layout), 0)
            with self.assertRaises(ValueError):
                keyboard.physical_keys((0x01,))
            self.check_unfocused_mapping()
            self.pump()
            self.assertEqual(len(self.events), self.maximum_events)
            self.assertEqual(self.callback_errors, [])
            self.remaining()


if __name__ == "__main__":
    unittest.main()
