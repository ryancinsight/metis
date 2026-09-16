"""Value-semantic tests for the matched resource comparison contract."""
import json
import os
import pathlib
import runpy
import tempfile
import unittest


SCRIPTS = pathlib.Path(__file__).resolve().parents[1]


def _record(*, phase="lifecycle", uid="series-1", panels=None, mean=100.0):
    """Build a small provenance record with the production schema shape."""
    panels = ["axial", "coronal"] if panels is None else panels
    metrics = {
        "peak_private_bytes": mean,
        "peak_working_set_bytes": mean / 2,
        "duration_ms": 20.0,
        "startup_observation_ms": 2.0,
    }
    return {
        "schema": 1,
        "status": "passed",
        "runtime": {"phase": phase},
        "dataset": {
            "series_instance_uid": uid,
            "files_submitted": 2,
            "bytes_read": 128,
        },
        "output": {"panels": panels},
        "resource": {
            name: {
                "mean": value,
                "sample_stddev": 1.0,
                "approximate_95_half_width": 2.0,
                "count": 3,
            }
            for name, value in metrics.items()
        },
    }


class ResourceComparisonTests(unittest.TestCase):
    """Keep matching and uncertainty checks independent of application code."""

    @classmethod
    def setUpClass(cls):
        cls.compare = runpy.run_path(str(SCRIPTS / "resource_compare.py"))

    def test_success_reports_right_minus_left_and_combined_uncertainty(self):
        left = _record(mean=100.0)
        right = _record(mean=110.0)
        result = self.compare["compare"](
            left,
            right,
            left_name="Metis",
            right_name="Reference",
            match_keys=(
                "runtime.phase",
                "dataset.series_instance_uid",
                "dataset.files_submitted",
                "dataset.bytes_read",
                "output.panels",
            ),
            left_digest="a" * 64,
            right_digest="b" * 64,
        )
        metric = result["metrics"]["peak_private_bytes"]
        self.assertEqual(result["status"], "passed")
        self.assertEqual(metric["delta_right_minus_left"], 10.0)
        self.assertEqual(metric["combined_approximate_95_half_width"], 2.0 * (2**0.5))
        self.assertFalse(metric["delta_within_combined_interval"])
        self.assertEqual(result["sources"]["left_sha256"], "a" * 64)

    def test_match_difference_is_rejected_before_metric_work(self):
        left = _record(uid="series-1")
        right = _record(uid="series-2")
        with self.assertRaisesRegex(ValueError, "series_instance_uid.*differs"):
            self.compare["compare"](
                left,
                right,
                left_name="left",
                right_name="right",
                match_keys=("dataset.series_instance_uid",),
            )

    def test_phase_difference_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "phases do not match"):
            self.compare["compare"](
                _record(phase="idle"),
                _record(phase="lifecycle"),
                left_name="left",
                right_name="right",
                match_keys=("dataset.files_submitted",),
            )

    def test_missing_match_key_is_rejected(self):
        right = _record()
        del right["output"]["panels"]
        with self.assertRaisesRegex(ValueError, "lacks match key 'output.panels'"):
            self.compare["compare"](
                _record(),
                right,
                left_name="left",
                right_name="right",
                match_keys=("output.panels",),
            )

    def test_metric_requires_repeated_samples(self):
        left = _record()
        left["resource"]["duration_ms"]["count"] = 1
        with self.assertRaisesRegex(ValueError, "duration_ms.*at least two"):
            self.compare["compare"](
                left,
                _record(),
                left_name="left",
                right_name="right",
                match_keys=("dataset.files_submitted",),
                metric_names=("duration_ms",),
            )

    def test_metric_rejects_non_finite_values(self):
        left = _record()
        left["resource"]["duration_ms"]["mean"] = float("nan")
        with self.assertRaisesRegex(ValueError, "duration_ms.mean.*finite"):
            self.compare["compare"](
                left,
                _record(),
                left_name="left",
                right_name="right",
                match_keys=("dataset.files_submitted",),
                metric_names=("duration_ms",),
            )

    def test_match_value_rejects_non_finite_json(self):
        left = _record()
        left["dataset"]["bytes_read"] = float("nan")
        with self.assertRaisesRegex(ValueError, "bytes_read.*JSON-compatible"):
            self.compare["compare"](
                left,
                _record(),
                left_name="left",
                right_name="right",
                match_keys=("dataset.bytes_read",),
                metric_names=("duration_ms",),
            )

    def test_duplicate_keys_and_empty_keys_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "match keys must be unique"):
            self.compare["compare"](
                _record(),
                _record(),
                left_name="left",
                right_name="right",
                match_keys=("dataset.files_submitted", "dataset.files_submitted"),
            )
        with self.assertRaisesRegex(ValueError, "at least one --match key"):
            self.compare["compare"](
                _record(), _record(), left_name="left", right_name="right", match_keys=()
            )

    def test_label_rejects_paths_and_control_characters(self):
        with self.assertRaisesRegex(ValueError, "path separator"):
            self.compare["compare"](
                _record(),
                _record(),
                left_name="left/name",
                right_name="right",
                match_keys=("dataset.files_submitted",),
            )
        with self.assertRaisesRegex(ValueError, "control character"):
            self.compare["compare"](
                _record(),
                _record(),
                left_name="left\nname",
                right_name="right",
                match_keys=("dataset.files_submitted",),
            )

    def test_cli_writes_bounded_output_without_input_paths(self):
        with tempfile.TemporaryDirectory(prefix="metis-resource-compare-") as directory:
            root = pathlib.Path(directory)
            left_path = root / "left.json"
            right_path = root / "right.json"
            output_path = root / "comparison.json"
            left_path.write_text(json.dumps(_record(mean=100.0)), encoding="utf-8")
            right_path.write_text(json.dumps(_record(mean=105.0)), encoding="utf-8")
            result = self.compare["main"]([
                "--left", str(left_path),
                "--right", str(right_path),
                "--left-name", "Metis",
                "--right-name", "Reference",
                "--match", "runtime.phase",
                "--match", "dataset.series_instance_uid",
                "--match", "dataset.files_submitted",
                "--match", "dataset.bytes_read",
                "--match", "output.panels",
                "--output", str(output_path),
            ])
            self.assertEqual(result, 0)
            report = json.loads(output_path.read_text(encoding="utf-8"))
            self.assertEqual(report["schema"], 1)
            self.assertEqual(report["left"]["name"], "Metis")
            self.assertEqual(report["metrics"]["peak_private_bytes"]["delta_right_minus_left"], 5.0)
            self.assertNotIn(str(left_path), json.dumps(report))
            self.assertNotIn(str(right_path), json.dumps(report))
            self.assertEqual(len(report["sources"]["left_sha256"]), 64)

    def test_input_symlink_is_rejected(self):
        with tempfile.TemporaryDirectory(prefix="metis-resource-compare-link-") as directory:
            root = pathlib.Path(directory)
            source = root / "source.json"
            link = root / "link.json"
            source.write_text(json.dumps(_record()), encoding="utf-8")
            try:
                link.symlink_to(source)
            except (OSError, NotImplementedError) as error:
                self.skipTest(f"symlinks unavailable: {error}")
            with self.assertRaisesRegex(ValueError, "redirected"):
                self.compare["_read_record"](link, role="left record")

    def test_output_hardlink_is_rejected(self):
        with tempfile.TemporaryDirectory(prefix="metis-resource-compare-hardlink-") as directory:
            root = pathlib.Path(directory)
            original = root / "original.json"
            alias = root / "alias.json"
            original.write_text("{}", encoding="utf-8")
            try:
                os.link(original, alias)
            except OSError as error:
                self.skipTest(f"hard links unavailable: {error}")
            with self.assertRaisesRegex(ValueError, "single regular file"):
                self.compare["_validate_output"](alias)
