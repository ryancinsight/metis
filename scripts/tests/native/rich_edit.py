"""Real system Rich Edit with a read-only, forwarding message observer.

MSFTEDIT_CLASS and EM_EXGETSEL come from the published Win32 SDK Richedit.h.
CHARRANGE is two signed 32-bit LONGs, including on a 64-bit Windows process.
Every observed message reaches the original WNDPROC exactly once. No selection
or text setter runs after construction; the production actor owns all input.

https://learn.microsoft.com/en-us/windows/win32/controls/create-rich-edit-controls
https://learn.microsoft.com/en-us/windows/win32/controls/em-exgetsel
https://learn.microsoft.com/en-us/windows/win32/api/richedit/ns-richedit-charrange
https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-callwindowprocw
"""

import ctypes

from native_input.windows import focus


class CharacterRange(ctypes.Structure):
    _fields_ = [("start", ctypes.c_int32), ("end", ctypes.c_int32)]


class RichEdit:
    def __init__(self, seed, maximum_events):
        self.user = ctypes.WinDLL("user32", use_last_error=True)
        self.kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        self.seed = seed
        self.maximum_events = maximum_events
        self.events, self.errors = [], []
        self.module, self.top, self.edit = 0, 0, 0
        self.procedure = self.original = None
        self.previous_foreground = 0
        self.bind()

    def bind(self):
        self.kernel.LoadLibraryExW.argtypes = [ctypes.c_wchar_p, ctypes.c_void_p, ctypes.c_uint32]
        self.kernel.LoadLibraryExW.restype = ctypes.c_void_p
        self.kernel.FreeLibrary.argtypes = [ctypes.c_void_p]
        self.kernel.FreeLibrary.restype = ctypes.c_int32
        self.user.CreateWindowExW.argtypes = [
            ctypes.c_uint32, ctypes.c_wchar_p, ctypes.c_wchar_p, ctypes.c_uint32,
            ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int,
            ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p,
        ]
        self.user.CreateWindowExW.restype = ctypes.c_void_p
        for name in ("DestroyWindow", "IsWindow", "IsWindowUnicode"):
            method = getattr(self.user, name)
            method.argtypes, method.restype = [ctypes.c_void_p], ctypes.c_int32
        self.user.GetForegroundWindow.argtypes, self.user.GetForegroundWindow.restype = [], ctypes.c_void_p
        self.user.SetForegroundWindow.argtypes = [ctypes.c_void_p]
        self.user.SetForegroundWindow.restype = ctypes.c_int32
        self.user.GetWindowTextLengthW.argtypes = [ctypes.c_void_p]
        self.user.GetWindowTextLengthW.restype = ctypes.c_int32
        self.user.GetWindowTextW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p, ctypes.c_int]
        self.user.GetWindowTextW.restype = ctypes.c_int
        self.user.SendMessageW.argtypes = [ctypes.c_void_p, ctypes.c_uint32,
                                          ctypes.c_size_t, ctypes.c_void_p]
        self.user.SendMessageW.restype = ctypes.c_ssize_t
        self.user.GetKeyState.argtypes, self.user.GetKeyState.restype = [ctypes.c_int], ctypes.c_int16

    def open(self):
        self.previous_foreground = int(self.user.GetForegroundWindow() or 0)
        # LOAD_LIBRARY_SEARCH_SYSTEM32 excludes the checkout from DLL lookup.
        self.module = int(self.kernel.LoadLibraryExW("Msftedit.dll", None, 0x800) or 0)
        if not self.module:
            raise ctypes.WinError(ctypes.get_last_error())
        self.top = self.create("STATIC", "Metis real text replacement", None)
        self.edit = self.create("RICHEDIT50W", self.seed, self.top)
        if not self.user.IsWindowUnicode(self.edit):
            raise ValueError("actual Rich Edit control is not Unicode")
        focus.focus_window(self.top)
        self.install()

    def create(self, name, text, parent):
        # A single-line child with WS_TABSTOP/ES_AUTOHSCROLL exposes its complete
        # input rather than constraining text to the current visible width.
        style = 0x10000000 | (0x40010080 if parent else 0x00CF0000)
        handle = int(self.user.CreateWindowExW(
            0, name, text, style, 0, 0, 640, 160,
            parent, 1 if parent else None, None, None,
        ) or 0)
        if not handle:
            raise ctypes.WinError(ctypes.get_last_error())
        return handle

    def install(self):
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
        previous = self.set_procedure(self.edit, -4, ctypes.cast(self.procedure, ctypes.c_void_p).value)
        if not previous:
            raise ctypes.WinError(ctypes.get_last_error())
        if previous != self.original:
            raise ValueError("actual Rich Edit original procedure changes during admission")

    def selection(self):
        selected = CharacterRange()
        # WM_USER+52 has no defined result: zero is not a native failure code.
        self.user.SendMessageW(self.edit, 0x400 + 52, 0, ctypes.byref(selected))
        return {"start": selected.start, "end": selected.end}

    def text(self):
        length = self.user.GetWindowTextLengthW(self.edit)
        # Even ignored SelectAll cannot produce more than the initial text plus
        # the declared input stream. Reject unexpected external growth before
        # allocating; message metadata already bounds that stream from above.
        maximum = len(self.seed.encode("utf-16-le")) // 2 + self.maximum_events
        if not 0 <= length <= maximum:
            raise ValueError(f"actual Rich Edit text exceeds its derived unit bound: {length}/{maximum}")
        buffer = ctypes.create_unicode_buffer(length + 1)
        copied = self.user.GetWindowTextW(self.edit, buffer, length + 1)
        return {"value": buffer.value, "units": length, "copied_units": copied}

    def capture(self):
        return {"selection": self.selection(), "text": self.text(),
                "control_high_bit_set": bool(self.user.GetKeyState(0x11) & 0x8000)}

    def receive(self, handle, message, word, parameter):
        tracked = handle == self.edit and (
            message == 0x102 or (message == 0x100 and word == 0x41)
            or (message == 0x101 and word == 0x11))
        event = None
        if tracked:
            try:
                if len(self.events) >= self.maximum_events:
                    raise ValueError("actual Rich Edit exceeds its derived input event budget")
                event = {"message": message, "word": word, "before": self.capture()}
            except BaseException as error:
                if not self.errors:
                    self.errors.append(error)
        result = self.user.CallWindowProcW(self.original, handle, message, word, parameter)
        if event is not None:
            try:
                event["after"] = self.capture()
                self.events.append(event)
            except BaseException as error:
                if not self.errors:
                    self.errors.append(error)
        return result

    def close(self, primary):
        failures = []
        if self.edit and self.user.IsWindow(self.edit):
            try:
                if self.procedure is not None:
                    if not self.set_procedure(self.edit, -4, self.original):
                        raise ctypes.WinError(ctypes.get_last_error())
                if not self.user.DestroyWindow(self.edit):
                    raise ctypes.WinError(ctypes.get_last_error())
            except BaseException as error:
                failures.append(error)
        if self.top and self.user.IsWindow(self.top):
            try:
                if not self.user.DestroyWindow(self.top):
                    raise ctypes.WinError(ctypes.get_last_error())
            except BaseException as error:
                failures.append(error)
        if self.module:
            try:
                if self.edit and self.user.IsWindow(self.edit):
                    raise RuntimeError("live Rich Edit HWND prevents releasing its native module")
                if not self.kernel.FreeLibrary(self.module):
                    raise ctypes.WinError(ctypes.get_last_error())
            except BaseException as error:
                failures.append(error)
        if self.previous_foreground:
            self.user.SetForegroundWindow(self.previous_foreground)
        if failures:
            if primary is None:
                primary = failures.pop(0)
                for error in failures:
                    primary.add_note(f"actual Rich Edit cleanup: {error!r}")
                raise primary
            for error in failures:
                primary.add_note(f"actual Rich Edit cleanup: {error!r}")
