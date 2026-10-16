"""Independent rational model and exact observed-value acceptance laws."""

from __future__ import annotations

from dataclasses import dataclass
from fractions import Fraction
import math
import re
import struct
from python_native_accessibility import MAX_UIA_OUTPUT_BYTES

FIXTURE_SECONDS = 60
MAX_TRACE_BYTES = 1024 * 1024
MAX_UIA_BYTES = MAX_UIA_OUTPUT_BYTES
SCHEMA = 1
PERMISSIONS = ("geolocation", "camera", "microphone", "notifications")


@dataclass(frozen=True)
class Calculation:
    patient: str
    weight: str
    concentration: str
    dose: str
    action: str

    def expected(self) -> tuple[float, float]:
        """mg/hour = dose*weight*60/1000; mL/hour = mg/hour/concentration.

        The three app-journey concentrations are powers of two, so native
        rate*c preserves the rounded rate's binary significand. The native
        remount point gives rate=1 and drug=1.875 exactly. All declared points
        make d*w*60 and c*1000 exact integers before division.
        """
        drug = Fraction(self.dose) * Fraction(self.weight) * 60 / 1000
        rate = drug / Fraction(self.concentration)
        return float(rate), float(drug)

    def arguments(self) -> list[str]:
        return [self.weight, self.concentration, self.dose]


CALCULATIONS = (
    Calculation("患者-🦀-one", "64", "2", "0.25", "click"),
    Calculation("患者-🦀-two", "80", "4", "0.5", "return"),
    Calculation("患者-🦀-three", "96", "8", "0.125", "click"),
)

RESULT = re.compile(r"^Rate (\S+) mL/hour; drug (\S+) mg/hour; audit ([1-9][0-9]*)$")
HEADLESS = re.compile(r"frontend_pid=([1-9][0-9]*) rate_ml_hr=(\S+) "
                      r"drug_rate_mg_hr=(\S+) audit_sequence=([1-9][0-9]*)")


def binary(value: object) -> bytes:
    if isinstance(value, bool):
        raise ValueError("boolean cannot represent a calculated binary64 value")
    parsed = float(value)
    if not math.isfinite(parsed):
        raise ValueError("calculated value is not finite")
    return struct.pack(">d", parsed)


def require_rates(actual: tuple[object, object], expected: tuple[float, float]) -> None:
    if tuple(map(binary, actual)) != tuple(map(binary, expected)):
        raise ValueError(f"calculated rates differ from the rational oracle: {actual!r} != {expected!r}")


def require_wire_rates(actual: tuple[object, object], expected: tuple[float, float]) -> None:
    if any(type(value) is not str for value in actual):
        raise ValueError("production protocol finite values must be decimal JSON strings")
    require_rates(actual, expected)


def permits_denial_reconciliation(kind: str, browser: dict) -> bool:
    """Mirror CapabilityObservation's closed W3C denial-result domain.

    PositionError code1 is PERMISSION_DENIED; getUserMedia NotAllowedError is
    permission rejection. Device discovery/read errors cannot stand for denial.
    Actual native callback correlation remains a separate required observation.
    """
    status = browser.get("status")
    if kind == "geolocation":
        return status == "rejected" and type(browser.get("code")) is int and browser["code"] == 1
    if kind in ("camera", "microphone"):
        return status == "rejected" and browser.get("name") == "NotAllowedError"
    return kind == "notifications" and status in ("denied", "default")


def parse_result(text: str, calculation: Calculation) -> dict:
    match = RESULT.fullmatch(text)
    if match is None:
        raise ValueError(f"actual UI result has no declared calculation: {text!r}")
    rate, drug, audit = match.groups()
    require_rates((rate, drug), calculation.expected())
    if int(audit) > 2**64 - 1:
        raise ValueError("actual audit sequence exceeds its u64 domain")
    return {"rate_ml_hr": rate, "drug_rate_mg_hr": drug, "audit_sequence": audit}


def parse_headless(text: str, calculation: Calculation) -> dict:
    matches = HEADLESS.findall(text)
    if len(matches) != 1:
        raise ValueError("unchanged model must emit exactly one actual terminal result")
    pid, rate, drug, audit = matches[0]
    require_rates((rate, drug), calculation.expected())
    if int(audit) > 2**64 - 1:
        raise ValueError("headless audit sequence exceeds its u64 domain")
    return {"frontend_pid": int(pid), "rate_ml_hr": rate,
            "drug_rate_mg_hr": drug, "audit_sequence": audit}


def require_released(counts: dict) -> None:
    zero = ("dom_listeners", "application_tasks", "retained_bridge_callbacks",
            "active_requests", "geolocation_watches", "media_leases", "media_tracks")
    if any(type(counts.get(key)) is not int or counts[key] != 0 for key in zero):
        raise ValueError("actual stop retains application resources or failed cleanup")
    if counts.get("bridge_listeners") != 1 or type(counts["bridge_listeners"]) is not int:
        raise ValueError("actual stop must retain exactly the coordinator bridge")
    if type(counts.get("geolocation_dispatchers")) is not int or counts["geolocation_dispatchers"] not in (0, 2):
        raise ValueError("actual geolocation dispatcher count is outside its ownership law")
    if type(counts.get("unsettled_guards")) is not int or not 0 <= counts["unsettled_guards"] <= 4:
        raise ValueError("actual settlement guards exceed the four admitted request kinds")
