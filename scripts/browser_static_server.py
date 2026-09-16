"""Keep one bounded static browser server alive for a hosted trace."""
from __future__ import annotations

import argparse
import os
import pathlib
import signal
import tempfile
import threading
import urllib.parse

from browser_protocol import BrowserRuntimeError, StaticServer


def publish_port(path: pathlib.Path, origin: str) -> None:
    """Atomically publish the loopback port selected by ``StaticServer``."""
    port = urllib.parse.urlsplit(origin).port
    if port is None or not 1 <= port <= 65_535:
        raise BrowserRuntimeError("static server selected an invalid loopback port")
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(descriptor, "w", encoding="ascii") as stream:
            stream.write(str(port))
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
    except BaseException:
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass
        raise


def serve(directory: pathlib.Path, port_file: pathlib.Path) -> None:
    """Serve generated assets until the host sends a bounded shutdown signal."""
    stopped = threading.Event()

    def stop(_signum: int, _frame: object) -> None:
        stopped.set()

    signal.signal(signal.SIGINT, stop)
    signal.signal(signal.SIGTERM, stop)
    with StaticServer(directory) as origin:
        publish_port(port_file, origin)
        stopped.wait()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=pathlib.Path, required=True)
    parser.add_argument("--port-file", type=pathlib.Path, required=True)
    arguments = parser.parse_args()
    serve(arguments.directory, arguments.port_file)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
