"""Closed native case plan and explicit provider-observation selection.

Only the two noninteractive cases may share a native wave. The three model
commands share one preceding wave. Every command retains the process owner's
60-second execution plus 10-second cleanup bound: seven waves require 490
seconds within the coordinator's existing 540-second outer limit.
"""

from dataclasses import dataclass

from process_tree import PROCESS_TREE_CLEANUP_SECONDS

from . import fixtures
from .contract import FIXTURE_SECONDS


SELECTION_ENV = "MOIRAI_WEBVIEW_STARTUP_OBSERVATION"
ROLES = ("form", "permissions", "permissions_capture", *fixtures.FIXTURES)
INPUT_ROLES = ("form", "permissions", "stop_remount_destruction",
               "correlated_terminal_rejections")


def provider_selection(role: str, observed: bool) -> dict:
    if role not in ROLES:
        raise ValueError(f"unknown actual native scenario: {role}")
    if type(observed) is not bool:
        raise ValueError("startup observation selection must be a boolean")
    if observed and role != "form":
        raise ValueError("startup observation is allowed only for the form worker")
    return {"selected": observed, "configuration": {
        "start_uri": "https://metis.invalid/index.html",
        "selectors": ["#host-status", "#result"],
        # VK_D starts suffix retention at the existing Dark-theme shortcut.
        "late_input_after_key": 68,
    } if observed else None}


@dataclass(frozen=True)
class Case:
    identifier: str
    role: str
    observe_startup: bool = False

    def __post_init__(self) -> None:
        provider_selection(self.role, self.observe_startup)

    def arguments(self) -> tuple[str, ...]:
        return ("--worker", self.role, *(("--observe-startup",) if self.observe_startup else ()))

    def admit(self, report: dict) -> None:
        selection = report.get("provider_startup_observation")
        expected = provider_selection(self.role, self.observe_startup)
        if (report.get("scenario") != self.role or not isinstance(selection, dict)
                or type(selection.get("selected")) is not bool or selection != expected):
            raise ValueError(f"actual worker role/observation differs from requested case: {self.identifier}")


WAVES = (
    (Case("form", "form", True),),
    (Case("form_ordinary", "form"),),
    (Case("permissions", "permissions"),),
    (Case("permissions_capture", "permissions_capture"),
     Case("malformed_duplicate_stale_commands", "malformed_duplicate_stale_commands")),
    (Case("stop_remount_destruction", "stop_remount_destruction"),),
    (Case("correlated_terminal_rejections", "correlated_terminal_rejections"),),
)
SCHEDULE_SECONDS = (1 + len(WAVES)) * (FIXTURE_SECONDS + PROCESS_TREE_CLEANUP_SECONDS)
