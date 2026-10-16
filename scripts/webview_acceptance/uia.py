"""One real UI Automation actor, bounded by the scenario's process-tree deadline."""

import ctypes
from contextlib import ExitStack
import json
import math
import msvcrt
import os
from pathlib import Path
import subprocess
import time

from native_display import physical_coordinates
from native_input.windows import abi, focus, focus_observation, keyboard, pointer, queue
from python_native_accessibility import MAX_UIA_NODES, UIA_POLL_MILLISECONDS

from .contract import MAX_UIA_BYTES
from .progress import Progress
from .retention import publish, streams
from .selection import admit
from .text import TextValueRepresentation


class Uia:
    def __init__(self, handle: int, deadline: float, output: Path, *,
                 focus_entry: focus.FocusEntry = focus.FocusEntry.CONTENT_CHILD):
        if not isinstance(focus_entry, focus.FocusEntry):
            raise TypeError("actual UIA actor requires an explicit native focus policy")
        self.handle = handle
        self.deadline = deadline
        self.focus_entry = focus_entry
        self.progress = Progress(output)
        # PowerShell's lexer recognizes five single-quote code points. Each
        # literal occurrence doubles, including curly quotes in native paths.
        # https://github.com/PowerShell/PowerShell/blob/master/src/System.Management.Automation/engine/parser/CharTraits.cs
        # Paths remain data in the complete owned UTF-16 script. CreateProcessW
        # receives its path instead of the expanding encoded program body.
        quotes = {ord(quote): quote * 2 for quote in "'‘’‚‛"}
        paths = ",".join("'" + str(path).translate(quotes) + "'"
                         for path in self.progress.actor_paths)
        helper = Path(__file__).with_name("uia_progress.ps1").read_text(encoding="utf-8")
        selection = Path(__file__).with_name("uia_selection.ps1").read_text(encoding="utf-8")
        initialize = f"Initialize-UiaProgress @{{paths=@({paths});maximum_bytes={MAX_UIA_BYTES}}}\n"
        program = helper + "\n" + selection + "\n" + initialize + Path(__file__).with_suffix(".ps1").read_text(encoding="utf-8")
        path = self.progress.actor_paths[0].with_name("uia-program.ps1")
        publish(path, b"\xff\xfe" + program.encode("utf-16-le"))
        # Popen duplicates the redirected native handle for the child. Closing
        # the parent's log after successful launch leaves that child owner live.
        with streams((path.with_name("uia.log"),)) as (error,):
            self.process = subprocess.Popen(
                ["powershell.exe", "-NoProfile", "-NonInteractive", "-File", str(path)],
                stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=error,
            )
        self.actions = []
        self.latest = []

    def remaining(self) -> float:
        remaining = self.deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError("actual UIA scenario exceeds its strict fixture deadline")
        return remaining

    def request(self, action: str, identifier: str | None = None) -> list[dict]:
        sequence = self.progress.begin(action, identifier)
        message = {"handle": self.handle, "action": action, "id": identifier,
                   "sequence": str(sequence), "maximum_nodes": MAX_UIA_NODES}
        self.process.stdin.write((json.dumps(message) + "\n").encode("utf-8"))
        self.process.stdin.flush()
        self.progress.mark("request_sent")
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.PeekNamedPipe.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_uint32,
                                        ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint32), ctypes.c_void_p]
        kernel.PeekNamedPipe.restype = ctypes.c_int32
        handle = msvcrt.get_osfhandle(self.process.stdout.fileno())
        result = bytearray()
        while True:
            self.remaining()
            available = ctypes.c_uint32()
            if not kernel.PeekNamedPipe(handle, None, 0, None, ctypes.byref(available), None):
                raise ctypes.WinError(ctypes.get_last_error())
            if available.value:
                result.extend(os.read(self.process.stdout.fileno(), min(available.value, MAX_UIA_BYTES + 1)))
                if len(result) > MAX_UIA_BYTES:
                    raise ValueError("actual UIA response exceeds its byte budget")
                if b"\n" in result:
                    break
            elif self.process.poll() is not None:
                raise RuntimeError("actual UIA actor exits before its response")
            else:
                time.sleep(min(self.remaining(), UIA_POLL_MILLISECONDS / 1000))
        self.progress.received(bytes(result))
        decoded = json.loads(result.decode("utf-8"))
        if decoded.get("schema") != 1 or "error" in decoded:
            raise RuntimeError(f"actual UIA operation fails: {decoded}")
        nodes = decoded.get("nodes")
        if not isinstance(nodes, list) or not 0 < len(nodes) <= MAX_UIA_NODES:
            raise ValueError("actual UIA tree exceeds its node contract")
        self.latest = nodes
        self.progress.mark("response_admitted", nodes=len(nodes))
        if action == "focus":
            self.actions.append({"action": "UIA SetFocus", "id": identifier})
        return nodes

    def node(self, identifier: str) -> dict:
        self.progress.operation("node", (identifier,))
        matched = [node for node in self.request("snapshot", identifier) if node["id"] == identifier]
        if len(matched) != 1:
            raise RuntimeError(f"actual UIA control is missing or ambiguous: {identifier}")
        return matched[0]

    def wait(self, identifier: str, predicate) -> dict:
        self.progress.operation("wait", (identifier,))
        while True:
            matched = [node for node in self.request("snapshot", identifier) if node["id"] == identifier]
            if len(matched) > 1:
                raise RuntimeError(f"actual UIA control is ambiguous: {identifier}")
            accepted = bool(matched and predicate(node := matched[0]))
            self.progress.mark("wait_observed", matches=len(matched), accepted=accepted)
            if accepted:
                self.actions.append({"action": "observed UIA", "id": identifier, "node": node})
                return node
            time.sleep(min(self.remaining(), UIA_POLL_MILLISECONDS / 1000))

    def selection(self, identifier: str) -> dict:
        """Observe only the uniquely named control, without changing its ranges."""
        nodes = self.request("selection", identifier)
        matched = [node for node in nodes if node["id"] == identifier]
        if len(matched) != 1:
            raise RuntimeError(f"actual UIA selection control is missing or ambiguous: {identifier}")
        admit(matched[0].get("selection"))
        return matched[0]

    def text(self, identifier: str) -> str:
        return self.node(identifier)["text"]

    def enter_host(self, identifier: str | None = None, *, observed=None) -> None:
        """Use the declared native entry once; observe the selected field again."""
        self.remaining()
        focus.focus_window(self.handle, entry=self.focus_entry)
        self.progress.mark("native_focus_entered", policy=self.focus_entry.name)
        self.actions.append({"action": "native focus entry", "policy": self.focus_entry.name,
                             "parent": self.handle, "selected_field": identifier})
        if observed is not None:
            observed("after_native_entry")
        else:
            self.progress.input({"kind": "native_focus_entry", "policy": self.focus_entry.name,
                                 "selected_field": identifier,
                                 "native": focus_observation.input_state(self.handle)})
        if identifier is not None:
            actual = self.node(identifier)
            if actual["focused"] is not True:
                raise RuntimeError(f"native parent entry does not preserve selected field {identifier}")

    def wait_hidden(self, *identifiers: str) -> list[dict]:
        self.progress.operation("wait_hidden", identifiers)
        while True:
            matched = [node for node in self.request("snapshot") if node["id"] in identifiers]
            if any(sum(node["id"] == identifier for node in matched) > 1 for identifier in identifiers):
                raise RuntimeError("actual hidden UIA control is ambiguous")
            accepted = all(node["offscreen"] is True for node in matched)
            self.progress.mark("hidden_observed", matches=len(matched), accepted=accepted)
            if accepted:
                # UIA's complete tree either omits hidden controls or reports
                # IsOffscreen=true. Both observations exclude an exposed menu.
                self.actions.append({"action": "observed hidden UIA", "ids": identifiers,
                                     "nodes": matched})
                return matched
            time.sleep(min(self.remaining(), UIA_POLL_MILLISECONDS / 1000))

    def replace(self, identifier: str, value: str, representation: TextValueRepresentation) -> dict:
        if not isinstance(representation, TextValueRepresentation):
            raise TypeError("actual UIA replacement requires an explicit text representation")
        expected = representation.observed(value)
        self.progress.operation("replace", (identifier,))
        observation = {"identifier": identifier, "requested": value,
                       "value_representation": representation.name, "expected_value": expected,
                       "focus_sequence": None, "focus_nodes": [],
                       "stages": {}, "selections": {}, "selection_sequences": {}}
        def observed(phase):
            observation["phase"] = phase
            observation["stages"][phase] = focus_observation.input_state(self.handle)
            self.progress.input(observation)

        def selection_observed(phase):
            observation["selections"][phase] = self.selection(identifier)
            observation["selection_sequences"][phase] = str(self.progress.sequence)

        observed("before_focus")
        nodes = self.request("focus", identifier)
        observation["focus_sequence"] = str(self.progress.sequence)
        observation["focus_nodes"] = [node for node in nodes if node["id"] == identifier]
        if self.focus_entry is focus.FocusEntry.NATIVE_PARENT:
            self.enter_host(identifier, observed=observed)
        selection_observed("before_select")
        observed("before_select")
        control, select = keyboard.physical_keys((0x11, 0x41))
        with control.held():
            with select.held():
                pass
            queue.flush_window(self.handle)
        selection_observed("after_select")
        _, units = keyboard.validate_text(value)
        observation["unicode_units"] = list(units)
        observed("before_unicode")
        keyboard.send_unicode_text(units)
        observed("after_unicode_inserted")
        queue.flush_window(self.handle)
        selection_observed("after_unicode")
        observed("after_sent_message_returned")
        self.actions.append({"action": "SendInput Unicode", "id": identifier, "value": value})
        return self.wait(identifier, lambda node: node["value"] == expected)

    def key(self, key: int) -> None:
        self.progress.operation("key", (str(key),))
        stroke, = keyboard.physical_keys((key,))
        with stroke.held():
            pass
        queue.flush_window(self.handle)
        self.progress.mark("key_completed")

    def shortcut(self, letter: str) -> None:
        self.progress.operation("shortcut", (letter,))
        observation = {"kind": "shortcut", "keys": ["Alt", "Shift", letter],
                       "strokes": [], "stages": {}}
        def observed(phase):
            observation["phase"] = phase
            observation["stages"][phase] = focus_observation.input_state(self.handle)
            self.progress.input(observation)

        observed("before_focus")
        self.enter_host()
        observed("before_mapping")
        *modifiers, stroke = keyboard.physical_keys((0x12, 0x10, ord(letter)))
        observation["strokes"] = [
            {"virtual_key": key.virtual_key, "scan_code": key.scan_code,
             "extended": key.extended} for key in (*modifiers, stroke)]
        observed("before_modifiers")
        with ExitStack() as held:
            for key in modifiers:
                held.enter_context(key.held())
            observed("modifiers_inserted")
            with stroke.held():
                pass
            observed("letter_inserted")
            queue.flush_window(self.handle)
            observed("after_sent_message_returned")
        observed("modifiers_released")
        self.actions.append({"action": "SendInput shortcut", "keys": ["Alt", "Shift", letter]})

    @physical_coordinates()
    def click(self, identifier: str) -> None:
        self.progress.operation("click", (identifier,))
        node = self.node(identifier)
        x, y, width, height = node["rect"]
        if node["offscreen"] or not node["enabled"] or width <= 0 or height <= 0:
            raise ValueError("actual pointer target is disabled, offscreen or has no rectangle")
        if not all(math.isfinite(value) for value in (x, y, width, height)):
            raise ValueError("actual pointer target has nonfinite UIA geometry")
        point = abi.Point(round(x + width / 2), round(y + height / 2))
        user = ctypes.WinDLL("user32", use_last_error=True)
        user.ScreenToClient.argtypes = [ctypes.c_void_p, ctypes.POINTER(abi.Point)]
        user.ScreenToClient.restype = ctypes.c_int32
        if not user.ScreenToClient(self.handle, ctypes.byref(point)):
            raise ctypes.WinError(ctypes.get_last_error())
        actual = pointer.send_pointer_click(self.handle, point.x, point.y)
        queue.flush_window(self.handle)
        self.actions.append({"action": "SendInput pointer", "id": identifier, "screen": actual})

    def close(self) -> None:
        self.progress.operation("close")
        self.process.stdin.close()
        self.progress.mark("actor_exit_wait")
        status = self.process.wait(timeout=self.remaining())
        self.process.stdout.close()
        self.progress.mark("actor_collected", returncode=status)
        if status != 0:
            raise RuntimeError(f"actual UIA actor exits with {status}")
