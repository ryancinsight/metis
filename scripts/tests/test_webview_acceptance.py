"""Value-semantic analytical acceptance and bounded real-file parsing contracts."""

import json
import math
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from webview_acceptance import compiler, evidence, fixtures
from webview_acceptance.contract import (
    CALCULATIONS, binary, parse_headless, parse_result, require_rates, require_released,
    permits_denial_reconciliation, require_wire_rates,
)


class ComputedModel(unittest.TestCase):
    def test_three_real_journey_oracles_match_independent_rational_rates(self):
        expected = ((12 / 25, 24 / 25), (3 / 5, 12 / 5), (9 / 100, 18 / 25))
        for case, rates in zip(CALCULATIONS, expected):
            with self.subTest(patient=case.patient):
                actual = case.expected()
                self.assertEqual(tuple(map(binary, actual)), tuple(map(binary, rates)))
                self.assertEqual(binary(actual[0] * float(case.concentration)), binary(actual[1]))
                result = parse_result(f"Rate {actual[0]} mL/hour; drug {actual[1]} mg/hour; audit 2", case)
                before = parse_headless(f"frontend_pid=37 rate_ml_hr={actual[0]} drug_rate_mg_hr={actual[1]} audit_sequence=2\n", case)
                self.assertEqual(result["audit_sequence"], before["audit_sequence"])
                self.assertEqual(binary(result["rate_ml_hr"]), binary(before["rate_ml_hr"]))
                self.assertEqual(binary(result["drug_rate_mg_hr"]), binary(before["drug_rate_mg_hr"]))

    def test_exact_float_observations_reject_one_ulp_nonfinite_and_boolean(self):
        actual = CALCULATIONS[0].expected()
        for changed in (math.nextafter(actual[0], math.inf), math.inf, math.nan, True):
            with self.subTest(value=changed), self.assertRaises(ValueError):
                require_rates((changed, actual[1]), actual)
        with self.assertRaises(ValueError):
            require_rates(("-0", "0"), (0.0, 0.0))

    def test_protocol_observations_require_decimal_strings_and_preserve_exact_bits(self):
        for actual, expected in ((("1.0", "1.875"), (1.0, 1.875)),
                                 (("-0.0", "5e-324"), (-0.0, math.ulp(0.0)))):
            with self.subTest(actual=actual):
                require_wire_rates(actual, expected)
                self.assertEqual(tuple(map(binary, actual)), tuple(map(binary, expected)))
        for actual in ((1.0, "1.875"), ("1.0", 1.875), (True, "1.875"),
                       ("1.0000000000000002", "1.875"), ("NaN", "1.875")):
            with self.subTest(actual=actual), self.assertRaises(ValueError):
                require_wire_rates(actual, (1.0, 1.875))
        with self.assertRaises(ValueError):
            require_wire_rates(("0.0", "1.875"), (-0.0, 1.875))

    def test_actual_output_preserves_full_u64_and_rejects_ambiguous_output(self):
        case = CALCULATIONS[0]
        for audit in ("1", str(2**64 - 1)):
            actual = parse_result(f"Rate 0.48 mL/hour; drug 0.96 mg/hour; audit {audit}", case)
            self.assertEqual(actual["audit_sequence"], audit)
        for audit in ("0", "01", str(2**64)):
            with self.subTest(audit=audit), self.assertRaises(ValueError):
                parse_result(f"Rate 0.48 mL/hour; drug 0.96 mg/hour; audit {audit}", case)
        duplicated = "frontend_pid=37 rate_ml_hr=0.48 drug_rate_mg_hr=0.96 audit_sequence=2\n" * 2
        with self.assertRaisesRegex(ValueError, "exactly one"):
            parse_headless(duplicated, case)


class ObservedOwnership(unittest.TestCase):
    def test_remount_progression_requires_each_actual_submit_close_and_stop_in_order(self):
        phases = ("ready", "submit", "close_requested", "stopped")
        progression = [(event, mount) for mount in ("1", "2", "3") for event in phases]
        self.assertEqual(fixtures.remount_progression(progression), ["1", "2", "3"])
        changed_mount = progression.copy()
        changed_mount[5] = ("submit", "1")
        changed_order = progression.copy()
        changed_order[5:7] = reversed(changed_order[5:7])
        for invalid in (progression[:-1], progression + [("ready", "4")],
                        progression[:2] + progression[1:], changed_mount, changed_order):
            with self.subTest(events=invalid), self.assertRaises(ValueError):
                fixtures.remount_progression(invalid)

    def test_cleanup_counts_reject_each_retained_application_owner(self):
        counts = dict(dom_listeners=0, application_tasks=0, bridge_listeners=1,
                      retained_bridge_callbacks=0, active_requests=0,
                      geolocation_watches=0, geolocation_dispatchers=2,
                      media_leases=0, media_tracks=0, unsettled_guards=4)
        self.assertEqual(require_released(counts), None)
        for key in ("dom_listeners", "application_tasks", "retained_bridge_callbacks",
                    "active_requests", "geolocation_watches", "media_leases", "media_tracks"):
            with self.subTest(owner=key), self.assertRaises(ValueError):
                require_released(counts | {key: 1})
        for key, value in (("bridge_listeners", 0), ("bridge_listeners", True),
                           ("geolocation_dispatchers", 1), ("unsettled_guards", 5),
                           ("application_tasks", False)):
            with self.subTest(owner=key, value=value), self.assertRaises(ValueError):
                require_released(counts | {key: value})

    def test_actual_trace_file_handles_only_complete_json_records_during_writes(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "native.jsonl"
            completed = json.dumps({"fields": {"event": "page_ready", "json": "患者-🦀"}}, ensure_ascii=False).encode()
            path.write_bytes(completed + b"\n" + b'{"fields": {"event": "page_stopped", "text": "' + "患".encode()[:1])
            values = evidence.records(path, complete=False)
            self.assertEqual(values, [{"event": "page_ready", "json": "患者-🦀"}])
            with self.assertRaisesRegex(ValueError, "incomplete"):
                evidence.records(path)
            path.write_bytes(completed + b"\n")
            self.assertEqual(evidence.one(evidence.records(path), "page_ready"), values[0])

    def test_native_event_arity_and_exact_schema_payload(self):
        record = {"event": "page_ready", "json": '{"type":"ready","identity":{"mount_id":"9007199254740993"}}'}
        self.assertEqual(evidence.payload(record)["identity"]["mount_id"], "9007199254740993")
        with self.assertRaisesRegex(ValueError, "observed 2"):
            evidence.one([record, record.copy()], "page_ready")
        with self.assertRaisesRegex(ValueError, "observed 0"):
            evidence.one([record], "page_stopped")


class PermissionDenial(unittest.TestCase):
    def test_each_api_accepts_only_its_actual_denial_result_domain(self):
        self.assertEqual(permits_denial_reconciliation("geolocation", {"status": "rejected", "code": 1}), True)
        for code in (None, 0, 2, 3, True, "1"):
            with self.subTest(code=code):
                self.assertEqual(permits_denial_reconciliation("geolocation", {"status": "rejected", "code": code}), False)
        for kind in ("camera", "microphone"):
            self.assertEqual(permits_denial_reconciliation(kind, {"status": "rejected", "name": "NotAllowedError"}), True)
            for name in (None, "NotFoundError", "NotReadableError", "AbortError", "OverconstrainedError", "SecurityError", "TypeError"):
                with self.subTest(kind=kind, name=name):
                    self.assertEqual(permits_denial_reconciliation(kind, {"status": "rejected", "name": name}), False)
        for status in ("denied", "default"):
            self.assertEqual(permits_denial_reconciliation("notifications", {"status": status}), True)
            for kind in ("geolocation", "camera", "microphone"):
                self.assertEqual(permits_denial_reconciliation(kind, {"status": status}), False)
        for kind in ("geolocation", "camera", "microphone", "notifications", "unknown"):
            for status in ("granted", "unavailable", "pending", None):
                with self.subTest(kind=kind, status=status):
                    self.assertEqual(permits_denial_reconciliation(kind, {"status": status, "code": 1, "name": "NotAllowedError"}), False)
        self.assertEqual(permits_denial_reconciliation("notifications", {"status": "rejected", "name": "NotAllowedError"}), False)

    def test_missing_native_kind_failure_retains_exact_browser_context(self):
        encoded = json.dumps({"type": "probe_completed", "observations": [
            {"kind": "camera", "browser": {"status": "rejected", "name": "NotFoundError", "code": 0,
                                             "message": "Requested device not found"}, "host_denial": None}]})
        native = {"event": "page_permission_denied", "permission": "geolocation", "user_initiated": False,
                  "source": "https://metis.invalid/"}
        browser = {"event": "page_probe_observed", "json": encoded, "source": "https://metis.invalid/index.html"}
        diagnostics = evidence.permission_diagnostics([native, browser])
        self.assertEqual(diagnostics["missing_native_kinds"], ["camera", "microphone", "notifications"])
        self.assertEqual(diagnostics["native_denials"], [native])
        self.assertEqual(diagnostics["browser_records"], [browser])
        self.assertEqual(evidence.payload(diagnostics["browser_records"][0])["observations"][0]["browser"]["message"],
                         "Requested device not found")
        with self.assertRaises(ValueError) as rejected:
            evidence.permissions([native, browser])
        self.assertIn("NotFoundError", str(rejected.exception))
        self.assertIn("missing_native_kinds", str(rejected.exception))


class CompilerArtifact(unittest.TestCase):
    def test_selects_one_actual_existing_file_by_package_target_and_profile(self):
        path = Path(sys.executable).resolve(strict=True)
        production = {"reason": "compiler-artifact", "package_id": "path+file:///repo/crates/metis-app#0.5.0",
                      "target": {"name": "metis-app"}, "profile": {"test": False}, "executable": str(path)}
        test = production | {"profile": {"test": True}}
        text = "Cargo progress\n" + json.dumps(production) + "\n" + json.dumps(test) + "\n"
        self.assertEqual(compiler.executable(text, "metis-app", "metis-app"), path)
        self.assertEqual(compiler.executable(text, "metis-app", "metis-app", tests=True), path)
        with self.assertRaisesRegex(ValueError, "exactly one"):
            compiler.executable(text, "metis-cli", "metis")
        with tempfile.TemporaryDirectory() as directory:
            missing = production | {"executable": str(Path(directory) / "absent")}
            with self.assertRaises(FileNotFoundError):
                compiler.executable(json.dumps(missing), "metis-app", "metis-app")


if __name__ == "__main__":
    unittest.main()
