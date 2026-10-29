"""Exercise real Cargo artifact reuse between native verification and packaging."""

import json
import os
import pathlib
import subprocess
import sys
import tempfile
import unittest


SCRIPTS = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
from verification.workspace import native_build_directory


class NativeBuildCacheTests(unittest.TestCase):
    def test_explicit_host_packaging_reuses_the_verified_native_artifact(self):
        toolchain = "1.97.0"
        with tempfile.TemporaryDirectory(prefix="metis-native-cache-") as temporary:
            root = pathlib.Path(temporary)
            (root / "src").mkdir()
            (root / "Cargo.toml").write_text(
                '[package]\nname = "metis-native-cache-probe"\n'
                'version = "0.1.0"\nedition = "2024"\n[workspace]\n',
                encoding="utf-8",
            )
            (root / "src/main.rs").write_text(
                'fn main() { println!("{}", [2_u32, 3, 5].iter().product::<u32>()); }\n',
                encoding="utf-8",
            )
            environment = os.environ.copy()
            # Own every cache/configuration input of this real compiler fixture.
            for name in ("CARGO_BUILD_TARGET", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS",
                         "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"):
                environment.pop(name, None)
            environment.update(CARGO_HOME=str(root / "cargo-home"),
                               CARGO_TARGET_DIR=str(root / "target"),
                               CARGO_NET_OFFLINE="true", CARGO_BUILD_JOBS="1")

            def run(arguments):
                return subprocess.run(
                    arguments, cwd=root, env=environment, check=True,
                    capture_output=True, text=True, timeout=20,
                ).stdout

            compiler = run(["rustup", "run", toolchain, "rustc", "-vV"])
            host = next(line.removeprefix("host: ") for line in compiler.splitlines()
                        if line.startswith("host: "))
            cargo = ["rustup", "run", toolchain, "cargo"]
            run([*cargo, "generate-lockfile", "--offline"])
            metadata = json.loads(run([*cargo, "metadata", "--format-version", "1",
                                       "--no-deps", "--locked", "--offline"]))
            directory = native_build_directory(
                metadata, host, environment, explicit_target=True
            )
            build = [*cargo, "build", "--release", "--locked", "--offline",
                     "--message-format=json-render-diagnostics"]

            def artifact(output):
                records = [json.loads(line) for line in output.splitlines() if line]
                binaries = [record for record in records
                            if record.get("reason") == "compiler-artifact"
                            and record.get("executable")]
                self.assertEqual(len(binaries), 1)
                return binaries[0]

            verified = artifact(run(build))
            packaged = artifact(run([*build, "--target", host]))
            self.assertTrue(packaged["fresh"],
                            "packaging recompiled the already verified host target")
            self.assertEqual(packaged["executable"], verified["executable"])
            executable = directory / "release" / (
                "metis-native-cache-probe.exe" if os.name == "nt"
                else "metis-native-cache-probe"
            )
            self.assertEqual(executable, pathlib.Path(verified["executable"]))
            self.assertEqual(run([str(executable)]).strip(), "30")

    def test_implicit_native_build_keeps_its_existing_directory(self):
        environment = {"CARGO_TARGET_DIR": "owned-target"}
        directory = native_build_directory(
            {"target_directory": "owned-target"}, "aarch64-apple-darwin",
            environment, explicit_target=False,
        )
        self.assertEqual(directory, pathlib.Path("owned-target"))
        self.assertEqual(environment, {"CARGO_TARGET_DIR": "owned-target"})


if __name__ == "__main__":
    unittest.main()
