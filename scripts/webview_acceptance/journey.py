"""Genuine OS input and actual UI Automation values for the Rust page."""

import time

from native_capture.images import files
from native_capture.windows import gdi, window
from native_input.windows import pointer
from python_native_accessibility import UIA_POLL_MILLISECONDS

from .contract import CALCULATIONS, Calculation, parse_result
from .text import TextValueRepresentation


def captured(uia, destination) -> dict:
    pointer.guard_surface_visible(uia.handle)
    observation = window.window_observation(uia.handle)
    frame = gdi.capture_screen_window(observation.bounds)
    sha = files.write_capture(destination, observation.bounds, frame.pixels)
    return {"path": str(destination), "sha256": sha,
            "left": frame.left, "top": frame.top,
            "width": frame.width, "height": frame.height,
            "dpi": observation.dpi}


def edited(uia, patient: str, weight: str, concentration: str, dose: str) -> None:
    for identifier, value in (("patient-id", patient), ("weight", weight),
                              ("concentration", concentration), ("dose", dose)):
        uia.replace(identifier, value, TextValueRepresentation.CONTROL)


def submitted(uia, action: str, previous: str) -> str:
    if action == "click":
        uia.click("calculation-submit")
    elif action == "return":
        uia.request("focus", "dose")
        uia.enter_host("dose")
        uia.key(0x0D)
        uia.actions.append({"action": "SendInput Return", "id": "dose"})
    else:
        raise ValueError("undeclared physical submit action")
    node = uia.wait("result", lambda node: bool(node["text"]) and node["text"] != previous
                    and not node["text"].startswith("Submitting"))
    uia.wait("calculation-submit", lambda node: node["enabled"] is True)
    return node["text"]


def form(uia, output) -> dict:
    uia.wait("host-status", lambda node: "Host bridge connected" in node["text"])
    initial = captured(uia, output / "initial.png")
    values = []
    previous = uia.text("result")
    for index, case in enumerate(CALCULATIONS):
        if index == 2:
            edited(uia, "患者-invalid", "80", "0", "0.5")
            error = submitted(uia, "click", previous)
            if "[0x" not in error or error.startswith("Rate "):
                raise ValueError(f"actual zero concentration produces no backend rejection: {error}")
            values.append({"error": error, "patient": "患者-invalid"})
            previous = error
        edited(uia, case.patient, case.weight, case.concentration, case.dose)
        previous = submitted(uia, case.action, previous)
        values.append({"patient": case.patient, "dom": parse_result(previous, case)})
    themes = []
    for offset, label in ((1, "light"), (3, "high contrast")):
        uia.request("focus", "theme-mode")
        uia.enter_host("theme-mode")
        uia.key(0x24)
        for _ in range(offset):
            uia.key(0x28)
        uia.key(0x0D)
        state = uia.wait("theme-state", lambda node: node["text"] == f"Theme: {label}")
        themes.append({"control": "theme-mode", "text": state["text"],
                       "capture": captured(uia, output / f"theme-{offset}.png")})
    for letter, label in (("D", "dark"), ("S", "system preference")):
        uia.shortcut(letter)
        state = uia.wait("theme-state", lambda node: node["text"] == f"Theme: {label}")
        themes.append({"shortcut": ["Alt", "Shift", letter], "text": state["text"],
                       "capture": captured(uia, output / f"theme-{letter}.png")})
    uia.click("command-menu-toggle")
    uia.wait("command-status", lambda node: node["text"] == "Commands opened")
    uia.wait("command-theme-dark", lambda node: node["focused"] is True)
    uia.click("command-theme-dark")
    uia.wait("theme-state", lambda node: node["text"] == "Theme: dark")
    uia.wait("command-menu-toggle", lambda node: node["focused"] is True)
    uia.click("command-menu-toggle")
    uia.wait("command-status", lambda node: node["text"] == "Commands opened")
    uia.key(0x1B)
    uia.wait("command-status", lambda node: node["text"] == "Commands closed")
    uia.wait("command-menu-toggle", lambda node: node["focused"] is True)
    uia.shortcut("P")
    uia.wait("patient-id", lambda node: node["focused"] is True)
    window.resize_window(uia.handle, 900, 700)
    while True:
        geometry = window.window_observation(uia.handle)
        if (geometry.client_width, geometry.client_height) == (900, 700):
            break
        time.sleep(min(uia.remaining(), UIA_POLL_MILLISECONDS / 1000))
    resized = captured(uia, output / "resized.png")
    uia.key(0x1B)
    return {"initial": initial, "values": values, "themes": themes, "resized": resized,
            "menu_open_focus": "command-theme-dark", "menu_close_focus": "command-menu-toggle",
            "shortcut_focus": "patient-id", "actions": uia.actions}


def correlated(uia, requested: dict) -> dict:
    edited(uia, requested["patient_id"], str(requested["weight_kg"]),
           str(requested["concentration_mg_ml"]), str(requested["target_dose_mcg_kg_min"]))
    uia.click("calculation-submit")
    return {"input": requested, "actions": uia.actions}


def remount(uia, requested: dict) -> dict:
    mount = requested.get("mount_id")
    if mount not in ("1", "2", "3"):
        raise ValueError("actual remount input marker has no declared mount identity")
    reset = uia.wait("result", lambda node: node["text"] == "No calculation submitted.")
    enabled = uia.wait("calculation-submit", lambda node: node["enabled"] is True)
    commands = uia.wait("command-status", lambda node: node["text"] == "Commands ready")
    hidden = uia.wait_hidden("command-menu", "command-theme-dark", "command-theme-system")
    case = Calculation(requested["patient_id"], str(requested["weight_kg"]),
                       str(requested["concentration_mg_ml"]),
                       str(requested["target_dose_mcg_kg_min"]), "click")
    if mount == "1":
        uia.click("command-menu-toggle")
        uia.wait("command-status", lambda node: node["text"] == "Commands opened")
        uia.wait("command-theme-dark", lambda node: node["focused"] is True)
    edited(uia, case.patient, case.weight, case.concentration, case.dose)
    computed = parse_result(submitted(uia, "click", reset["text"]), case)
    if mount == "1":
        uia.wait("command-status", lambda node: node["text"] == "Commands opened")
        uia.key(0x1B)
        uia.wait("command-status", lambda node: node["text"] == "Commands closed")
        uia.wait_hidden("command-menu", "command-theme-dark", "command-theme-system")
        uia.wait("command-menu-toggle", lambda node: node["focused"] is True)
    uia.key(0x1B)
    return {"input": requested, "reset": reset, "submit": enabled,
            "commands": commands, "closed_menu": hidden, "dom": computed}
