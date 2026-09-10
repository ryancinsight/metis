"""Exercise gate bootstrap failures without invoking a Rust toolchain."""
import json
import os
import pathlib
import re
import runpy
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile


SCRIPTS = pathlib.Path(__file__).resolve().parents[1]


class NeutralWorkspaceTests(unittest.TestCase):
    """Keep the standalone gate independent of an Atlas parent overlay."""

    def test_neutral_workspace_hides_ancestor_cargo_configuration(self):
        module = runpy.run_path(str(SCRIPTS / "verify.py"))
        physical = module["PHYSICAL_ROOT"]
        has_configuration = any(
            (directory / ".cargo" / name).is_file()
            for directory in (physical, *physical.parents)
            for name in ("config", "config.toml")
        )

        with module["neutral_workspace"]() as root:
            self.assertTrue((root / "Cargo.toml").is_file())
            if os.name == "nt" and has_configuration:
                self.assertNotEqual(root.drive, physical.drive)
                self.assertFalse((root / ".cargo" / "config.toml").is_file())
            else:
                self.assertEqual(root, physical)


class FormatNeutralWorkspaceTests(unittest.TestCase):
    """Keep DICOM parsing and medical display in the RITK boundary."""

    @classmethod
    def setUpClass(cls):
        cls.verify = runpy.run_path(str(SCRIPTS / "verify.py"))

    def test_format_neutral_workspace_accepts_host_packages(self):
        metadata = {
            "packages": [
                {"id": "core", "name": "metis-core", "dependencies": []},
                {"id": "web", "name": "metis-web", "dependencies": [{"name": "moirai-http"}]},
            ],
            "workspace_members": ["core", "web"],
        }
        self.verify["format_neutral_workspace"](metadata)

    def test_format_neutral_workspace_rejects_dicom_dependency(self):
        metadata = {
            "packages": [
                {"id": "host", "name": "metis-platform", "dependencies": [{"name": "ritk-dicom"}]},
            ],
            "workspace_members": ["host"],
        }
        with self.assertRaisesRegex(ValueError, "DICOM belongs in RITK"):
            self.verify["format_neutral_workspace"](metadata)


class BootstrapEvidenceTests(unittest.TestCase):
    """A failed new invocation must never retain an earlier green report."""

    def setUp(self):
        fixture = tempfile.TemporaryDirectory(prefix="metis-gate-test-")
        self.addCleanup(fixture.cleanup)
        self.root = pathlib.Path(fixture.name)
        scripts = self.root / "scripts"
        scripts.mkdir()
        for name in ("verify.py", "visual.py"):
            shutil.copyfile(SCRIPTS / name, scripts / name)
        (self.root / "Cargo.lock").write_text("version = 4\n", encoding="utf-8")
        (self.root / "rust-toolchain.toml").write_text(
            '[toolchain]\nchannel = "1.97.0"\n', encoding="utf-8"
        )
        output = self.root / "output"
        output.mkdir()
        self.report = output / "verification.json"
        self.previous = json.dumps({
            "schema": 1,
            "status": "passed",
            "stages": {"tests": "passed"},
            "revision": "previous-revision",
            "lock_sha256": "previous-lock",
            "visual": {"status": "passed"},
        }).encode("utf-8")
        self.report.write_bytes(self.previous)

    def invoke(self, *arguments):
        environment = os.environ.copy()
        environment["RUSTC"] = "deliberate-bootstrap-override"
        environment["PYTHONPATH"] = ""
        # A regression reaching tool execution cannot discover the host toolchain.
        environment["PATH"] = str(self.root / "unavailable-tools")
        return subprocess.run(
            [sys.executable, "-S", str(self.root / "scripts" / "verify.py"), *arguments],
            cwd=self.root,
            env=environment,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=10,
            check=False,
        )

    def assert_failed_report(self, result, diagnostic):
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn(diagnostic, result.stdout + result.stderr)
        report = json.loads(self.report.read_text(encoding="utf-8"))
        self.assertEqual(set(report), {"schema", "status", "stages", "commands", "error"})
        self.assertEqual(report["schema"], 1)
        self.assertEqual(report["status"], "failed")
        self.assertEqual(report["stages"], {})
        self.assertEqual(report["commands"], {})
        self.assertIn(report["error"], result.stdout + result.stderr)
        self.assertNotEqual(report["error"], "")

    def test_compiler_override_replaces_prior_success(self):
        result = self.invoke()
        self.assert_failed_report(result, "Unset RUSTC/RUSTDOC overrides")
        self.assertIn(
            "Unset RUSTC/RUSTDOC overrides",
            json.loads(self.report.read_text(encoding="utf-8"))["error"],
        )

    def test_malformed_toolchain_replaces_prior_success(self):
        (self.root / "rust-toolchain.toml").write_text("[toolchain\n", encoding="utf-8")
        self.assert_failed_report(self.invoke(), "TOMLDecodeError")

    def test_missing_visual_module_replaces_prior_success(self):
        (self.root / "scripts" / "visual.py").unlink()
        self.assert_failed_report(self.invoke(), "No module named 'visual'")

    def test_help_preserves_prior_report(self):
        result = self.invoke("--help")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("--update-snapshots", result.stdout)
        self.assertEqual(result.stderr, "")
        self.assertEqual(self.report.read_bytes(), self.previous)

    def outside_sentinel(self):
        outside = tempfile.TemporaryDirectory(prefix="metis-outside-test-")
        self.addCleanup(outside.cleanup)
        path = pathlib.Path(outside.name) / "verification.json"
        path.write_bytes(b"Unique outside work must survive a rejected gate invocation.\n")
        return path

    def assert_rejected_alias(self, sentinel):
        original = sentinel.read_bytes()
        result = self.invoke()
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("Unsafe", result.stdout + result.stderr)
        self.assertEqual(sentinel.read_bytes(), original)

    def test_report_hardlink_preserves_outside_file(self):
        sentinel = self.outside_sentinel()
        self.report.unlink()
        os.link(sentinel, self.report)
        self.assertEqual(self.report.stat().st_nlink, 2)
        self.assert_rejected_alias(sentinel)

    def test_output_directory_link_preserves_outside_file(self):
        sentinel = self.outside_sentinel()
        output = self.report.parent
        self.report.unlink()
        output.rmdir()
        if os.name == "nt":
            # Junction creation needs no administrator symlink privilege. Paths
            # travel through environment values rather than PowerShell source.
            shell = shutil.which("powershell.exe")
            self.assertIsNotNone(shell, "Windows PowerShell creates the real junction fixture")
            environment = os.environ.copy()
            environment["METIS_TEST_LINK"] = str(output)
            environment["METIS_TEST_TARGET"] = str(sentinel.parent)
            result = subprocess.run(
                [shell, "-NoProfile", "-NonInteractive", "-Command",
                 "$ErrorActionPreference = 'Stop'; New-Item -ItemType Junction "
                 "-Path $env:METIS_TEST_LINK -Value $env:METIS_TEST_TARGET | Out-Null"],
                env=environment,
                capture_output=True,
                text=True,
                timeout=10,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.addCleanup(output.rmdir)
        else:
            output.symlink_to(sentinel.parent, target_is_directory=True)
            self.addCleanup(output.unlink)
        self.assertEqual(output.resolve(), sentinel.parent.resolve())
        self.assert_rejected_alias(sentinel)

class WorkflowContractTests(unittest.TestCase):
    """Keep the hosted workflow aligned with the committed local gate."""

    @classmethod
    def setUpClass(cls):
        cls.workflow = SCRIPTS.parent / ".github" / "workflows" / "ci.yml"
        cls.source = cls.workflow.read_text(encoding="utf-8")

    def test_one_pinned_pipeline_covers_supported_targets(self):
        required = (
            "pull_request:",
            "merge_group:",
            "schedule:",
            "workflow_dispatch:",
            "push:",
            "branches: [feat/process-foundation]",
            "permissions:\n  contents: read",
            "concurrency:",
            "cancel-in-progress:",
            "python scripts/verify.py",
            "taiki-e/install-action@a6b2e2dcd845ddd7f509ce4f3ed3d922b80cc5d9",
            "cargo-nextest@0.9.143",
            "cargo-deny@0.20.2",
            "wasm-bindgen@0.2.128",
            "checksum: true",
            "cargo fetch --locked --manifest-path Cargo.toml",
            "cargo-deny fetch db",
            "output/verification.json",
            "output/build-links.json",
            "if-no-files-found: ignore",
            "workflow-lint:",
            "lockfile:",
            "adr-index:",
            "semver:",
            "semver-gate.yml",
            "fuzz:",
            "cargo fuzz run --target x86_64-unknown-linux-gnu protocol",
            "-max_total_time=300 -rss_limit_mb=2048 -timeout=25",
        )
        for fragment in required:
            with self.subTest(fragment=fragment):
                self.assertIn(fragment, self.source)

    def test_gate_tools_use_release_binaries_with_checksums(self):
        self.assertNotIn("cargo install cargo-nextest", self.source)
        self.assertNotIn("cargo install cargo-deny", self.source)
        self.assertNotIn("cargo install wasm-bindgen-cli", self.source)
        self.assertIn("checksum: true", self.source)

    def test_external_actions_and_guards_are_revision_pinned(self):
        references = re.findall(r"^\s*(?:-\s+)?uses:\s+([^@\s]+)@([^\s#]+)", self.source, re.MULTILINE)
        self.assertGreaterEqual(len(references), 4)
        for action, revision in references:
            with self.subTest(action=action):
                self.assertRegex(revision, r"\A[0-9a-f]{40}\Z")
        atlas = "848e6649c52e8226a9abf7bc336f8cbf0e39ba08"
        for guard in ("workflow-lint.yml", "lockfile-guard.yml", "adr-index-guard.yml"):
            with self.subTest(guard=guard):
                self.assertIn(f"ryancinsight/atlas/.github/workflows/{guard}@{atlas}", self.source)

    def test_draft_pull_requests_and_unsupported_hosts_are_excluded(self):
        draft_guard = "if: github.event_name != 'pull_request' || github.event.pull_request.draft == false"
        self.assertEqual(self.source.count(draft_guard), 3)
        self.assertIn("if: github.event_name != 'schedule' && (github.event_name != 'pull_request'", self.source)
        self.assertIn("github.event_name == 'pull_request' && github.event.pull_request.draft == false", self.source)
        self.assertNotIn("pull_request_target", self.source)
        self.assertIn("runs-on: windows-latest", self.source)

    def test_scheduled_fuzz_campaign_is_pinned_and_bounded(self):
        for fragment in (
            "name: LibFuzzer parser campaign",
            "if: github.event_name == 'schedule' || github.event_name == 'workflow_dispatch'",
            "timeout-minutes: 10",
            "RUSTUP_TOOLCHAIN: nightly-2026-08-01",
            "toolchain: nightly-2026-08-01",
            "targets: x86_64-unknown-linux-gnu",
            "tool: cargo-fuzz@0.13.2",
            "checksum: true",
            "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02",
            "metis-fuzz-artifacts-${{ github.run_id }}",
        ):
            with self.subTest(fragment=fragment):
                self.assertIn(fragment, self.source)
        self.assertIn("working-directory: fuzz", self.source)
        self.assertIn("cargo metadata --locked --format-version 1 --no-deps", self.source)
        self.assertIn("-print_final_stats=1", self.source)
        self.assertIn("if: failure()", self.source)


class ReleaseWorkflowContractTests(unittest.TestCase):
    """Keep registry publication tokenless and release-only."""

    @classmethod
    def setUpClass(cls):
        cls.workflow = SCRIPTS.parent / ".github" / "workflows" / "rust-release.yml"
        cls.source = cls.workflow.read_text(encoding="utf-8")

    def test_release_caller_uses_atlas_oidc_workflows(self):
        required = (
            "release:\n    types: [published]",
            "workflow_dispatch:",
            "release-gate: true",
            "needs: identify",
            "package: ${{ needs.identify.outputs.package }}",
            "metis|metis-backend|metis-core|metis-frontend|metis-ipc|metis-platform|metis-ui-lang|metis-app|metis-web|metis-python",
            "id-token: write",
            "ryancinsight/atlas/.github/workflows/semver-gate.yml@848e6649c52e8226a9abf7bc336f8cbf0e39ba08",
            "ryancinsight/atlas/.github/workflows/crates-publish.yml@848e6649c52e8226a9abf7bc336f8cbf0e39ba08",
        )
        for fragment in required:
            with self.subTest(fragment=fragment):
                self.assertIn(fragment, self.source)

    def test_caller_has_no_registry_secret_or_implicit_publish_trigger(self):
        self.assertNotIn("secrets:", self.source)
        self.assertNotIn("CARGO_REGISTRY_TOKEN", self.source)
        for forbidden in ("private_key", "signing-key", "GPG", "SSH_PRIVATE_KEY"):
            with self.subTest(forbidden=forbidden):
                self.assertNotIn(forbidden, self.source)
        self.assertNotIn("push:", self.source)
        self.assertIn(
            "validate:\n    needs: [identify, semver]\n    if: github.event_name == 'workflow_dispatch'",
            self.source,
        )
        self.assertIn(
            "publish:\n    needs: [identify, semver]\n    if: github.event_name == 'release' && startsWith(github.event.release.tag_name, 'crate-')",
            self.source,
        )
        dispatch_block = self.source.split("  validate:\n", 1)[1].split("  publish:\n", 1)[0]
        self.assertNotIn("id-token: write", dispatch_block)
        references = re.findall(
            r"^\s*(?:-\s+)?uses:\s+([^@\s]+)@([^\s#]+)",
            self.source,
            re.MULTILINE,
        )
        self.assertEqual(len(references), 3)
        for action, revision in references:
            with self.subTest(action=action):
                self.assertRegex(revision, r"\A[0-9a-f]{40}\Z")
        self.assertNotIn("metis-cli", self.source)


class PythonBindingContractTests(unittest.TestCase):
    """Keep the PyO3 package, wheel metadata and OIDC caller aligned."""

    @classmethod
    def setUpClass(cls):
        cls.root = SCRIPTS.parent
        cls.validate_wheel_surface = staticmethod(
            runpy.run_path(str(SCRIPTS / "python_binding.py"))["validate_wheel_surface"]
        )
        cls.manifest = (cls.root / "crates" / "metis-python" / "Cargo.toml").read_text(encoding="utf-8")
        cls.pyproject = (cls.root / "crates" / "metis-python" / "pyproject.toml").read_text(encoding="utf-8")
        cls.workflow = (cls.root / ".github" / "workflows" / "python-release.yml").read_text(encoding="utf-8")

    def test_package_is_abi3_typed_and_workspace_owned(self):
        for fragment in (
            'name = "metis-python"',
            'name = "_metis"',
            'crate-type = ["cdylib"]',
            'metis-backend.workspace = true',
            'pyo3.workspace = true',
            'name = "metis-rs"',
            'requires-python = ">=3.9"',
            'module-name = "metis._metis"',
            'python-source = "python"',
        ):
            with self.subTest(fragment=fragment):
                self.assertIn(fragment, self.manifest + self.pyproject)
        package = self.root / "crates" / "metis-python" / "python" / "metis"
        self.assertTrue((package / "__init__.py").is_file())
        self.assertTrue((package / "_metis.pyi").is_file())
        self.assertTrue((package / "py.typed").is_file())
        # The declaration is what a free-threaded interpreter reads on
        # import; which file in the crate carries it is a module-tree
        # decision, since `#[pymodule]` emits `PyInit__metis` as
        # `#[no_mangle]` from wherever it sits. Pinning the assertion to one
        # file made returning `lib.rs` to a manifest fail a contract that
        # move preserved, and re-pinning it to `module.rs` would fail the
        # next such move the same way.
        binding_source = "\n".join(
            path.read_text(encoding="utf-8")
            for path in sorted(
                (self.root / "crates" / "metis-python" / "src").rglob("*.rs")
            )
        )
        self.assertIn("#[pymodule(gil_used = false)]", binding_source)

    def write_wheel_fixture(self, tag, extra_members=()):
        temporary = tempfile.TemporaryDirectory(prefix="metis-wheel-contract-")
        self.addCleanup(temporary.cleanup)
        root = pathlib.Path(temporary.name)
        wheel = root / f"metis_rs-0.1.0-{tag}.whl"
        dist_info = "metis_rs-0.1.0.dist-info"
        members = {
            "metis/__init__.py": b"from ._metis import Application\n",
            "metis/_metis.pyi": b"class Application: ...\n",
            "metis/py.typed": b"",
            "metis/_metis.cp39-win_amd64.pyd": b"extension",
            f"{dist_info}/METADATA": (
                b"Metadata-Version: 2.1\n"
                b"Name: metis-rs\n"
                b"Version: 0.1.0\n"
                b"Requires-Python: >=3.9\n"
                b"Classifier: Typing :: Typed\n"
            ),
            f"{dist_info}/WHEEL": (
                b"Wheel-Version: 1.0\n"
                b"Generator: contract-test\n"
                b"Root-Is-Purelib: false\n"
                + f"Tag: {tag}\n".encode()
            ),
        }
        members.update(extra_members)
        with zipfile.ZipFile(wheel, "w") as archive:
            for name, content in members.items():
                archive.writestr(name, content)
        return wheel

    def test_wheel_validation_rejects_unexpected_native_extension(self):
        wheel = self.write_wheel_fixture(
            "cp39-abi3-win_amd64",
            {"metis/extra.pyd": b"unexpected"},
        )
        with self.assertRaisesRegex(SystemExit, "expected one metis native extension"):
            self.validate_wheel_surface(wheel)

    def test_wheel_validation_rejects_substring_abi_tag(self):
        wheel = self.write_wheel_fixture("cp39-notabi3-win_amd64")
        with self.assertRaisesRegex(SystemExit, "exact stable abi3 tag"):
            self.validate_wheel_surface(wheel)

    def test_release_caller_is_tokenless_and_uses_atlas_wheels(self):
        for fragment in (
            "release:\n    types: [published]",
            "metis-python-v",
            "distribution: metis-rs",
            "import-name: metis",
            "manifest-path: crates/metis-python/Cargo.toml",
            "abi3: true",
            "abi3-python: \"3.9\"",
            "python-test-path: crates/metis-python/tests",
            "id-token: write",
            "ryancinsight/atlas/.github/workflows/python-wheels.yml@848e6649c52e8226a9abf7bc336f8cbf0e39ba08",
            "pypa/gh-action-pypi-publish@ba38be9e461d3875417946c167d0b5f3d385a247",
        ):
            with self.subTest(fragment=fragment):
                self.assertIn(fragment, self.workflow)
        for forbidden in ("secrets:", "PYPI_TOKEN", "TWINE_PASSWORD", "private_key", "signing-key", "GPG", "SSH_PRIVATE_KEY"):
            with self.subTest(forbidden=forbidden):
                self.assertNotIn(forbidden, self.workflow)
        references = re.findall(
            r"^\s*(?:-\s+)?uses:\s+([^@\s]+)@([^\s#]+)",
            self.workflow,
            re.MULTILINE,
        )
        self.assertEqual(len(references), 3)
        for action, revision in references:
            with self.subTest(action=action):
                self.assertRegex(revision, r"\A[0-9a-f]{40}\Z")


if __name__ == "__main__":
    unittest.main()
