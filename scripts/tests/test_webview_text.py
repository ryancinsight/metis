"""OS text and provider-owned paragraph representation remain separate."""

from pathlib import Path
import struct
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_input.windows.keyboard import MAX_TEXT_BYTES, validate_text
from webview_acceptance.text import TextValueRepresentation


class TextValueProjection(unittest.TestCase):
    def test_literal_and_story_values_preserve_every_original_input_unit(self):
        for value in ("seed-α-32768", "患者-🦀-one", "患者-🦀-two", "a" * MAX_TEXT_BYTES):
            with self.subTest(value=value):
                encoded, units = validate_text(value)
                self.assertEqual(encoded, value.encode("utf-8"))
                self.assertEqual(struct.pack(f"<{len(units)}H", *units), value.encode("utf-16-le"))
                literal = TextValueRepresentation.CONTROL.observed(value)
                story = TextValueRepresentation.RICH_EDIT_STORY.observed(value)
                self.assertEqual(literal, value)
                self.assertEqual(story, value + "\r")
                self.assertEqual(story.encode("utf-16-le"), literal.encode("utf-16-le") + b"\x0d\x00")
                self.assertEqual(len(story.encode("utf-16-le")) // 2, len(units) + 1)
                self.assertEqual(validate_text(value), (encoded, units))

    def test_provider_marker_is_never_admitted_as_committed_os_input(self):
        for value in ("seed-α-32768", "患者-🦀-one", "患者-🦀-two"):
            with self.subTest(value=value):
                story = TextValueRepresentation.RICH_EDIT_STORY.observed(value)
                with self.assertRaisesRegex(ValueError, "control character") as caught:
                    validate_text(story)
                self.assertEqual(str(caught.exception), "--text contains a control character")
                self.assertEqual(validate_text(value)[0], value.encode("utf-8"))


if __name__ == "__main__":
    unittest.main()
