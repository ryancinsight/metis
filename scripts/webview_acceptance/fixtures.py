"""The exact real native fixtures; assertions inspect their actual boundary events."""

import re

from .contract import require_wire_rates, require_released
from .evidence import events, one, payload

PREFIX = "frontend::webview::runtime_tests::"
FIXTURES = {
    "stop_remount_destruction": PREFIX + "rust_page_real_stop_remount_destruction",
    "malformed_duplicate_stale_commands": PREFIX + "rust_page_real_malformed_duplicate_stale_commands",
    "correlated_terminal_rejections": PREFIX + "correlated::rust_page_real_correlated_terminal_rejections",
}
TEST_RESULT = re.compile(r"test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out;")


def remount_progression(observed: list[tuple[str, str]]) -> list[str]:
    expected = [(event, str(mount)) for mount in range(1, 4)
                for event in ("ready", "submit", "close_requested", "stopped")]
    if observed != expected:
        raise ValueError("actual remount events miss an ordered Ready/Submit/CloseRequested/Stopped cycle")
    return [mount for event, mount in observed if event == "ready"]


def backend_observations(values: list[dict], page_events: list[dict],
                         transactions: int, reply_count: int) -> dict:
    submit = [value for value in page_events if value.get("type") == "submit"]
    if len(submit) != transactions:
        raise ValueError("real fixture misses a genuine OS submission")
    for value in submit:
        inputs = value["inputs"]
        if inputs["patient_id"] != "PT-REAL-患者" or value["request_id"] != "1":
            raise ValueError("real fixture submission differs from its Unicode/request identity")
        for field, expected in (("weight_kg", 62.5), ("concentration_mg_ml", 1.875),
                                ("target_dose_mcg_kg_min", 0.5)):
            require_wire_rates((inputs[field], inputs[field]), (expected, expected))
    replies = [payload(value) for value in events(values, "page_terminal")]
    if len(replies) != reply_count or any(value.get("type") != "result" for value in replies):
        raise ValueError("real fixture does not deliver every actual terminal variant")
    for reply in replies:
        require_wire_rates((reply["rate_ml_hr"], reply["drug_rate_mg_hr"]), (1.0, 1.875))
        if reply["audit_sequence_id"] != "2" or reply.get("is_pediatric") is not False:
            raise ValueError("actual terminal differs from the verified two-record backend ledger")
    return {"submissions": submit, "terminals": replies}


def verify(name: str, values: list[dict], stdout: str, module_digest: str) -> dict:
    if len(TEST_RESULT.findall(stdout)) != 1:
        raise ValueError("real native fixture does not report exactly one executed passing test")
    complete = one(values, "fixture_completed")
    if complete.get("fixture") != name:
        raise ValueError("real native fixture completion belongs to another scenario")
    page_events = [payload(value) for value in events(values, "fixture_page_event")]
    if not page_events or any(value.get("identity", {}).get("module_digest") != module_digest for value in page_events):
        raise ValueError("real fixture page events differ from the compiled module identity")
    ready = [value for value in page_events if value.get("type") == "ready"]
    stopped = [value for value in page_events if value.get("type") == "stopped"]
    failed = [value for value in page_events if value.get("type") == "failed"]
    mounts = 3 if name == "stop_remount_destruction" else 1
    if len(ready) != mounts or len(stopped) != mounts:
        raise ValueError("real native fixture misses a required readiness/stop observation")
    if [value["identity"]["mount_id"] for value in ready] != [str(index) for index in range(1, mounts + 1)]:
        raise ValueError("actual remount generations do not advance exactly once")
    for admitted, released in zip(ready, stopped):
        if admitted["identity"] != released["identity"]:
            raise ValueError("actual fixture stop does not match its ready mount")
        require_released(released["resources"])
    destroyed = one(values, "page_destroyed")
    cleaned = one(values, "page_package_cleaned")
    if not values.index(destroyed) < values.index(cleaned) < values.index(complete):
        raise ValueError("real fixture cleanup completes before native destruction")
    expected_rejections = 0 if mounts == 3 else 3
    if len(failed) != expected_rejections or len(events(values, "fixture_rejection")) != expected_rejections:
        raise ValueError("real hostile native command rejection count differs from its contract")
    if expected_rejections and complete.get("rejections") != expected_rejections:
        raise ValueError("real fixture computes an incorrect rejection total")
    backend = None
    if mounts == 3:
        if complete.get("mounts") != mounts or complete.get("actual_backend_transactions") != mounts:
            raise ValueError("real fixture computes an incorrect mount/backend total")
        remount_progression([(value.get("type"), value["identity"]["mount_id"]) for value in page_events])
        backend = backend_observations(values, page_events, mounts, mounts)
        for submit, reply in zip(backend["submissions"], backend["terminals"]):
            if reply["identity"] != submit["identity"] or reply["request_id"] != submit["request_id"]:
                raise ValueError("actual remount result has a stale mount or request correlation")
    if name == "correlated_terminal_rejections":
        if complete.get("actual_backend_transactions") != 1:
            raise ValueError("correlated native fixture does not compute one real backend transaction")
        backend = backend_observations(values, page_events, 1, 4)
        submit = backend["submissions"][0]
        stale_request, stale_mount, correct, duplicate = backend["terminals"]
        if (stale_request["identity"] != submit["identity"] or stale_request["request_id"] != "2"
                or stale_mount["identity"]["mount_id"] != "2"
                or stale_mount["request_id"] != submit["request_id"]
                or correct["identity"] != submit["identity"]
                or correct["request_id"] != submit["request_id"] or duplicate != correct):
            raise ValueError("actual terminal variants do not retain the declared stale/duplicate correlations")
    return {"completion": complete, "page_events": page_events,
            "destroyed": destroyed, "cleaned": cleaned, "backend": backend}
