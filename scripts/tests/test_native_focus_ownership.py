"""Real Win32 parent/focus ownership; the full WebView journeys own integration.

Predefined native classes and styles:
https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-createwindowexw
https://learn.microsoft.com/en-us/windows/win32/winmsg/window-styles
"""

import ctypes
import itertools
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_input.windows import focus


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
        self.assertFalse(focus._activation_matches(self.user, self.top, 0, ledger))
        self.assertFalse(ledger["criterion"]["focus_within_target"])
        self.assertEqual(ledger["criterion"]["focus_hwnd"], self.grandchild)


if __name__ == "__main__":
    unittest.main()
