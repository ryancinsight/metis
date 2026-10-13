"""Public capture CLI and isolated wheel-child launch contract."""
from __future__ import annotations
import argparse
import importlib
import json
import pathlib
import subprocess
import sys
import tempfile
from typing import Any
from native_capture import commands, presentation
from native_capture.images import png
from python_binding import extract_wheel, validate_wheel_surface


ENTRY_SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "python_native_capture.py"


DEFAULT_WIDTH = 320


DEFAULT_HEIGHT = 240


def parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Capture a visible NativeApplication frame as a Windows BMP or PNG."
    )
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--wheel", type=pathlib.Path)
    source.add_argument("--site", type=pathlib.Path, help=argparse.SUPPRESS)
    source.add_argument(
        "--command",
        type=pathlib.Path,
        help="launch one visible native process and capture its first window",
    )
    parser.add_argument(
        "--argument",
        action="append",
        default=[],
        dest="command_arguments",
        help="one argument passed to --command; repeat for each argument",
    )
    parser.add_argument(
        "--cwd",
        type=pathlib.Path,
        help="working directory for --command",
    )
    parser.add_argument(
        "--frame",
        type=pathlib.Path,
        help="an existing RGBA PNG frame to present through NativeApplication",
    )
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--title", default="Metis Python native capture")
    parser.add_argument("--width", type=int)
    parser.add_argument("--height", type=int)
    parser.add_argument(
        "--resize",
        nargs=2,
        type=int,
        metavar=("WIDTH", "HEIGHT"),
        help="resize a --command window to this client size before the second capture",
    )
    parser.add_argument(
        "--resize-output",
        type=pathlib.Path,
        help="output path for the post-resize capture; requires --resize",
    )
    parser.add_argument(
        "--move-monitor",
        type=int,
        metavar="INDEX",
        help="move a --command window to the sorted physical monitor index",
    )
    parser.add_argument(
        "--monitor-output",
        type=pathlib.Path,
        help="output path for the post-monitor-move capture; requires --move-monitor",
    )
    parser.add_argument(
        "--require-dpi-change",
        action="store_true",
        help="fail unless moving the window changes its effective DPI",
    )
    return parser


def load_site(site: pathlib.Path) -> Any:
    sys.path.insert(0, str(site))
    return importlib.import_module("metis")


def main() -> None:
    """Capture one visible frame from a supplied wheel or native command."""
    arguments = parser().parse_args()
    if (arguments.width is None) != (arguments.height is None):
        raise SystemExit("--width and --height must be supplied together")
    if arguments.width is not None and (arguments.width <= 0 or arguments.height <= 0):
        raise SystemExit("width and height must be positive")
    if arguments.command is None and (
        arguments.command_arguments or arguments.cwd is not None
    ):
        raise SystemExit("--argument and --cwd require --command")
    if arguments.frame is not None and arguments.command is not None:
        raise SystemExit("--frame cannot be combined with --command")
    # Usage errors are reported on every host; only a capture needs Windows.
    if sys.platform != "win32":
        raise SystemExit("python_native_capture.py requires a Windows desktop")
    resize = None if arguments.resize is None else tuple(arguments.resize)
    try:
        commands.validate_resize_options(arguments.command, resize, arguments.resize_output, arguments.output)
        commands.validate_monitor_options(
            arguments.command,
            arguments.move_monitor,
            arguments.monitor_output,
            arguments.require_dpi_change,
            arguments.output,
        )
        if resize is not None and arguments.move_monitor is not None:
            raise ValueError("--resize and --move-monitor are mutually exclusive")
    except ValueError as error:
        raise SystemExit(str(error)) from error
    frame = None if arguments.frame is None else png.read_png(arguments.frame)
    if frame is None:
        width = DEFAULT_WIDTH if arguments.width is None else arguments.width
        height = DEFAULT_HEIGHT if arguments.height is None else arguments.height
    else:
        width, height = frame.width, frame.height
        if arguments.width is not None and (arguments.width, arguments.height) != (width, height):
            raise SystemExit("--width and --height must match the supplied frame")
    output = arguments.output.resolve()
    if arguments.site is not None:
        if not arguments.site.is_dir():
            raise SystemExit(f"Python wheel site does not exist: {arguments.site}")
        metis = load_site(arguments.site.resolve())
        result = presentation.capture(metis, arguments.title, width, height, output, frame)
        print(json.dumps(result, sort_keys=True))
        return

    if arguments.command is not None:
        if arguments.wheel is not None or arguments.site is not None:
            raise SystemExit("--command cannot be combined with a wheel or site")
        result = commands.capture_command(
            arguments.command,
            arguments.command_arguments,
            arguments.cwd,
            output,
            resize,
            None if arguments.resize_output is None else arguments.resize_output.resolve(),
            arguments.move_monitor,
            None if arguments.monitor_output is None else arguments.monitor_output.resolve(),
            arguments.require_dpi_change,
        )
        print(json.dumps(result, sort_keys=True))
        return

    wheel = arguments.wheel.resolve()
    if not wheel.is_file() or wheel.suffix != ".whl":
        raise SystemExit(f"wheel does not exist: {wheel}")
    with tempfile.TemporaryDirectory(prefix="metis-python-native-") as temporary:
        site = pathlib.Path(temporary)
        validate_wheel_surface(wheel)
        extract_wheel(wheel, site)
        command = [
            sys.executable,
            str(ENTRY_SCRIPT),
            "--site",
            str(site),
            "--output",
            str(output),
            "--title",
            arguments.title,
            "--width",
            str(width),
            "--height",
            str(height),
        ]
        if arguments.frame is not None:
            command.extend(("--frame", str(arguments.frame.resolve(strict=True))))
        completed = subprocess.run(command, check=False, text=True, capture_output=True)
        if completed.returncode != 0:
            if completed.stdout:
                print(completed.stdout, end="")
            if completed.stderr:
                print(completed.stderr, file=sys.stderr, end="")
            raise SystemExit(completed.returncode)
        print(completed.stdout, end="")
