"""Build the Rust Fetch probe and serve analytical bytes over real HTTP."""
from __future__ import annotations

import argparse
import http.server
import json
import pathlib
import subprocess
import threading
import time

import browser

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "output" / "browser" / "http-transport"
BOUND = 16 * 1024
ORIGIN = "http://127.0.0.1:8080"
PORT = 8767


def build() -> None:
    result = browser.cargo([
        "build", "--locked", "-p", "metis-web", "--example", "http_transport",
        "--target", "wasm32-unknown-unknown", "--release", "--message-format=json",
    ], capture_output=True, text=True)
    artifacts = []
    for line in result.stdout.splitlines():
        record = json.loads(line)
        if record.get("reason") == "compiler-artifact" and record["target"]["name"] == "http_transport":
            artifacts.extend(pathlib.Path(name) for name in record["filenames"] if name.endswith(".wasm"))
    if len(artifacts) != 1 or not artifacts[0].is_file():
        raise ValueError("Rust must produce exactly one HTTP probe WASM artifact")
    OUTPUT.mkdir(parents=True, exist_ok=True)
    subprocess.run([
        browser.wasm_bindgen(), str(artifacts[0]), "--target", "web",
        "--out-name", "http_transport", "--out-dir", str(OUTPUT),
    ], check=True, timeout=60)
    policy = browser.content_security_policy().replace("127.0.0.1:8766", "127.0.0.1:8767")
    (OUTPUT / "index.html").write_text(
        '<!doctype html><html lang="en"><head><meta charset="utf-8">'
        '<meta name="viewport" content="width=device-width, initial-scale=1">'
        f'<meta http-equiv="Content-Security-Policy" content="{policy}">'
        '<title>Rust HTTP transport verification</title>'
        '<link rel="stylesheet" href="../styles.css"></head><body data-metis-theme="dark">'
        '<main id="metis-app"><h1>Rust HTTP transport verification</h1>'
        '<p id="transport-status" role="status">Reading real HTTP streams…</p></main>'
        '<script type="module" src="./start.js"></script></body></html>', encoding="utf-8",
    )
    (OUTPUT / "start.js").write_text(
        'import init from "./http_transport.js";\n\n(await init()).metis_http_transport_start();\n',
        encoding="utf-8",
    )


class Peer(http.server.BaseHTTPRequestHandler):
    """Independent producer: bytes and lifecycle facts come from actual I/O."""
    protocol_version = "HTTP/1.1"
    abort_started = threading.Event()
    abort_closed = threading.Event()
    counts_lock = threading.Lock()
    redirected = 0
    followed = 0

    def headers_for(self, status: int = 200, **headers: str) -> None:
        self.send_response(status)
        self.send_header("Access-Control-Allow-Origin", ORIGIN)
        self.send_header("Content-Type", "application/octet-stream")
        self.send_header("Connection", "close")
        for name, value in headers.items():
            self.send_header(name.replace("_", "-"), value)
        self.end_headers()

    def reply(self, body: bytes, status: int = 200) -> None:
        self.headers_for(status, Content_Length=str(len(body)))
        self.wfile.write(body)

    def do_POST(self) -> None:
        if self.path != "/echo" or self.headers.get("Origin") != ORIGIN:
            self.reply(b"denied\n", 403)
            return
        length = int(self.headers.get("Content-Length", "-1"))
        if not 0 <= length <= BOUND:
            self.reply(b"body bound\n", 413)
            return
        body = self.rfile.read(length)
        self.reply(body if len(body) == length else b"truncated\n", 200 if len(body) == length else 400)

    def do_OPTIONS(self) -> None:
        self.send_response(204)
        self.send_header("Access-Control-Allow-Origin", ORIGIN)
        self.send_header("Access-Control-Allow-Methods", "GET, POST")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.send_header("Content-Length", "0")
        self.end_headers()

    def do_GET(self) -> None:
        if self.headers.get("Origin") != ORIGIN:
            self.reply(b"denied\n", 403)
            return
        try:
            self.exchange()
        except (BrokenPipeError, ConnectionResetError):
            # A closed socket is the intended observation for cancelled reads.
            if self.path == "/abort":
                type(self).abort_closed.set()

    def exchange(self) -> None:
        if self.path.startswith("/bodyless/"):
            status = int(self.path.removeprefix("/bodyless/"))
            if status not in (204, 205, 304):
                self.reply(b"fixture status\n", 400)
                return
            # A 304 length describes the selected representation, not received
            # body bytes (RFC 9110 §8.6). The body remains absent on the wire.
            if status == 204:
                self.headers_for(status)
            else:
                length = BOUND + 1 if status == 304 else 0
                self.headers_for(status, Content_Length=str(length))
        elif self.path.startswith("/bytes/"):
            size = int(self.path.removeprefix("/bytes/"))
            if not 0 <= size <= BOUND + 1:
                self.reply(b"fixture bound\n", 400)
                return
            self.headers_for(Transfer_Encoding="chunked")
            # Exact bound and excess cross two chunk boundaries; no Content-
            # Length header can substitute for counting the decoded stream.
            for start in range(0, size, BOUND // 2):
                body = bytes(index % 251 for index in range(start, min(size, start + BOUND // 2)))
                self.wfile.write(f"{len(body):x}\r\n".encode() + body + b"\r\n")
                self.wfile.flush()
            self.wfile.write(b"0\r\n\r\n")
        elif self.path == "/declared-excess":
            self.headers_for(Content_Length=str(BOUND + 1))
        elif self.path == "/redirect":
            with self.counts_lock:
                type(self).redirected += 1
            self.headers_for(302, Location="/followed", Content_Length="0")
        elif self.path == "/followed":
            with self.counts_lock:
                type(self).followed += 1
            self.reply(b"followed\n")
        elif self.path == "/redirect-result":
            with self.counts_lock:
                body = f"redirected={self.redirected};followed={self.followed}\n".encode()
            self.reply(body)
        elif self.path == "/slow-headers":
            time.sleep(0.5)
            self.reply(b"late\n")
        elif self.path == "/slow-body":
            self.headers_for(Content_Length="5")
            self.wfile.flush()
            time.sleep(0.5)
            self.wfile.write(b"late\n")
        elif self.path == "/abort":
            self.headers_for(Content_Length=str(BOUND))
            self.wfile.write(b"a")
            self.wfile.flush()
            self.abort_started.set()
            self.connection.settimeout(5)
            if self.connection.recv(1) == b"":
                self.abort_closed.set()
        elif self.path == "/abort-ready":
            self.reply(b"ready\n" if self.abort_started.wait(4) else b"missing\n")
        elif self.path == "/abort-result":
            self.reply(b"closed\n" if self.abort_closed.wait(4) else b"open\n")
        else:
            self.reply(b"unknown route\n", 404)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["build", "serve"])
    arguments = parser.parse_args()
    if arguments.command == "build":
        build()
    else:
        with http.server.ThreadingHTTPServer(("127.0.0.1", PORT), Peer) as server:
            server.serve_forever()


if __name__ == "__main__":
    main()
