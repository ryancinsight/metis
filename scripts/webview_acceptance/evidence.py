"""Bounded actual tracing input and lifecycle/protocol observation checks."""

import hashlib
import json
from pathlib import Path

from .contract import (
    MAX_TRACE_BYTES, PERMISSIONS, permits_denial_reconciliation,
    require_wire_rates, require_released,
)


def digest(path: Path) -> str:
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def records(path: Path, *, complete: bool = True) -> list[dict]:
    if path.stat().st_size > MAX_TRACE_BYTES:
        raise ValueError("actual native trace exceeds its bounded evidence budget")
    data = path.read_bytes()
    if not complete:
        data = data.rsplit(b"\n", 1)[0] if b"\n" in data else b""
    elif data and not data.endswith(b"\n"):
        raise ValueError("actual native trace ends with an incomplete record")
    result = []
    for line in data.decode("utf-8").splitlines():
        if not line.startswith("{"):
            continue
        value = json.loads(line)
        fields = value.get("fields")
        if not isinstance(fields, dict):
            continue
        if "event" in fields:
            result.append(fields)
    return result


def events(values: list[dict], name: str) -> list[dict]:
    return [value for value in values if value.get("event") == name]


def one(values: list[dict], name: str) -> dict:
    selected = events(values, name)
    if len(selected) != 1:
        raise ValueError(f"actual native trace requires one {name}, observed {len(selected)}")
    return selected[0]


def payload(record: dict) -> dict:
    value = record.get("json")
    if not isinstance(value, str):
        raise ValueError("actual event has no encoded production protocol value")
    decoded = json.loads(value)
    if not isinstance(decoded, dict):
        raise ValueError("actual protocol observation is not an object")
    return decoded


def lifecycle(values: list[dict], module_digest: str) -> dict:
    names = ("page_ready", "page_stop_requested", "page_stopped",
             "page_destroyed", "page_package_cleaned")
    selected = [one(values, name) for name in names]
    positions = [values.index(value) for value in selected]
    if positions != sorted(positions) or len(set(positions)) != len(positions):
        raise ValueError("actual stop/destruction/cleanup observations are out of order")
    ready = payload(selected[0])
    stopped = payload(selected[2])
    if ready.get("type") != "ready" or stopped.get("type") != "stopped":
        raise ValueError("lifecycle trace does not contain actual Ready and Stopped events")
    identity = ready["identity"]
    if (identity.get("module_digest") != module_digest
            or identity.get("schema") != 1
            or stopped.get("identity") != identity):
        raise ValueError("actual lifecycle identity differs from the compiled inventory")
    require_released(stopped["resources"])
    if events(values, "page_failed"):
        raise ValueError("actual page reports a failed application operation")
    return {"identity": identity, "stopped_resources": stopped["resources"],
            "stages": names}


def calculation(values: list[dict], case, result: dict) -> dict:
    submits = [payload(record) for record in events(values, "page_submit")]
    matches = [value for value in submits if value.get("inputs", {}).get("patient_id") == case.patient]
    if len(matches) != 1:
        raise ValueError("Unicode submission has no unique actual native observation")
    submitted = matches[0]
    inputs = submitted["inputs"]
    for field, expected in (("weight_kg", case.weight), ("concentration_mg_ml", case.concentration),
                            ("target_dose_mcg_kg_min", case.dose)):
        require_wire_rates((inputs[field], inputs[field]), (float(expected), float(expected)))
    replies = [payload(record) for record in events(values, "page_terminal")]
    correlated = [value for value in replies if value.get("request_id") == submitted["request_id"]]
    if len(correlated) != 1:
        raise ValueError("actual native submission has no unique correlated terminal reply")
    reply = correlated[0]
    if reply.get("type") != "result" or reply.get("identity") != submitted["identity"]:
        raise ValueError("actual terminal reply has a mismatched type or identity")
    require_wire_rates((reply["rate_ml_hr"], reply["drug_rate_mg_hr"]), case.expected())
    if reply["audit_sequence_id"] != result["audit_sequence"]:
        raise ValueError("UI audit identifier differs from the real native terminal reply")
    signature = reply.get("result_signature")
    if (not isinstance(signature, list) or len(signature) != 32
            or any(type(byte) is not int or not 0 <= byte <= 255 for byte in signature)):
        raise ValueError("actual terminal reply does not retain its native result signature")
    return {"submit": submitted, "terminal": reply}


def permission_diagnostics(values: list[dict]) -> dict:
    native = events(values, "page_permission_denied")
    observed = [value.get("permission") for value in native]
    return {"native_denials": native,
            "missing_native_kinds": [kind for kind in PERMISSIONS if kind not in observed],
            "duplicate_native_kinds": [kind for kind in PERMISSIONS if observed.count(kind) > 1],
            "browser_records": [value for value in values if value.get("event") in
                                ("page_probe_observed", "page_probe_completed")]}


def permissions(values: list[dict]) -> dict:
    diagnostic = permission_diagnostics(values)
    context = json.dumps(diagnostic, ensure_ascii=False, sort_keys=True)
    native = events(values, "page_permission_denied")
    if (len(native) != len(PERMISSIONS)
            or {value.get("permission") for value in native} != set(PERMISSIONS)
            or any(type(value.get("user_initiated")) is not bool for value in native)):
        raise ValueError(f"required actual native permission kinds are missing or duplicated: {context}")
    completed = events(values, "page_probe_completed")
    reconciled = [payload(value) for value in completed if value.get("reconciled") is True]
    if len(reconciled) != 1:
        raise ValueError(f"browser outcomes have no unique final native-denial reconciliation: {context}")
    final = reconciled[0]
    observations = final.get("observations")
    if not isinstance(observations, list) or [value.get("kind") for value in observations] != list(PERMISSIONS):
        raise ValueError("settled browser observations differ from the four actual request kinds")
    for observation in observations:
        denial = next(value for value in native if value["permission"] == observation["kind"])
        if not permits_denial_reconciliation(observation["kind"], observation.get("browser", {})):
            raise ValueError(f"browser outcome is outside the actual permission-denial domain: {observation}; {context}")
        if observation.get("host_denial") != denial["user_initiated"]:
            raise ValueError(f"settled browser observation has no matching actual native denial: {context}")
    counts = final["resources"]
    for name in ("application_tasks", "active_requests", "geolocation_watches",
                 "media_leases", "media_tracks", "unsettled_guards"):
        if type(counts.get(name)) is not int or counts[name] != 0:
            raise ValueError("completed browser requests retain unsettled actual resources")
    return {"native": native, "completed": final, "diagnostics": diagnostic}
