"""Exercise archive identity and public-index comparison without publishing."""

import copy
import importlib.util
import io
import os
import pathlib
import re
import subprocess
import sys
import tarfile
import tempfile
import textwrap
import unittest
import zipfile


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "release_identity.py"
SPEC = importlib.util.spec_from_file_location("release_identity", SCRIPT)
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)


class ReleaseIdentityTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="metis-release-identity-")
        self.addCleanup(directory.cleanup)
        self.directory = pathlib.Path(directory.name)
        self.wheel = self.directory / "metis_ui-0.1.0-cp310-abi3-linux_x86_64.whl"
        self.sdist = self.directory / "metis_ui-0.1.0.tar.gz"
        self.write_wheel()
        self.write_sdist()
        reference = tempfile.TemporaryDirectory(prefix="metis-source-reference-")
        self.addCleanup(reference.cleanup)
        self.reference = pathlib.Path(reference.name) / self.sdist.name
        self.reference.write_bytes(self.sdist.read_bytes())

    def write_wheel(self, *, project="metis-ui", version="0.1.0", extra_metadata=False):
        with zipfile.ZipFile(self.wheel, "w") as archive:
            archive.writestr(
                f"metis_ui-{version}.dist-info/METADATA",
                f"Metadata-Version: 2.4\nName: {project}\nVersion: {version}\n\nSI quantities.\n",
            )
            if extra_metadata:
                archive.writestr("unexpected.dist-info/METADATA", "Name: unrelated\nVersion: 0.1.0\n")

    def write_sdist(self, *, project="metis-ui", version="0.1.0", duplicate=False,
                    source="pub fn value() -> u8 { 7 }"):
        with tarfile.open(self.sdist, "w:gz") as archive:
            records = [
                (f"metis_ui-{version}/PKG-INFO", f"Metadata-Version: 2.4\nName: {project}\nVersion: {version}\n"),
                (f"metis_ui-{version}/dependency/PKG-INFO", "Name: dependency\nVersion: 9.0.0\n"),
                (f"metis_ui-{version}/src/lib.rs", source),
            ]
            if duplicate:
                records.append(("other/PKG-INFO", "Name: unrelated\nVersion: 0.1.0\n"))
            for name, content in records:
                data = content.encode()
                member = tarfile.TarInfo(name)
                member.size = len(data)
                archive.addfile(member, io.BytesIO(data))

    def receipt(self):
        return release.create_receipt(self.directory, "0.1.0", "a" * 40, self.reference)

    def published(self, receipt):
        return {
            "info": {"name": "metis-ui", "version": "0.1.0"},
            "urls": [
                {"filename": name, "digests": {"sha256": row["sha256"]}, "size": row["size"], "yanked": False}
                for name, row in receipt["files"].items()
            ],
        }

    def test_receipt_records_both_actual_archives_and_source(self):
        import hashlib

        receipt = self.receipt()
        self.assertEqual(receipt["source"], {
            "repository": "ryancinsight/metis", "commit": "a" * 40, "workflow": "python-release.yml",
        })
        self.assertEqual(receipt["project"], "metis-ui")
        self.assertEqual(receipt["version"], "0.1.0")
        self.assertEqual(receipt["source_distribution"]["reference_sha256"],
                         hashlib.sha256(self.reference.read_bytes()).hexdigest())
        self.assertEqual(set(receipt["files"]), {self.wheel.name, self.sdist.name})
        for path in [self.wheel, self.sdist]:
            self.assertEqual(receipt["files"][path.name], {
                "sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "size": path.stat().st_size,
            })
        release.verify_index(receipt, self.published(receipt))

    def test_project_name_comparison_accepts_python_normalization(self):
        self.write_wheel(project="Metis_UI")
        self.write_sdist(project="METIS.UI")
        self.reference.write_bytes(self.sdist.read_bytes())
        self.receipt()

    def test_cargo_versions_use_canonical_python_metadata_and_keep_raw_tag_version(self):
        for cargo, python in [
            ("0.2.0", "0.2.0"),
            ("0.2.0-alpha.1", "0.2.0a1"),
            ("0.2.0-beta.2", "0.2.0b2"),
            ("0.2.0-rc.1", "0.2.0rc1"),
            ("0.2.0-dev.3", "0.2.0.dev3"),
            ("0.2.0-rc.1+build.001", "0.2.0rc1+build.1"),
        ]:
            with self.subTest(cargo=cargo):
                for path in self.directory.iterdir():
                    path.unlink()
                self.wheel = self.directory / f"metis_ui-{python}-cp310-abi3-linux_x86_64.whl"
                self.sdist = self.directory / f"metis_ui-{python}.tar.gz"
                self.write_wheel(version=python)
                self.write_sdist(version=python)
                self.reference.write_bytes(self.sdist.read_bytes())
                receipt = release.create_receipt(self.directory, cargo, "a" * 40, self.reference)
                self.assertEqual(receipt["version"], python)
                self.assertEqual(receipt["cargo_version"], cargo)
                index = self.published(receipt)
                index["info"]["version"] = python
                release.verify_index(receipt, index)

    def test_prerelease_checks_reject_wrong_phase_number_and_noncanonical_metadata(self):
        for metadata_version in ["0.2.0rc2", "0.2.0b1", "0.2.0", "0.2.0-rc.1"]:
            with self.subTest(version=metadata_version):
                self.write_wheel(version=metadata_version)
                self.write_sdist(version=metadata_version)
                with self.assertRaisesRegex(ValueError, "version"):
                    release.create_receipt(self.directory, "0.2.0-rc.1", "a" * 40, self.reference)

    def test_rejects_cargo_prerelease_that_maturin_cannot_convert_to_pep440(self):
        with self.assertRaises(ValueError):
            release.create_receipt(self.directory, "0.2.0-canary.1", "a" * 40, self.reference)

    def test_source_check_compares_contents_and_modes_while_ignoring_archive_timestamps(self):
        def write(path, *, mtime, source=b"pub fn value() -> u8 { 7 }", mode=0o644, extra=False):
            with tarfile.open(path, "w:gz") as archive:
                records = [("metis_ui-0.1.0/src/lib.rs", source)]
                if extra:
                    records.append(("metis_ui-0.1.0/src/other.rs", b"pub const OTHER: u8 = 9;"))
                for name, content in records:
                    member = tarfile.TarInfo(name)
                    member.size, member.mtime, member.mode = len(content), mtime, mode
                    archive.addfile(member, io.BytesIO(content))

        write(self.reference, mtime=100)
        write(self.sdist, mtime=200)
        self.assertNotEqual(self.reference.read_bytes(), self.sdist.read_bytes())
        release.verify_source_archive(self.sdist, self.reference)
        for override in [dict(source=b"pub fn value() -> u8 { 8 }"), dict(mode=0o755), dict(extra=True)]:
            with self.subTest(override=override):
                write(self.sdist, mtime=200, **override)
                with self.assertRaisesRegex(ValueError, "source archive differs"):
                    release.verify_source_archive(self.sdist, self.reference)

    def test_receipt_rejects_changed_sdist_source_despite_equal_project_and_version(self):
        self.write_sdist(source="pub fn value() -> u8 { 8 }")
        self.assertEqual(release.metadata(self.sdist)["Name"], "metis-ui")
        self.assertEqual(release.metadata(self.sdist)["Version"], "0.1.0")
        with self.assertRaisesRegex(ValueError, "source archive differs"):
            self.receipt()

    def test_source_check_rejects_ambiguous_or_unsafe_archive_members(self):
        for name, duplicate in [("metis_ui-0.1.0/src/lib.rs", True), ("../src/lib.rs", False), ("/src/lib.rs", False)]:
            with self.subTest(name=name, duplicate=duplicate):
                with tarfile.open(self.sdist, "w:gz") as archive:
                    for _ in range(2 if duplicate else 1):
                        member = tarfile.TarInfo(name)
                        member.size = 4
                        archive.addfile(member, io.BytesIO(b"code"))
                with self.assertRaisesRegex(ValueError, "duplicate paths|unsafe path"):
                    release.verify_source_archive(self.sdist, self.reference)

    def test_caller_pins_source_to_event_commit_and_checks_the_shared_sdist(self):
        workflow = (SCRIPT.parents[1] / ".github/workflows/python-release.yml").read_text()
        checkouts = re.findall(r"(?ms)^      - uses: actions/checkout@[^\n]+\n(?P<settings>.*?)(?=^      -|\Z)", workflow)
        self.assertEqual(len(checkouts), 3)
        for settings in checkouts:
            self.assertIn("ref: ${{ github.sha }}", settings)
            self.assertNotIn("github.event.release.tag_name", settings)
        self.assertIn("maturin==1.14.1", workflow)
        self.assertIn("--source-dist", workflow)
        self.assertLess(workflow.index("maturin sdist"), workflow.index("release_identity.py create"))

    def version_reader(self):
        workflow = SCRIPT.parents[1] / ".github/workflows/python-release.yml"
        match = re.search(
            r"(?ms)^          python3 - <<'PY'\n(?P<body>.*?)^          PY$",
            workflow.read_text(encoding="utf-8"),
        )
        self.assertIsNotNone(match)
        return textwrap.dedent(match.group("body"))

    def test_workflow_keeps_exact_cargo_tag_and_outputs_python_version_separately(self):
        script = self.version_reader()
        package = self.directory / "crates/metis-python"
        package.mkdir(parents=True)
        (package / "Cargo.toml").write_text(
            '[package]\nname = "metis-python"\nversion.workspace = true\nedition = "2024"\n',
            encoding="utf-8",
        )
        (self.directory / "Cargo.toml").write_text(
            '[workspace.package]\nversion = "0.2.0-rc.1"\n', encoding="utf-8",
        )
        output = self.directory / "github-output"
        environment = {**os.environ, "PYTHONPATH": str(SCRIPT.parents[1]), "EVENT": "release", "GITHUB_OUTPUT": str(output)}
        for tag, accepted in [
            ("metis-ui-v0.2.0-rc.1", True),
            ("metis-ui-v0.2.0rc1", False),
            ("metis-ui-v0.2.0-rc.2", False),
        ]:
            with self.subTest(tag=tag):
                output.unlink(missing_ok=True)
                result = subprocess.run(
                    [sys.executable, "-c", script], cwd=self.directory,
                    env={**environment, "TAG": tag}, capture_output=True, text=True, timeout=10,
                )
                if accepted:
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(output.read_text().splitlines(), [
                        "version=0.2.0-rc.1", "python-version=0.2.0rc1",
                    ])
                else:
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("does not name metis-ui", result.stderr)
                    self.assertFalse(output.exists())

    def test_manual_reader_uses_actual_workspace_version_and_rejects_independent_binding(self):
        package = self.directory / "crates/metis-python"
        package.mkdir(parents=True)
        (self.directory / "Cargo.toml").write_text(
            '[workspace.package]\nversion = "0.5.0"\n', encoding="utf-8",
        )
        output = self.directory / "github-output"
        environment = {
            **os.environ, "PYTHONPATH": str(SCRIPT.parents[1]),
            "EVENT": "workflow_dispatch", "TAG": "", "GITHUB_OUTPUT": str(output),
        }
        for declaration, accepted in [('version.workspace = true', True), ('version = "9.0.0"', False)]:
            with self.subTest(declaration=declaration):
                output.unlink(missing_ok=True)
                (package / "Cargo.toml").write_text(
                    f'[package]\nname = "metis-python"\n{declaration}\n', encoding="utf-8",
                )
                result = subprocess.run(
                    [sys.executable, "-c", self.version_reader()], cwd=self.directory,
                    env=environment, capture_output=True, text=True, timeout=10,
                )
                if accepted:
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(output.read_text().splitlines(), ["version=0.5.0", "python-version=0.5.0"])
                else:
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("must inherit its workspace version", result.stderr)
                    self.assertFalse(output.exists())

    def test_rejects_wrong_project_or_version_in_either_archive(self):
        for writer in [self.write_wheel, self.write_sdist]:
            for override, message in [({"project": "aequitas"}, "name"), ({"version": "0.2.0"}, "version")]:
                with self.subTest(writer=writer.__name__, override=override):
                    writer(**override)
                    with self.assertRaisesRegex(ValueError, message):
                        self.receipt()
                    writer()

    def test_rejects_missing_wheel_or_sdist_and_unexpected_upload_entry(self):
        for path in [self.wheel, self.sdist]:
            with self.subTest(missing=path.name):
                data = path.read_bytes()
                path.unlink()
                with self.assertRaises(ValueError):
                    self.receipt()
                path.write_bytes(data)
        (self.directory / "source-receipt.json").write_text("{}", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "unexpected entry"):
            self.receipt()

    def test_rejects_ambiguous_root_metadata_and_short_source_commit(self):
        self.write_wheel(extra_metadata=True)
        with self.assertRaisesRegex(ValueError, "one wheel metadata"):
            self.receipt()
        self.write_wheel()
        self.write_sdist(duplicate=True)
        with self.assertRaisesRegex(ValueError, "one root source metadata"):
            self.receipt()
        self.write_sdist()
        with self.assertRaisesRegex(ValueError, "full Git SHA"):
            release.create_receipt(self.directory, "0.1.0", "aaaaaaa", self.reference)

    def test_rejects_index_identity_file_set_checksum_size_and_yank_changes(self):
        receipt = self.receipt()
        original = self.published(receipt)
        changed = []
        for field, value in [("name", "unrelated"), ("version", "0.2.0")]:
            entry = copy.deepcopy(original)
            entry["info"][field] = value
            changed.append(entry)
        for field, value in [("digests", {"sha256": "b" * 64}), ("size", 1), ("yanked", True)]:
            entry = copy.deepcopy(original)
            entry["urls"][0][field] = value
            changed.append(entry)
        changed.extend([
            {**original, "urls": original["urls"][:1]},
            {**original, "urls": original["urls"] + [original["urls"][0]]},
            {**original, "urls": original["urls"] + [{**original["urls"][0], "filename": "extra.whl"}]},
        ])
        for entry in changed:
            with self.subTest(published=entry):
                with self.assertRaises(ValueError):
                    release.verify_index(receipt, entry)


if __name__ == "__main__":
    unittest.main()
