"""Present pixels through the Python host while pumping its owner thread."""
from __future__ import annotations
import os
import pathlib
from concurrent.futures import Future, ThreadPoolExecutor
from typing import Any
from native_capture import limits, model
from native_capture.images import files
from native_capture.windows import gdi, window


def checkerboard(width: int, height: int) -> bytes:
    red = bytes((229, 62, 62, 255)) * (width // 2)
    blue = bytes((49, 130, 206, 255)) * (width - width // 2)
    return b"".join((red + blue) if row % 2 == 0 else (blue + red) for row in range(height))


def capture(
    metis: Any,
    title: str,
    width: int,
    height: int,
    output: pathlib.Path,
    frame: model.Frame | None = None,
) -> dict[str, Any]:
    host = metis.NativeApplication(title, width, height, "visible")
    generation = host.generation
    executor = ThreadPoolExecutor(max_workers=1)
    future: Future[bytes] | None = None
    try:
        presented = checkerboard(width, height) if frame is None else frame.rgba
        host.present(generation, presented)
        events = host.wait_events(generation, 0)
        bounds = window.window_for_processes({os.getpid()})
        future = executor.submit(gdi.capture_window, bounds)
        for _ in range(limits.MAX_PUMP_ROUNDS):
            if future.done():
                break
            host.wait_events(generation, limits.EVENT_WAIT_MILLISECONDS)
        if future is None or not future.done():
            raise RuntimeError("native capture did not complete within the pump bound")
        pixels = future.result()
        digest = files.write_capture(output, bounds, pixels)
        result = {
            "title": title,
            "generation": generation,
            "window": {"width": bounds.width, "height": bounds.height},
            "events": events,
            "image": output.as_posix(),
            "sha256": digest,
        }
        if frame is None:
            result["input"] = {
                "format": "rgba",
                "width": width,
                "height": height,
                "source": "checkerboard",
            }
        else:
            result["input"] = {
                "format": "png-rgba",
                "width": frame.width,
                "height": frame.height,
                "sha256": frame.sha256,
            }
        return result
    finally:
        if future is not None and not future.done():
            future.cancel()
        executor.shutdown(wait=True, cancel_futures=True)
        host.close(generation)
