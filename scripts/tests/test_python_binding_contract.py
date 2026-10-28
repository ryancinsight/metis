"""Verify Python ABI, wheel metadata and the release contract."""
import json
import pathlib
import re
import runpy
import sys
import tempfile
import unittest
import zipfile

SCRIPTS = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))


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
            'name = "metis-ui"',
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
        wheel = root / f"metis_ui-0.1.0-{tag}.whl"
        dist_info = "metis_ui-0.1.0.dist-info"
        members = {
            "metis/__init__.py": b"from ._metis import Application\n",
            "metis/_metis.pyi": b"class Application: ...\n",
            "metis/py.typed": b"",
            "metis/_metis.cp39-win_amd64.pyd": b"extension",
            f"{dist_info}/METADATA": (
                b"Metadata-Version: 2.1\n"
                b"Name: metis-ui\n"
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
            "workflow_dispatch:",
            "github.event_name == 'workflow_dispatch'",
            "metis-ui-v",
            "distribution: metis-ui",
            "import-name: metis",
            "manifest-path: crates/metis-python/Cargo.toml",
            "abi3: true",
            "abi3-python: \"3.9\"",
            "python-test-path: crates/metis-python/tests",
            "free-threaded: true",
            "free-threaded-versions: '[\"3.14t\", \"3.15t\"]'",
            "abi3t: true",
            "abi3t-python: \"3.15t\"",
            "abi3t-features: abi3t",
            "verification: ${{ github.event_name == 'workflow_dispatch' }}",
            "id-token: write",
            "ryancinsight/atlas/.github/workflows/python-wheels.yml@227ff172612b8aff553be22be89fabdf71838a03",
            "pypa/gh-action-pypi-publish@ba38be9e461d3875417946c167d0b5f3d385a247",
            "needs: [wheels, identity]",
            "name: release-identity",
            "name: pypi-source-receipt",
            '"metis-ui==$VERSION"',
        ):
            with self.subTest(fragment=fragment):
                self.assertIn(fragment, self.workflow)
        self.assertIn(
            "if: github.event_name == 'release' && startsWith(github.event.release.tag_name, 'metis-ui-v')",
            self.workflow,
        )
        for forbidden in ("secrets:", "PYPI_TOKEN", "TWINE_PASSWORD", "private_key", "signing-key", "GPG", "SSH_PRIVATE_KEY"):
            with self.subTest(forbidden=forbidden):
                self.assertNotIn(forbidden, self.workflow)

    def test_free_threaded_feature_selects_pyo3_abi3t(self):
        self.assertNotIn("pyo3/extension-module", self.manifest)
        for fragment in (
            'abi3t = ["pyo3/macros", "pyo3/abi3t-py315"]',
            'abi3t-features: abi3t',
        ):
            with self.subTest(fragment=fragment):
                self.assertIn(fragment, self.manifest if fragment.startswith("abi3t =") else self.workflow)
        references = re.findall(
            r"^\s*(?:-\s+)?uses:\s+([^@\s]+)@([^\s#]+)",
            self.workflow,
            re.MULTILINE,
        )
        # The archive/source receipt and index read-back add pinned checkout,
        # Python and artifact actions; retain the full pin check for every use.
        self.assertEqual(len(references), 13)
        for action, revision in references:
            with self.subTest(action=action):
                self.assertRegex(revision, r"\A[0-9a-f]{40}\Z")
