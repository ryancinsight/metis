"""Trace every reviewed native capability pair to a source and live owner."""
from __future__ import annotations

import copy
import json
import pathlib
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from verification.tauri import validate


class TauriInventoryTests(unittest.TestCase):
    def setUp(self):
        self.inventory = json.loads((ROOT / "docs/conformance/tauri.json").read_text())

    def test_complete_pinned_inventory_has_every_admitted_pair_and_live_owner(self):
        result = validate(self.inventory, ROOT)
        self.assertEqual(result["features"], 32)
        self.assertEqual(result["admitted_pairs"], 124)
        self.assertEqual(result["admitted_pairs"], result["bounded_pairs"] + result["open_pairs"])
        self.assertEqual(result["restricted_pairs"], 160 - 124)
        biometric = next(row for row in self.inventory["features"] if row["name"] == "biometric")
        self.assertEqual((biometric["tauri"]["macos"], biometric["metis"]["macos"]), ("full", "open"))

    def test_restriction_cannot_hide_an_admitted_native_pair(self):
        inventory = copy.deepcopy(self.inventory)
        inventory["features"][0]["metis"]["macos"] = "restriction"
        with self.assertRaisesRegex(ValueError, "admitted pair"):
            validate(inventory, ROOT)

    def test_invalid_source_owner_boundary_and_coverage_fail_closed(self):
        changes = [
            lambda data: data["plugin_source"].update(revision="unverified"),
            lambda data: data["features"][0].update(owner="METIS-MISSING-OWNER"),
            lambda data: data["features"][0].update(boundary="crates/missing.rs"),
            lambda data: data["features"][0].update(remaining=""),
            lambda data: data["features"].append(copy.deepcopy(data["features"][0])),
            lambda data: data["features"][0]["tauri"].pop("ios"),
            lambda data: data["features"][0]["metis"].update(windows="complete"),
        ]
        for change in changes:
            with self.subTest(change=change):
                inventory = copy.deepcopy(self.inventory)
                change(inventory)
                with self.assertRaises(ValueError):
                    validate(inventory, ROOT)
