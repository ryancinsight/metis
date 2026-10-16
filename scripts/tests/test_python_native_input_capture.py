"""Value-semantic contracts for the native Unicode input capture utility."""

from __future__ import annotations

import ctypes
import pathlib
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
from native_input import cli, frames, workflow
from native_input.windows import abi, focus, geometry, keyboard, pointer


class NativeInputCaptureTests(unittest.TestCase):
    def test_input_structure_keeps_the_64_bit_windows_abi_width(self) -> None:
        self.assertEqual(ctypes.sizeof(abi.KeyboardInput), 24)
        self.assertEqual(ctypes.sizeof(abi.InputUnion), 32)
        self.assertEqual(ctypes.sizeof(abi.Input), 40)

    def test_text_validation_returns_utf8_size_and_utf16_units(self) -> None:
        encoded, units = keyboard.validate_text("東京😀")
        self.assertEqual(len(encoded), len("東京😀".encode("utf-8")))
        self.assertEqual(
            units,
            tuple(int.from_bytes("東京😀".encode("utf-16-le")[offset : offset + 2], "little")
                  for offset in range(0, len("東京😀".encode("utf-16-le")), 2)),
        )

    def test_text_validation_rejects_empty_control_and_oversized_input(self) -> None:
        with self.assertRaisesRegex(ValueError, "must not be empty"):
            keyboard.validate_text("")
        with self.assertRaisesRegex(ValueError, "control character"):
            keyboard.validate_text("patient\n")
        with self.assertRaisesRegex(ValueError, "128-byte bound"):
            keyboard.validate_text("界" * 43)

    def test_parser_keeps_command_arguments_and_outputs_distinct(self) -> None:
        parsed = cli.parser().parse_args(
            [
                "--command",
                "metis-app.exe",
                "--argument=--metis-native-window",
                "--argument",
                "value with spaces",
                "--text",
                "東京",
                "--shortcut",
                "control-enter",
                "--initial-output",
                "before.png",
                "--output",
                "after.png",
                "--manifest",
                "trace.json",
            ]
        )
        self.assertEqual(
            parsed.command_arguments,
            ["--metis-native-window", "value with spaces"],
        )
        self.assertEqual(parsed.initial_output, pathlib.Path("before.png"))
        self.assertEqual(parsed.output, pathlib.Path("after.png"))
        self.assertEqual(parsed.shortcut, "control-enter")
        self.assertEqual(parsed.manifest, pathlib.Path("trace.json"))

    def test_physical_records_preserve_scan_prefix_and_release_bits(self) -> None:
        # MAPVK_VK_TO_VSC_EX encodes E0 in the high byte. KEYBDINPUT uses
        # SCANCODE=8, EXTENDEDKEY=1 and KEYUP=2; wVk is ignored and stays zero.
        for virtual_key, mapped, scan, flags in (
            (0x41, 0x001E, 0x1E, 0x08),
            (0x27, 0xE04D, 0x4D, 0x09),
            (0x27, 0x004D, 0x4D, 0x08),
            (0x11, 0x001D, 0x1D, 0x08),
            (0x0D, 0x001C, 0x1C, 0x08),
        ):
            with self.subTest(virtual_key=virtual_key, mapped=mapped):
                stroke = keyboard.PhysicalKey.from_mapping(virtual_key, mapped)
                self.assertEqual((stroke.virtual_key, stroke.scan_code, stroke.extended),
                                 (virtual_key, scan, flags == 0x09))
                for key_up in (False, True):
                    record = stroke.record(key_up=key_up)
                    self.assertEqual((record.kind, record.keyboard.virtual_key,
                                      record.keyboard.scan_code, record.keyboard.flags,
                                      record.keyboard.time, record.keyboard.extra_info),
                                     (1, 0, scan, flags | (2 if key_up else 0), 0, 0))

    def test_physical_mapping_rejects_absent_scan_and_unsupported_prefix(self) -> None:
        for mapped in (0, 0xE000, 0xE11D, 0xE21D, 0x1001E, -1):
            with self.subTest(mapped=mapped), self.assertRaises(ValueError):
                keyboard.PhysicalKey.from_mapping(0x41, mapped)

    def test_physical_key_rejects_out_of_domain_record_fields(self) -> None:
        for virtual_key, scan, extended in (
            (0, 0x1E, False), (256, 0x1E, False), (True, 0x1E, False),
            (0x41, 0, False), (0x41, 256, False), (0x41, True, False),
            (0x41, 0x1E, 1),
        ):
            with self.subTest(fields=(virtual_key, scan, extended)), self.assertRaises((TypeError, ValueError)):
                keyboard.PhysicalKey(virtual_key, scan, extended)
        with self.assertRaises(TypeError):
            keyboard.physical_keys((0x41,), recipient="focused")
        with self.assertRaises(ValueError):
            keyboard.physical_keys(())

    def test_mouse_input_keeps_the_64_bit_windows_abi_width(self) -> None:
        # LONG dx, LONG dy, DWORD mouseData, DWORD dwFlags, DWORD time, then
        # four alignment bytes before the 64-bit ULONG_PTR dwExtraInfo.
        self.assertEqual(ctypes.sizeof(abi.MouseInput), 32)
        self.assertEqual(ctypes.sizeof(abi.Point), 8)
        self.assertEqual(ctypes.sizeof(abi.InputUnion), 32)
        self.assertEqual(ctypes.sizeof(abi.Input), 40)

    def test_click_validation_bounds_each_coordinate(self) -> None:
        self.assertEqual(geometry.validate_click(512, 616), (512, 616))
        with self.assertRaisesRegex(ValueError, "0..16384"):
            geometry.validate_click(-1, 10)
        with self.assertRaisesRegex(ValueError, "0..16384"):
            geometry.validate_click(10, geometry.MAX_CLICK_COORDINATE + 1)

    def test_absolute_point_normalizes_onto_the_virtual_desktop_grid(self) -> None:
        self.assertEqual(geometry.absolute_point(0, 0, (0, 0, 1024, 768)), (0, 0))
        self.assertEqual(
            geometry.absolute_point(1023, 767, (0, 0, 1024, 768)), (65535, 65535)
        )
        self.assertEqual(geometry.absolute_point(1, 1, (0, 0, 3, 3)), (32768, 32768))
        self.assertEqual(geometry.absolute_point(-1920, 0, (-1920, 0, 3840, 2160)), (0, 0))
        self.assertEqual(geometry.absolute_point(-50, -50, (0, 0, 1024, 768)), (0, 0))

    def test_blank_frame_reads_the_center_not_the_window_chrome(self) -> None:
        uniform = b"\xff\xff\xff\xff" * 64
        self.assertTrue(frames.blank_frame(b"", 8, 8))
        self.assertTrue(frames.blank_frame(uniform, 8, 8))
        # A changed corner pixel is chrome; the client center is still blank.
        chrome = bytearray(uniform)
        chrome[0:4] = b"\x00\x00\x00\xff"
        self.assertTrue(frames.blank_frame(bytes(chrome), 8, 8))
        # A changed center pixel is rendered client content.
        content = bytearray(uniform)
        center = (4 * 8 + 4) * 4
        content[center : center + 4] = b"\x00\x00\x00\xff"
        self.assertFalse(frames.blank_frame(bytes(content), 8, 8))

    def test_capture_requires_at_least_one_trusted_input(self) -> None:
        with self.assertRaisesRegex(
            ValueError, "provide --text, --shortcut, --click-client, or --scroll-client"
        ):
            workflow.capture_input(
                pathlib.Path(sys.executable),
                [],
                None,
                None,
                None,
                None,
                None,
                pathlib.Path("before.png"),
                pathlib.Path("after.png"),
                pathlib.Path("trace.json"),
            )

    def test_wheel_data_masks_the_signed_delta_for_the_unsigned_field(self) -> None:
        self.assertEqual(pointer.wheel_data(1), 120)
        self.assertEqual(pointer.wheel_data(-1), 0xFFFFFF88)

    def test_click_records_position_only_in_the_move_record(self) -> None:
        move, press, release = pointer.click_records(10926, 24400)
        self.assertEqual(
            move.mouse.flags,
            abi.MOUSEEVENTF_ABSOLUTE
            | abi.MOUSEEVENTF_VIRTUALDESK
            | 0x0001,
        )
        self.assertEqual((move.mouse.dx, move.mouse.dy), (10926, 24400))
        # A button record without MOUSEEVENTF_ABSOLUTE treats dx and dy as
        # relative movement, which would deliver the click away from the
        # point the move record claimed.
        self.assertEqual(press.mouse.flags, abi.MOUSEEVENTF_LEFTDOWN)
        self.assertEqual((press.mouse.dx, press.mouse.dy), (0, 0))
        self.assertEqual(release.mouse.flags, abi.MOUSEEVENTF_LEFTUP)
        self.assertEqual((release.mouse.dx, release.mouse.dy), (0, 0))

    def test_scroll_validation_bounds_notches(self) -> None:
        self.assertEqual(geometry.validate_scroll(512, 600, 12), (512, 600, 12))
        with self.assertRaisesRegex(ValueError, "nonzero"):
            geometry.validate_scroll(512, 600, 0)
        with self.assertRaisesRegex(ValueError, "within"):
            geometry.validate_scroll(512, 600, geometry.MAX_SCROLL_NOTCHES + 1)

    def test_parser_accepts_click_client_without_text(self) -> None:
        parsed = cli.parser().parse_args(
            [
                "--command",
                "metis-app.exe",
                "--click-client",
                "512",
                "616",
                "--initial-output",
                "before.png",
                "--output",
                "after.png",
                "--manifest",
                "trace.json",
            ]
        )
        self.assertIsNone(parsed.text)
        self.assertEqual(parsed.click_client, [512, 616])
        self.assertFalse(parsed.screen)

    def test_parser_accepts_screen_capture_for_composition_surfaces(self) -> None:
        parsed = cli.parser().parse_args(
            [
                "--command",
                "metis-app.exe",
                "--shortcut",
                "enter",
                "--screen",
                "--initial-output",
                "before.png",
                "--output",
                "after.png",
                "--manifest",
                "trace.json",
            ]
        )
        self.assertTrue(parsed.screen)

    def test_paths_reject_colliding_outputs_and_wrong_extensions(self) -> None:
        with self.assertRaisesRegex(ValueError, "distinct"):
            workflow.validate_paths(
                pathlib.Path(sys.executable),
                pathlib.Path("capture.png"),
                pathlib.Path("capture.png"),
                pathlib.Path("trace.json"),
            )
        with self.assertRaisesRegex(ValueError, r"\.bmp or \.png"):
            workflow.validate_paths(
                pathlib.Path(sys.executable),
                pathlib.Path("before.gif"),
                pathlib.Path("after.png"),
                pathlib.Path("trace.json"),
            )
    def test_content_child_picks_the_largest_surface_descendant(self) -> None:
        # A hosted browser window: small chrome children plus the render
        # surface that owns the keyboard.
        chosen = focus.pick_content_child(((11, 1200), (22, 502_848)), 502_848)
        self.assertEqual(chosen, 22)

    def test_content_child_rejects_descendants_too_small_for_the_surface(self) -> None:
        # A window that paints its own content has no embedded surface.
        chosen = focus.pick_content_child(((11, 1200), (22, 3072)), 502_848)
        self.assertIsNone(chosen)

    def test_content_child_never_selects_without_candidates(self) -> None:
        self.assertIsNone(focus.pick_content_child((), 502_848))

    def test_content_child_rejects_a_degenerate_client_area(self) -> None:
        self.assertIsNone(focus.pick_content_child(((11, 1200),), 0))

    def test_after_settle_requires_a_repeating_frame(self) -> None:
        # A frame still changing is not evidence of a settled response.
        self.assertFalse(
            frames.after_settled(
                changed=True,
                repeated=False,
                observed_seconds=9.0,
                minimum_seconds=5.0,
            )
        )

    def test_after_settle_captures_a_changed_response_once_it_holds(self) -> None:
        # The response arrived early and now repeats; no need to wait out
        # the minimum window.
        self.assertTrue(
            frames.after_settled(
                changed=True,
                repeated=True,
                observed_seconds=1.2,
                minimum_seconds=5.0,
            )
        )

    def test_after_settle_waits_out_the_window_on_an_unchanged_surface(self) -> None:
        # A frame that merely holds still must not end the observation
        # before the asynchronous response window has passed.
        self.assertFalse(
            frames.after_settled(
                changed=False,
                repeated=True,
                observed_seconds=1.2,
                minimum_seconds=5.0,
            )
        )
        self.assertTrue(
            frames.after_settled(
                changed=False,
                repeated=True,
                observed_seconds=5.0,
                minimum_seconds=5.0,
            )
        )


if __name__ == "__main__":
    unittest.main()
