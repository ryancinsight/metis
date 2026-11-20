"""Test the bounded WebDriver health protocol."""
import io
import json
import pathlib
import sys
import unittest
import urllib.error
from unittest import mock

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

from browser_protocol import MAX_TRACE_BYTES, BrowserRuntimeError
from webdriver_health import wait_until_ready


class Response:
    """Provide the bounded response surface consumed by the health probe."""

    def __init__(self, value: object, status: int = 200) -> None:
        self.status = status
        self.body = io.BytesIO(json.dumps(value).encode("utf-8"))

    def __enter__(self):
        return self

    def __exit__(self, *unused) -> None:
        del unused

    def read(self, limit: int) -> bytes:
        return self.body.read(limit)


class WebDriverHealthTests(unittest.TestCase):
    """Reject malformed status and stop at the first ready response."""

    @mock.patch("webdriver_health.time.sleep")
    @mock.patch("webdriver_health.time.monotonic", return_value=0.0)
    @mock.patch("webdriver_health.urllib.request.urlopen")
    def test_transient_failure_and_not_ready_precede_success(self, urlopen, monotonic, sleep):
        del monotonic
        urlopen.side_effect = [
            urllib.error.URLError("starting"),
            Response({"value": {"ready": False}}),
            Response({"value": {}}),
            Response({"value": {"ready": "true"}}),
            Response({"value": {"ready": True}}),
        ]
        wait_until_ready("http://127.0.0.1:9515", 1.0)
        self.assertEqual(urlopen.call_count, 5)
        self.assertEqual(sleep.call_count, 4)

    def test_invalid_endpoint_and_timeout_are_typed(self):
        with self.assertRaisesRegex(BrowserRuntimeError, r"HTTP\(S\)"):
            wait_until_ready("file:///driver", 1.0)
        with (
            mock.patch(
                "webdriver_health.time.monotonic",
                side_effect=[0.0, 0.0, 0.0, 1.0, 1.0],
            ),
            mock.patch(
                "webdriver_health.urllib.request.urlopen",
                side_effect=urllib.error.URLError("refused"),
            ),
            self.assertRaisesRegex(BrowserRuntimeError, "refused"),
        ):
            wait_until_ready("http://127.0.0.1:9515", 1.0)

    @mock.patch("webdriver_health.time.sleep")
    @mock.patch(
        "webdriver_health.time.monotonic",
        side_effect=[0.0, 0.0, 0.0, 1.0, 1.0],
    )
    @mock.patch("webdriver_health.urllib.request.urlopen")
    def test_oversized_status_response_is_rejected(self, urlopen, monotonic, sleep):
        del monotonic, sleep
        response = Response({"value": {"ready": True}})
        response.body = io.BytesIO(b"x" * (MAX_TRACE_BYTES + 1))
        urlopen.return_value = response
        with self.assertRaisesRegex(BrowserRuntimeError, "trace bound"):
            wait_until_ready("http://127.0.0.1:9515", 1.0)


if __name__ == "__main__":
    unittest.main()
