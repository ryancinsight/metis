"""Value-semantic tests for the bounded WebGPU recovery runner."""
from __future__ import annotations

import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

from browser_gpu_recovery import _pixel_observation, _validate_gpu_trace
from browser_protocol import BrowserRuntimeError


def _pixels(left: tuple[int, ...], right: tuple[int, ...]) -> dict:
    row = list(left) * 16 + list(right) * 16
    return {"ok": True, "width": 32, "height": 32, "pixels": row * 32}


def _gpu_trace() -> dict:
    return {
        "adapter": {
            "vendor": "nvidia",
            "architecture": "blackwell",
            "device": "",
            "description": "",
            "is_fallback_adapter": False,
        },
        "devices": [
            {
                "id": 1,
                "lost": {"reason": "destroyed", "message": "Device was destroyed."},
                "uncaptured_errors": [],
            },
            {"id": 2, "lost": None, "uncaptured_errors": []},
        ],
        "configurations": [
            {"device_id": 1, "format": "bgra8unorm", "alpha_mode": "opaque"},
            {"device_id": 2, "format": "bgra8unorm", "alpha_mode": "opaque"},
        ],
        "uploads": [
            {"device_id": 1, "method": "copyExternalImageToTexture"},
            {"device_id": 2, "method": "copyExternalImageToTexture"},
        ],
        "errors": [],
        "overflow": False,
    }


class BrowserGpuRecoveryTests(unittest.TestCase):
    def test_exact_split_frame_is_hashed(self):
        observation = _pixel_observation(
            _pixels((255, 0, 0, 255), (0, 255, 0, 255)),
            (255, 0, 0, 255),
            (0, 255, 0, 255),
        )
        self.assertEqual(
            observation["rgba_sha256"],
            "531b42afea4ea2a6c8dac4c2bcb69b70d38b405f2256530effad6a4deb9991b9",
        )

    def test_one_wrong_component_fails_exact_oracle(self):
        value = _pixels((0, 0, 255, 255), (255, 255, 255, 255))
        value["pixels"][15 * 4] = 1
        with self.assertRaisesRegex(BrowserRuntimeError, r"pixel \(15, 0\)"):
            _pixel_observation(value, (0, 0, 255, 255), (255, 255, 255, 255))

    def test_gpu_trace_requires_distinct_recovery_activity(self):
        trace = _gpu_trace()
        self.assertIs(_validate_gpu_trace(trace), trace)
        trace = _gpu_trace()
        trace["devices"][0]["lost"]["reason"] = "unknown"
        with self.assertRaisesRegex(BrowserRuntimeError, "not observed as destroyed"):
            _validate_gpu_trace(trace)
        trace = _gpu_trace()
        trace["errors"].append({"kind": "uncapturederror"})
        with self.assertRaisesRegex(BrowserRuntimeError, "recorded errors"):
            _validate_gpu_trace(trace)

    def test_missing_generation_and_post_loss_upload_are_rejected(self):
        for name in ("configurations", "uploads"):
            for identities in ([1, 1], [2, 1], [1, 1, 2], [1]):
                with self.subTest(name=name, identities=identities):
                    trace = _gpu_trace()
                    trace[name] = [{"device_id": identity} for identity in identities]
                    with self.assertRaisesRegex(BrowserRuntimeError, "must cover devices"):
                        _validate_gpu_trace(trace)

    def test_lost_replacement_and_device_errors_are_rejected(self):
        trace = _gpu_trace()
        trace["devices"][1]["lost"] = {"reason": "destroyed"}
        with self.assertRaisesRegex(BrowserRuntimeError, "replacement device was lost"):
            _validate_gpu_trace(trace)
        trace = _gpu_trace()
        trace["devices"][1]["uncaptured_errors"] = [{"name": "GPUValidationError"}]
        with self.assertRaisesRegex(BrowserRuntimeError, "uncaptured errors"):
            _validate_gpu_trace(trace)


if __name__ == "__main__":
    unittest.main()
