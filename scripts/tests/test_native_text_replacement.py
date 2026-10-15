"""Two actual production actor replacements through documented Rich Edit input.

The existing plain EDIT/focus and physical-key fixtures remain independent.
The window owner pumps messages while one worker calls the actual actor. Native
selection reads only observe the original procedure's Ctrl+A and Unicode work.
No SetValue, selection setter, synthetic input handler, or retyping is present.
One original 60-second deadline covers the actor's complete operation sequence.

https://learn.microsoft.com/en-us/windows/win32/controls/about-rich-edit-controls
"""

import ctypes
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import time
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native.pump import run_with_pump
from native.rich_edit import CharacterRange, RichEdit


@unittest.skipUnless(sys.platform == "win32", "real Rich Edit Unicode replacement requires Windows")
class NativeTextReplacement(unittest.TestCase):
    VALUES = ("患者-🦀-one", "患者-🦀-two")

    def retain(self, output, fixture, states):
        (output / "native-replacement.json").write_text(json.dumps({
            "schema": 1, "top": fixture.top, "target": fixture.edit,
            "seed": fixture.seed, "values": self.VALUES, "states": states,
            "maximum_events": fixture.maximum_events, "events": fixture.events,
            "observer_errors": [repr(error) for error in fixture.errors],
        # A real intermediate high-surrogate WM_CHAR can leave an unpaired
        # UTF-16 unit in native text. JSON escapes retain it without coercion.
        }, ensure_ascii=True, indent=2), encoding="utf-8")

    def assert_text(self, actual, expected):
        units = len(expected.encode("utf-16-le")) // 2
        self.assertEqual(actual["value"], expected)
        self.assertEqual(actual["units"], units)
        self.assertEqual(actual["copied_units"], units)

    def assert_observation(self, actor, expected, output, states):
        progress = json.loads((output / "uia-progress.json").read_bytes())
        observed = progress["input"]["observation"]
        self.assertEqual(observed["requested"], expected)
        self.assertEqual(observed["phase"], "after_sent_message_returned")
        self.assertEqual(observed["identifier"], states["identifier"])
        self.assertEqual(len(observed["focus_nodes"]), 1)
        self.assertTrue(observed["focus_nodes"][0]["focused"])
        phases = ("before_select", "after_select", "after_unicode")
        self.assertEqual(set(observed["selections"]), set(phases))
        sequences = [int(observed["selection_sequences"][phase]) for phase in phases]
        self.assertEqual(sequences, sorted(set(sequences)))
        self.assertGreater(sequences[0], int(observed["focus_sequence"]))
        self.assertLess(sequences[-1], int(progress["sequence"]))
        for phase in phases:
            node = observed["selections"][phase]
            self.assertEqual(node["id"], states["identifier"])
            self.assertEqual(type(node["selection"]["available"]), bool)
            # Availability is an observed provider capability, not a shortcut
            # for successful native input or an unconditional fixture premise.
            self.assertIn(node["selection"]["supported"], (None, "None", "Single", "Multiple"))
        units = list(memoryview(expected.encode("utf-16-le")).cast("H"))
        self.assertEqual(observed["unicode_units"], units)
        raw = (output / "uia-response.json").read_bytes()
        self.assertEqual(json.loads(raw)["nodes"], actor.latest)
        self.assertEqual(progress["last_response"]["bytes"], len(raw))
        self.assertEqual(progress["last_response"]["sha256"], hashlib.sha256(raw).hexdigest())
        return observed

    def assert_events(self, fixture, start, prior, expected):
        events = fixture.events[start:]
        if fixture.errors:
            raise fixture.errors[0]
        self.assertEqual(fixture.errors, [])
        selected = [event for event in events if event["message"] == 0x100 and event["word"] == 0x41]
        self.assertEqual(len(selected), 1, events)
        self.assertTrue(selected[0]["before"]["control_high_bit_set"])
        self.assert_text(selected[0]["before"]["text"], prior)
        characters = [event for event in events if event["message"] == 0x102 and event["word"] != 1]
        units = list(memoryview(expected.encode("utf-16-le")).cast("H"))
        self.assertEqual([event["word"] for event in characters], units)
        self.assertEqual(characters[0]["before"]["selection"], {
            "start": 0, "end": len(prior.encode("utf-16-le")) // 2})
        self.assertFalse(characters[0]["before"]["control_high_bit_set"])
        self.assert_text(characters[-1]["after"]["text"], expected)
        return events

    def test_real_actor_replaces_two_unicode_values_with_native_selection_evidence(self):
        from webview_acceptance.uia import Uia

        deadline = time.monotonic() + 60
        retained = Path(__file__).resolve().parents[2] / "output/browser/windows-uia-regression"
        retained.mkdir(parents=True, exist_ok=True)
        output = Path(tempfile.mkdtemp(prefix="metis-rich-edit-患者-🦀-", dir=retained))
        maximum_events = sum(len(value.encode("utf-16-le")) // 2 + 3 for value in self.VALUES)
        fixture = RichEdit("seed-α-" + str(2 ** 15), maximum_events)
        actor, primary = None, None
        states = {"identifier": None, "replacements": []}
        try:
            self.assertEqual(ctypes.sizeof(CharacterRange), 8)
            self.assertEqual((CharacterRange.start.offset, CharacterRange.end.offset), (0, 4))
            fixture.open()
            states["initial_native"] = fixture.text()
            self.assert_text(states["initial_native"], fixture.seed)
            actor = Uia(fixture.top, deadline, output)
            nodes = run_with_pump(fixture.user, actor, actor.request, "snapshot")
            matches = [node for node in nodes if node["value"] == fixture.seed]
            self.assertEqual(len(matches), 1, json.dumps(nodes, ensure_ascii=True))
            self.assertEqual(matches[0]["role"], "ControlType.Edit")
            self.assertEqual(matches[0]["value_source"], "ValuePattern")
            identifier = states["identifier"] = matches[0]["id"]
            self.assertTrue(identifier)
            prior = fixture.seed
            for expected in self.VALUES:
                start = len(fixture.events)
                actual = run_with_pump(fixture.user, actor, actor.replace, identifier, expected)
                self.assertEqual(actual["value"], expected)
                self.assertTrue(actual["focused"])
                native = fixture.text()
                self.assert_text(native, expected)
                observation = self.assert_observation(actor, expected, output, states)
                events = self.assert_events(fixture, start, prior, expected)
                states["replacements"].append({"value": expected, "native": native,
                                                "uia": actual, "input": observation,
                                                "events": events})
                self.retain(output, fixture, states)
                prior = expected
            self.assertEqual(len(states["replacements"]), 2)
            self.assertEqual([entry["value"] for entry in states["replacements"]], list(self.VALUES))
        except BaseException as error:
            primary = error
            error.add_note(f"actual Rich Edit actor evidence: {output!r}")
            raise
        finally:
            try:
                self.retain(output, fixture, states)
            except BaseException as error:
                if primary is None:
                    primary = error
                else:
                    primary.add_note(f"retaining actual Rich Edit evidence: {error!r}")
            if actor is not None:
                try:
                    run_with_pump(fixture.user, actor, actor.close)
                except BaseException as error:
                    if primary is None:
                        primary = error
                    else:
                        primary.add_note(f"closing actual Rich Edit actor: {error!r}")
            fixture.close(primary)
            if primary is not None and sys.exception() is None:
                raise primary


if __name__ == "__main__":
    unittest.main()
