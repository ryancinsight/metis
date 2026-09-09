# Build executables and installable applications

Métis uses one application manifest for Cargo targets, launch arguments and
resources. The CLI emits a portable `app/` directory, an inventory of SHA-256
hashes and, with `package`, a Windows Installer package. Application versions
are independent of the Métis framework version.

## Configure and build

The repository [metis.json](../../metis.json) is a runnable configuration for the
single-executable, two-process console demonstration. Change the application identity, display name,
manufacturer and version for your own application. Generate one uppercase braced
upgrade GUID for that identity and retain it. Declare every binary and resource;
the demonstration declares only `metis-app`. Additional declared binaries are
optional sidecars, not mandatory frontend/backend components. There is no
recursive asset scan. Paths are relative to the manifest directory.

On Windows x64, use the pinned Rust toolchain and an ordinary Cargo workspace:

```powershell
cargo build -p metis-cli --release --locked
metis --help
metis build metis.json output/portable
metis package metis.json output/installer
```

Here `metis` denotes the built executable in the configured Cargo target directory;
put that directory on PATH or use its absolute path. Create `output` first.
Each destination must be new. A failed build leaves its partial directory for
inspection and does not erase existing output. A successful operation writes
`inventory.json` last. Do not reuse an output directory as application input.

The CLI invokes Cargo in release mode with `--locked`, using the caller's Cargo
configuration and an explicit Windows x64 target. In Atlas, local provider overlays can differ from the standalone
lock; `python scripts/verify.py` uses the standalone resolution while retaining
the shared build cache. Missing or ambiguous compiler artifacts are errors.
Building runs the selected project's build scripts with your developer account.

The installed Windows cabinet tool and MSI API supply packaging; no WiX/NSIS
installation is needed. Cabinet staging paths must currently be ASCII. Legacy installer-tool paths
must fit 259 UTF-16 characters, including generated staging components; known
canonical drive/UNC prefixes are converted without changing their target. Metadata
uses code page 1252 and rejects text that cannot be represented losslessly. Shortcut arguments must fit 255 UTF-16 characters after
Windows command-line quoting. Payloads are limited to 4096 files and 1 GiB; the manifest is
limited to 1 MiB. Linked inputs, traversal and destination collisions reject.
Keep source files and output ancestors stable while the command runs.

## Run the portable application

```powershell
output/portable/app/metis-app.exe 60 2 0.2
output/portable/app/metis-app.exe 80 2 0.2
```

The application starts another instance of its own executable for presentation
and exchanges real IPC messages. These synthetic example inputs produce
respectively 0.36 and 0.48 mL/hour. The executable can be copied or renamed
without a frontend companion; explicitly declared application resources still
travel with applications that use them. The manifest launch arguments supply
the first input set to the Start Menu shortcut. This demonstration is a console program and exits
after its calculation; an application with a persistent GUI supplies that
behavior in its own entry executable. Packaging does not create a GUI host.

## Install and remove

Open the generated `.msi` using Windows Installer. It installs for the current
user under Local AppData, registers an uninstall entry and creates a Start Menu
shortcut. It does not request a machine-wide installation. The manifest names
all files the installer owns; keep changing application data outside those files.

For a quiet local test, pass the generated package path to `msiexec /i` with
`/qn /norestart`. Remove it through Installed Apps or `msiexec /x PRODUCT_CODE
/qn /norestart`, using the ProductCode in `inventory.json`. Uninstall removes
owned files and registration while preserving unrelated files. The installer
persists its installation directory in its current-user registry key and restores
it for repair/removal; a missing location rejects maintenance before deleting
files. Uninstall does not need an `INSTALLDIR` parameter. The bounded
verification workflow also tests a private install directory and retained user
file; it does not require installing into the application's default location.

Re-running the same MSI supports Windows Installer maintenance. A different
package with the same upgrade GUID rejects while the existing product is
installed, even at the same version: uninstall first. Automatic updates,
migration and rollback across releases are not implemented.

## Inspect the result

`inventory.json` contains application configuration, executable/resource paths,
byte counts, SHA-256 values and the installer ProductCode/hash. These identify
the bytes tested; hashes do not authenticate a publisher. MSI GUIDs are fresh,
so repeated package builds do not produce identical MSI bytes.

Run the complete local installation demonstration with:

```powershell
python scripts/verify.py --install
```

The Windows x64 verification workflow checks both the portable and installed
forms against these independent expected values:

| Input (weight, concentration, dose) | Flow | Drug rate |
| --- | --- | --- |
| `60 2 0.2` | 0.36 mL/hour | 0.72 mg/hour |
| `80 4 0.5` | 0.60 mL/hour | 2.40 mg/hour |

The single-executable acceptance check requires exactly one application
executable in the portable and installed payloads, while the calculation still
reports distinct backend/frontend process identifiers. It also checks the
shortcut's executable, arguments and working directory,
removes the installed application and retains its user-created test file.

The manifest's explicit SVG, PNG and ICO resources are copied into the portable
and MSI payloads, so browser and native branding remain available after
installation. Set `icon` to the ICO source when packaging an application. Métis
validates the SVG and ICO before staging them; the SVG validator admits one
fixed viewport with literal path geometry and colors, while the ICO validator
checks its bounded PNG entries. The Start Menu shortcut's `Icon_` field
references the embedded `MetisIcon` row. The starter ICO contains seven PNG
resolutions from 16×16 through 256×256 and is generated from the local project
mark; replace the SVG, PNG and ICO with project-owned artwork that meets the
same bounded format contracts.

The report at `output/distribution/latest/workflow.json` records exact inventory,
commands, calculated values and install/uninstall outcomes. With `--install`,
the shortcut probe also records `IconLocation` and requires the Windows
Installer cache reference to end in `MetisIcon,0`; this is the shell-visible
proof that the MSI `Icon` row is used. The gate preserves only the latest
marked test run and refuses to replace a still-registered test installation.
Normal verification exercises packaging and portable execution;
`--install` opts into the current-user OS installation workflow.

## Publish crates through CI

`.github/workflows/rust-release.yml` is a thin caller of Atlas's pinned
`crates-publish.yml` and `semver-gate.yml` workflows. A GitHub Release tagged
`crate-<package>-v<version>` runs the release gate and publishes one validated
workspace package. `workflow_dispatch` calls a separate validation-only job;
that path has no OIDC permission and cannot publish.

The publish job requests a short-lived crates.io token through GitHub Actions
OIDC and the `crates-io` environment. The repository stores no Cargo token,
private signing key or other registry credential. Each crate must have its
trusted publisher registered at crates.io with owner `ryancinsight`, repository
`metis`, workflow `.github/workflows/rust-release.yml` and environment
`crates-io`; registry setup and the first publication remain explicit release
authority actions.

No private-key prompt is part of this release path. If a developer's local Git
installation asks for a signing key, cancel it and inspect that local Git
configuration; Metis publication does not invoke local signing. The GitHub
jobs exchange their OIDC identity for a short-lived registry credential.

The `metis-python` crate builds the `metis-rs` PyPI distribution for
`import metis`. Its caller uses Atlas's `python-wheels.yml` and the `pypi`
environment with OIDC; a long-lived PyPI token or developer private key is not
added to the repository. See the [Python binding manual](python.md) for the
local wheel test and release tag contract.

### Configure trusted publishers without keys

Create the GitHub environments named `crates-io` and `pypi` and apply the
repository's normal tag or reviewer protection rules. Leave registry secrets,
passwords and signing keys out of these environments. The environment is an
approval and trust boundary; it is not a credential store. The current public
repository audit has no Actions secrets, variables or environments yet, so this
setup remains a release-owner action.

For every publishable Cargo package, add a crates.io GitHub Actions trusted
publisher with these exact values:

```text
Owner: ryancinsight
Repository: metis
Workflow: .github/workflows/rust-release.yml
Environment: crates-io
```

The first crates.io publication still requires the registry's normal initial
release step. After that, the publisher entry authorizes the reusable Atlas job
to exchange its GitHub OIDC identity for a short-lived upload token. See the
[Rust Forge trusted-publishing guide](https://forge.rust-lang.org/infra/docs/trusted-publishing.html).

For the `metis-rs` project on PyPI, add a GitHub Actions trusted publisher (or
a pending publisher before the project is created) with:

```text
Owner: ryancinsight
Repository: metis
Workflow: .github/workflows/python-release.yml
Environment: pypi
```

PyPI exchanges the job's OIDC identity for a short-lived upload authorization;
the workflow does not read `PYPI_TOKEN`, `TWINE_PASSWORD`, SSH keys or GPG
material. See [PyPI's publisher setup](https://docs.pypi.org/trusted-publishers/adding-a-publisher/).

The [application gallery](applications.md) shows the existing renderer workflows.
The [verification contract](../VERIFICATION.md#V10) distinguishes installation,
rendering and host interaction evidence. Executable and installer creation does
not establish lower memory usage or stronger OS isolation than Tauri.

The first package backend is Windows x64 MSI. macOS bundles/DMG, Linux packages,
other Windows architectures, signing, authenticated updates and browser deployment
remain separate target work. This command does not sign, publish or release an
application.
