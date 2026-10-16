"""Command-line parsing for the production native input capture workflow."""
from __future__ import annotations
import argparse
import json
import pathlib
import sys
from native_input import workflow


def parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Capture a production Windows window before and after Unicode input."
    )
    parser.add_argument("--command", type=pathlib.Path, required=True)
    parser.add_argument(
        "--argument",
        action="append",
        default=[],
        dest="command_arguments",
        help="one argument passed to --command; repeat for each argument",
    )
    parser.add_argument("--cwd", type=pathlib.Path)
    parser.add_argument(
        "--text",
        help="bounded Unicode text delivered through Win32 SendInput",
    )
    parser.add_argument(
        "--shortcut",
        choices=("enter", "control-enter", "alt-shift-c"),
        help="optional submit shortcut delivered after the Unicode text",
    )
    parser.add_argument(
        "--follow-shortcut",
        choices=("enter", "control-enter", "alt-shift-c"),
        help="optional second real shortcut, sent after the first settles",
    )
    parser.add_argument(
        "--follow-wait-seconds",
        type=float,
        default=1.0,
        help="bounded wait between the shortcut phases",
    )
    parser.add_argument(
        "--expected-clipboard",
        help="exact text the clipboard must carry after the journey closes",
    )
    parser.add_argument(
        "--click-client",
        nargs=2,
        type=int,
        metavar=("X", "Y"),
        dest="click_client",
        help="trusted OS pointer click at this 96-dpi logical client point",
    )
    parser.add_argument(
        "--scroll-client",
        nargs=3,
        type=int,
        metavar=("X", "Y", "NOTCHES"),
        dest="scroll_client",
        help="trusted OS wheel notches over this 96-dpi logical client point",
    )
    parser.add_argument(
        "--screen",
        action="store_true",
        help="read visible screen pixels for composition surfaces instead of WM_PRINT",
    )
    parser.add_argument("--initial-output", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--manifest", type=pathlib.Path, required=True)
    return parser


def main() -> None:
    if sys.platform != "win32":
        raise SystemExit("python_native_input_capture.py requires a Windows desktop")
    arguments = parser().parse_args()
    try:
        result = workflow.capture_input(
            arguments.command,
            arguments.command_arguments,
            arguments.cwd,
            arguments.text,
            arguments.shortcut,
            tuple(arguments.click_client) if arguments.click_client is not None else None,
            tuple(arguments.scroll_client) if arguments.scroll_client is not None else None,
            arguments.initial_output.resolve(),
            arguments.output.resolve(),
            arguments.manifest.resolve(),
            screen=arguments.screen,
            follow_shortcut=arguments.follow_shortcut,
            follow_wait_seconds=arguments.follow_wait_seconds,
            expected_clipboard=arguments.expected_clipboard,
        )
    except (OSError, RuntimeError, ValueError) as error:
        raise SystemExit(str(error)) from error
    print(json.dumps(result, sort_keys=True))
