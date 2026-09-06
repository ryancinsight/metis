"""Adversarial inventory and independent application-result workflow oracles."""
import copy
import hashlib
import os
import json
import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
import distribution
import verify


class DistributionTests(unittest.TestCase):
    def test_demonstration_has_one_executable_with_exact_entry(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            entry = "metis-app" + (".exe" if os.name == "nt" else "")
            content = b"Executable inventory acceptance fixture"
            (root / entry).write_bytes(content)
            (root / "assets").mkdir()
            (root / "assets" / "applications.md").write_bytes(b"Application instructions")
            inventory = {
                "entry": entry,
                "application": {"entry": "metis-app", "binaries": [{"package": "metis-app", "bin": "metis-app"}],
                                "resources": [{"destination": "assets/applications.md"}]},
                "files": [{"destination": name, "bytes": (root / name).stat().st_size,
                           "sha256": distribution.digest(root / name)}
                          for name in (entry, "assets/applications.md")]}
            self.assertEqual(distribution.verify_application(root, inventory), ["assets/applications.md", entry])
            for replacement in ("metis-backend.exe", "assets/applications.md", "metis-app"):
                if replacement != entry:
                    with self.subTest(entry=replacement), self.assertRaisesRegex(ValueError, "exactly one"):
                        distribution.verify_application(root, {**inventory, "entry": replacement})
            for binaries in ([], [{"package": "metis-backend", "bin": "metis-app"}],
                             inventory["application"]["binaries"] * 2,
                             inventory["application"]["binaries"] + [{"package": "metis-frontend", "bin": "metis-frontend"}]):
                changed = {**inventory, "application": {**inventory["application"], "binaries": binaries}}
                with self.subTest(binaries=binaries), self.assertRaisesRegex(ValueError, "exactly one"):
                    distribution.verify_application(root, changed)
            for helper in ("metis-frontend.exe", "assets/hidden.EXE"):
                (root / helper).write_bytes(b"Undeclared application image")
                changed = copy.deepcopy(inventory)
                changed["application"]["resources"].append({"destination": helper})
                changed["files"].append({"destination": helper, "bytes": (root / helper).stat().st_size,
                                         "sha256": distribution.digest(root / helper)})
                with self.subTest(helper=helper), self.assertRaisesRegex(ValueError, "exactly one"):
                    distribution.verify_application(root, changed)
                (root / helper).unlink()

    def test_workspace_target_ownership_prevents_split_executables(self):
        packages = [
            {"id": "app", "name": "metis-app", "targets": [{"name": "metis-app", "kind": ["bin"]}]},
            {"id": "cli", "name": "metis-cli", "targets": [{"name": "metis", "kind": ["bin"]}]},
            {"id": "backend", "name": "metis-backend", "targets": [{"name": "metis_backend", "kind": ["lib"]}]},
            {"id": "frontend", "name": "metis-frontend", "targets": [{"name": "metis_frontend", "kind": ["lib"]}]}]
        metadata = {"packages": packages, "workspace_members": [package["id"] for package in packages]}
        self.assertEqual(verify.application_targets(metadata), [("metis-app", "metis-app"), ("metis-cli", "metis")])
        for index in (2, 3):
            changed = copy.deepcopy(metadata)
            changed["packages"][index]["targets"].append({"name": packages[index]["name"], "kind": ["bin"]})
            with self.subTest(role=packages[index]["name"]), self.assertRaisesRegex(ValueError, "only the application"):
                verify.application_targets(changed)
            changed["packages"][index]["targets"] = [{"name": packages[index]["name"], "kind": ["test"]}]
            with self.subTest(missing_library=packages[index]["name"]), self.assertRaisesRegex(ValueError, "remain a library"):
                verify.application_targets(changed)
        changed = copy.deepcopy(metadata)
        changed["packages"][0]["targets"][0]["name"] = "metis-backend"
        with self.assertRaisesRegex(ValueError, "only the application"):
            verify.application_targets(changed)
        changed = copy.deepcopy(metadata)
        changed["packages"][1]["targets"][0]["name"] = "metis-app"
        with self.assertRaisesRegex(ValueError, "only the application"):
            verify.application_targets(changed)

    def test_input_sensitive_unit_conversion_and_process_separation(self):
        first = distribution.calculation(
            "backend_pid=10\nfrontend_pid=20 rate_ml_hr=0.36 drug_rate_mg_hr=0.72 audit_sequence=2\n"
            "Metis session completed; 3 audit records verified", ["60", "2", "0.2"])
        second = distribution.calculation(
            "backend_pid=30\nfrontend_pid=40 rate_ml_hr=0.6 drug_rate_mg_hr=2.4 audit_sequence=2\n"
            "Metis session completed; 3 audit records verified", ["80", "4", "0.5"])
        self.assertEqual((first["rate_ml_hr"], first["drug_rate_mg_hr"]), (0.36, 0.72))
        self.assertEqual((second["rate_ml_hr"], second["drug_rate_mg_hr"]), (0.6, 2.4))
        self.assertEqual(first["frontend_pid"], 20)
        self.assertEqual(second["backend_pid"], 30)

    def test_rejects_wrong_computation_and_incomplete_session(self):
        template = ("backend_pid=10\nfrontend_pid=20 rate_ml_hr=0.36 drug_rate_mg_hr=0.72 audit_sequence=2\n"
                    "Metis session completed; 3 audit records verified")
        for output in (template.replace("0.36", "0.37"), template.replace("0.72", "0.73"),
                       template.replace("0.36", "nan"), template.replace("0.36", "inf"),
                       template.replace("frontend_pid=20", "frontend_pid=10"),
                       template.replace("audit_sequence=2", "audit_sequence=4"),
                       template.replace("Metis session completed", "Session aborted")):
            with self.subTest(output=output), self.assertRaises(ValueError):
                distribution.calculation(output, ["60", "2", "0.2"])

    def test_inventory_matches_bytes_and_rejects_mutation_or_extra_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            content = b"Input-sensitive payload bytes\x00\xff"
            (root / "assets").mkdir()
            path = root / "assets" / "source.dat"
            path.write_bytes(content)
            inventory = {"files": [{"destination": "assets/source.dat", "bytes": len(content),
                                    "sha256": hashlib.sha256(content).hexdigest()}]}
            self.assertEqual(distribution.verify_payload(root, inventory), ["assets/source.dat"])
            path.write_bytes(content[:-1] + b"a")
            with self.assertRaisesRegex(ValueError, "does not match"):
                distribution.verify_payload(root, inventory)
            path.write_bytes(content)
            (root / "unlisted.txt").write_bytes(b"unexpected")
            with self.assertRaisesRegex(ValueError, "file set"):
                distribution.verify_payload(root, inventory)

    def test_inventory_rejects_missing_duplicate_and_wrong_length(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            (root / "entry.exe").write_bytes(b"payload")
            record = {"destination": "entry.exe", "bytes": 7, "sha256": hashlib.sha256(b"payload").hexdigest()}
            for records in ([], [record, record], [{**record, "bytes": 6}], [{**record, "destination": "absent.exe"}]):
                with self.subTest(records=records), self.assertRaises(ValueError):
                    distribution.verify_payload(root, {"files": records})

    def test_inventory_rejects_path_escape(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            for name in ("", "../escape", "/absolute", "a//b", "a/./b", "C:/Windows", "a\\b", "entry.exe:stream"):
                with self.subTest(name=name), self.assertRaises(ValueError):
                    distribution.destination(root, name)
            self.assertEqual(distribution.destination(root, "assets/scan.dcm"), root / "assets" / "scan.dcm")

    def test_registry_records_actual_uninstall_directory_and_component_versions(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            inventory = {"application": {"version": "0.1.0"}, "files": [{}, {}]}
            values = {"F0": "0.1.0", "F1": "0.1.0", "InstallLocation": str(root / "installed")}
            distribution.verify_registration(values, inventory, root / "installed")
            for changed in ({**values, "InstallLocation": str(root / "default-location")},
                            {**values, "F1": "0.0.9"}, {**values, "unexpected": "0.1.0"},
                            {"F0": "0.1.0", "F1": "0.1.0"}):
                with self.subTest(changed=changed), self.assertRaises(ValueError):
                    distribution.verify_registration(changed, inventory, root / "installed")

    def test_retention_refuses_other_directories(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            preserved = root / "user.txt"
            preserved.write_bytes(b"unique")
            with self.assertRaisesRegex(ValueError, "output must be"):
                distribution.prepare(root)
            self.assertEqual(preserved.read_bytes(), b"unique")

    def test_hardlink_inventory_cannot_alias_user_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            source = root / "user.txt"
            source.write_bytes(b"unique")
            alias = root / "alias.txt"
            alias.hardlink_to(source)
            with self.assertRaisesRegex(ValueError, "Multiply linked"):
                distribution.digest(alias)
            self.assertEqual(source.read_bytes(), b"unique")

    def test_read_only_tool_accepts_cargo_hardlinks_but_retention_rejects_them(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            compiler_artifact = root / "compiler.exe"
            compiler_artifact.write_bytes(b"compiler-produced executable bytes")
            exported_tool = root / "metis.exe"
            exported_tool.hardlink_to(compiler_artifact)
            tool = distribution.unredirected(exported_tool)
            self.assertEqual(tool.read_bytes(), compiler_artifact.read_bytes())
            with self.assertRaisesRegex(ValueError, "Multiply linked"):
                distribution.unlinked(exported_tool)
            with self.assertRaisesRegex(ValueError, "Multiply linked"):
                distribution.tree_files(root)
            self.assertEqual(compiler_artifact.read_bytes(), b"compiler-produced executable bytes")

    def test_json_size_and_workflow_report(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            workflow = distribution.Workflow(root)
            report = distribution.read_json(root / "workflow.json")
            self.assertEqual(report, {"schema": 1, "status": "running", "commands": [], "calculations": []})
            path = root / "oversize.json"
            with path.open("wb") as stream:
                stream.truncate(distribution.LOG_LIMIT + 1)
            with self.assertRaisesRegex(ValueError, "16 MiB"):
                distribution.read_json(path)
            workflow.report["status"] = "failed"
            workflow.save()
            self.assertEqual(json.loads((root / "workflow.json").read_text())["status"], "failed")


if __name__ == "__main__":
    unittest.main()
