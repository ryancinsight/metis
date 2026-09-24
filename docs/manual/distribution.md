# Build executables and installable applications

Métis uses one application manifest for Cargo targets, launch arguments and
resources. The CLI emits a portable `app/` directory, an inventory of SHA-256
hashes and, with `package`, a package for the matching host. Application versions
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

`metis build` is host-native and stages a portable application on Windows,
macOS and Linux. The executable suffix and Cargo artifact path follow the host
target. `metis package` emits the Windows x64 MSI on Windows, a `.app` bundle
on macOS, or a USTAR `.tar` archive on Linux. The package command must run on
the target host; cross-compilation alone does not provide native installation
evidence.

The non-Windows package formats use the same staged inventory as the portable
output. A macOS package contains `<id>.app/Contents/MacOS`,
`Contents/Resources` and a generated `Contents/Info.plist`. A Linux package is
an uncompressed USTAR archive with binaries under `usr/bin`, resources under
`usr/share/<id>` and a generated desktop entry under
`usr/share/applications`. `inventory.json` records the package format, byte
count, SHA-256 digest and each archived or bundled file. The generated metadata
does not add signing, notarization, a package-manager registration or an update
channel.

### Register URL schemes

List custom URL schemes in `url_schemes` to have the installers open your
application for links such as `org.example.viewer://open/study?series=3`, as
Tauri's deep-link plugin does:

```json
"url_schemes": ["org.example.viewer"]
```

A scheme is a lowercase ASCII letter followed by lowercase letters, digits,
`+`, `-` or `.`, at most 32 bytes, and never a standard scheme such as
`https`, `file` or `mailto`. A reverse-domain name avoids collisions with
other applications. At most eight are accepted. Each installer registers them
for the entry binary:

- The Linux desktop entry adds `MimeType=x-scheme-handler/<scheme>;` and passes
  the link through `%u`.
- The macOS bundle adds a `CFBundleURLTypes` entry.
- The Windows MSI writes `HKCU\Software\Classes\<scheme>` with `URL Protocol`
  and an `open` command. These rows belong to the entry component, so
  uninstalling removes them.

The operating system starts the application with the link as an argument.
`metis_core::deep_link::DeepLink::from_arguments` finds that argument and
parses it against the same schemes. It returns percent-decoded path segments
and query pairs, and rejects `..` segments, control characters and undeclared
schemes. A second launch starts a second process; call
`metis_platform::claim_or_forward` first so that process hands its arguments
to the running instance and exits.

### Associate document types

List document types in `file_associations` so the installers offer your
application for them, as a Tauri bundle's `fileAssociations` does:

```json
"file_associations": [
  {
    "extensions": ["dcm", "dicom"],
    "mime_type": "application/dicom",
    "description": "DICOM image"
  }
]
```

Extensions are 1 to 16 lowercase letters or digits without the dot, and a
MIME type is a lowercase `type/subtype`. Both must be unique across the
manifest. The description is at most 64 characters, without control
characters or the `[]{}` installer formatting syntax. At most eight types,
each with at most eight extensions, are accepted.

- The Linux package lists the MIME types in the desktop entry, which receives
  the document through `%u`. It also installs
  `usr/share/mime/packages/<id>.xml`, which maps each extension to its type.
- The macOS bundle adds `CFBundleDocumentTypes` with the Viewer role.
- The Windows MSI writes a per-user ProgID `<id>.document<n>` with an `open`
  command and lists it under each extension's `OpenWithProgids`. The
  application then appears in "Open with" without replacing the user's default
  program. Uninstalling removes these rows.

The application receives the document path as an argument.

Here `metis` denotes the built executable in the configured Cargo target directory;
put that directory on PATH or use its absolute path. Create `output` first.
Each destination must be new. A failed build leaves its partial directory for
inspection and does not erase existing output. A successful operation writes
`inventory.json` last. Do not reuse an output directory as application input.

The CLI invokes Cargo in release mode with `--locked`, using the caller's Cargo
configuration. Portable builds use the host target; Windows x64 MSI builds
select `x86_64-pc-windows-msvc` explicitly. In Atlas, local provider overlays can differ from the standalone
lock; `python scripts/verify.py` uses the standalone resolution while retaining
the shared build cache. Missing or ambiguous compiler artifacts are errors.
Building runs the selected project's build scripts with your developer account.
The package Cargo build has a finite 900-second deadline for cold workspaces;
the committed Windows workflow keeps the remaining time for MSI authoring and
inventory verification. A deadline terminates the owned compiler tree and never
selects a stale executable.

The bound was exercised by the RITK consumer package workflow in hosted run
[34990164849](https://github.com/ryancinsight/ritk/actions/runs/34990164849),
which completed the locked Windows build, package creation, inventory/hash
checks and executable `--help` within its 30-minute job budget. The distribution
demo harness retains its separate 300-second command bound; this does not
shorten the packager's cold Cargo deadline in the consumer workflow.

## Start a project and reload it

Create a new application in a directory whose parent already exists:

```powershell
metis init .\sample-app
metis dev .\sample-app\metis.json --once
```

`init` refuses an existing directory and writes `Cargo.toml`, a complete
dependency-free `Cargo.lock`, `app/Cargo.toml`, `app/src/main.rs` and
`metis.json`. The generated entry is intentionally format-neutral: it prints
one supplied argument and has no DICOM, image or clinical-domain behavior.
Replace that entry with the application you own, then keep the manifest's
explicit binary and resource inventory synchronized with it.

Start a live development session on any host:

```powershell
metis dev .\sample-app\metis.json --watch
```

On Windows the watcher observes the manifest directory through a bounded
native change-notification handle. Other hosts poll a metadata fingerprint —
relative path, size and modification time — of the same bounded file tree
every 250 ms. Either signal only prompts a check: the loop then hashes regular
source/resource bytes and rebuilds only when those bytes changed, so a coarse
filesystem clock delays a reload rather than missing or duplicating one. Cargo is run with `--locked` for every
generation. A source or resource change terminates the contained child and
starts a new Cargo run; changes under `.git`, `target`, `output`, `dist` and
`node_modules` are excluded from the fingerprint so compiler output cannot
trigger a reload loop. If the manifest or source does not parse, or Cargo
returns a failure status, the diagnostic is shown and no previously built
executable is launched. Fix the input and save it again to retry. `--once` has
a 300-second process deadline. The Windows child runs in a contained job;
other hosts run it uncontained until Moirai provides their containment. Watch
mode prints a
readiness line before the first build, an idle line after each generation and a
reload line for every accepted source or resource change.

Generate completions from the same command descriptions used by `--help`:

```powershell
metis completions powershell | Out-File -Encoding utf8 metis-completion.ps1
metis completions bash > metis-completion.bash
metis completions fish > metis-completion.fish
metis completions zsh > _metis
```

The generated scripts list `init`, `dev`, `build`, `package` and `completions`
plus the `dev` lifecycle flags. They contain no credentials and do not change
the manifest grammar.

The installed Windows cabinet tool and MSI API supply packaging; no WiX/NSIS
installation is needed. Cabinet staging paths must currently be ASCII. Legacy installer-tool paths
must fit 259 UTF-16 characters, including generated staging components; known
canonical drive/UNC prefixes are converted without changing their target. Metadata
uses code page 1252 and rejects text that cannot be represented losslessly. Shortcut arguments must fit 255 UTF-16 characters after
Windows command-line quoting. Payloads are limited to 4096 files and 1 GiB; the manifest is
limited to 1 MiB. Linked inputs, traversal and destination collisions reject.
Keep source files and output ancestors stable while the command runs.

## Build and serve a browser application

A manifest with a `frontend` member describes a page served to a browser rather
than a native executable, like a Tauri project's frontend with its Rust code
compiled to WebAssembly. The [starter manifest](../../crates/metis-starter/metis.json)
is the reference:

```json
{
  "schema": 1,
  "name": "Metis Starter",
  "cargo_manifest": "Cargo.toml",
  "frontend": { "package": "metis-starter", "directory": "frontend" }
}
```

`frontend.package` is a workspace package with a `cdylib` target and
`frontend.directory` holds the static page, which must include `index.html`.
Native fields such as `binaries` or `entry` are rejected in this shape, and a
native manifest rejects `frontend`. In the manifest's directory:

```powershell
metis build
metis serve
```

Either command reads `./metis.json` unless a manifest path is given. `build`
compiles the package with `--release --locked --target wasm32-unknown-unknown`,
runs the `wasm-bindgen` CLI with `--target web --no-typescript`, copies the page
beside the generated loader, and writes `dist/app` and `dist/inventory.json`
beside the manifest; `metis build MANIFEST OUTPUT` chooses another output. A
page file named like a generated file is an error. The CLI's version must equal
the `wasm-bindgen` crate the package's locked dependency graph resolves, because
the loader and the crate's embedded schema change together: `WASM_BINDGEN`
names the executable, otherwise it is found on `PATH`, and a mismatch reports
the install command. Rebuilding replaces an output only when its inventory
records a previous browser build; any other existing directory is refused.

`serve` builds, reads the staged page into memory and serves it on
`http://127.0.0.1:1420/`, the port a Tauri development server uses, until
interrupted. `--port PORT` selects another port; `0` asks the system for one,
and the address line reports it. Requests are `GET` only; a path outside the
build is 404 and a malformed target 400. Responses carry `Cache-Control:
no-cache`, so reloading after a rebuild fetches the new files, and
`X-Content-Type-Options: nosniff`, with `application/wasm` for modules. The
server accepts at most 16 connections at a time, each with a 10-second request
deadline. Serving does not watch sources; run `metis serve` again after a change.

On Windows, `metis dev` opens the page in a native window instead of a browser,
as `tauri dev` does:

```powershell
metis dev --watch
```

`dev` builds as `build` does, then hosts `dist/app` in a WebView2 window of
create-tauri-app's 800 × 600 size, titled with the manifest `name`. WebView2
serves the folder as `https://app.metis.example/` (a name under the `.example`
domain reserved by RFC 2606, so it cannot shadow a real site), so the page
loads as a secure origin: its modules, WebAssembly and `default-src 'self'`
policy behave as they do under `metis serve`, which a `file:///` page does not
allow. Navigation is confined to that host. With `--watch`, a change under the
manifest directory, `dist/` excluded, rebuilds the page and reloads the window;
a failed compile is reported and the previous page stays. Closing the window
ends the command. Without `--watch` the window runs until closed.

## Run the portable application

```powershell
output/portable/app/metis-app.exe 60 2 0.2
output/portable/app/metis-app.exe 80 2 0.2
```

On macOS or Linux, the same host-native bundle uses
`output/portable/app/metis-app` without the Windows executable suffix.

The application starts another instance of its own executable for presentation
and exchanges real IPC messages. These synthetic example inputs produce
respectively 0.36 and 0.48 mL/hour. The executable can be copied or renamed
without a frontend companion; explicitly declared application resources still
travel with applications that use them. The manifest launch arguments supply
the first input set to the Start Menu shortcut. This demonstration is a console program and exits
after its calculation; an application with a persistent GUI supplies that
behavior in its own entry executable. Packaging does not create a GUI host.

## Install and remove

### Windows MSI

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

### macOS application bundle

Run `metis package metis.json output/package` on macOS, then copy the generated
`<id>.app` to a user-owned Applications directory and launch its
`Contents/MacOS/<entry>` executable. Remove the copied bundle to uninstall it.
The current package writer does not notarize, sign or register a launch service;
actual macOS host installation, launch, permission and removal captures remain
open under `METIS-DISTRIBUTION-003`.

### Linux USTAR archive

Build the archive and install it below an existing absolute prefix:

```shell
metis package metis.json output/package
metis install output/package/<id>.tar "$HOME/.local"
```

The installer validates the generated USTAR archive, maps its `usr/` tree below
the prefix (`$HOME/.local/bin`, `$HOME/.local/share/<id>` and
`$HOME/.local/share/applications`), and rewrites the desktop entry's `Exec`
and `Icon` fields to those absolute paths. It writes a schema-versioned
ownership record containing each installed path, byte count and SHA-256 digest.
Existing files, linked paths, traversal entries and conflicting destinations are
rejected without partial installation.

Remove an installation with:

```shell
metis uninstall <id> "$HOME/.local"
```

Removal verifies every recorded digest and refuses to delete a package file that
the user changed or replaced. It removes only unchanged package files and
empty directories, preserving unrelated files in the prefix. Native X11/Wayland
installation, launch, permission and removal captures remain open under
`METIS-DISTRIBUTION-003`.

## Inspect the result

`inventory.json` contains application configuration, executable/resource paths,
byte counts, SHA-256 values and the host package record when `package` is used.
The Windows record includes ProductCode/hash; macOS/Linux records include the
bundle/archive file, byte count, digest and per-file paths. These identify the
bytes tested; hashes do not authenticate a publisher. Product and package
GUIDs are fresh while component GUIDs are stable per application resource, so
repeated package builds do not produce identical MSI bytes.

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
commands, calculated values and install/uninstall outcomes. Its
`artifact_sizes.portable_payload` object records the verified payload file count,
total bytes, application executable bytes and declared-resource bytes;
`artifact_sizes.installer` records the MSI byte count when an installer was built.
These are artifact-size measurements for this package and do not compare
frameworks or imply a runtime memory result. With `--install`,
the shortcut probe also records `IconLocation` and requires the Windows
Installer cache reference to end in `MetisIcon,0`; this is the shell-visible
proof that the MSI `Icon` row is used. The gate preserves only the latest
marked test run and refuses to replace a still-registered test installation.
Local verification without `--install` exercises packaging and portable
execution. The Windows gate invokes `python scripts/verify.py --install`, so
hosted acceptance covers the current-user installation, input-sensitive runs,
uninstall cleanup and user-file preservation as well.

The verifier may use a temporary `SUBST` drive to isolate Cargo from the Atlas
development overlay. The Windows Installer service cannot see that per-user
mapping, so the install probe passes the physical MSI source and physical test
directory to `msiexec`; this keeps source resolution and transactional cleanup
on paths visible to the service. The MSI stores its application location under
the explicit current-user registry root and the maintenance probe reads that
same location before repair or removal. Each payload file is the key path for
its MSI component; registry values remain component-owned, and deterministic
component identities survive package rebuilds.

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
approval and trust boundary; it is not a credential store. The `crates-io` and
`pypi` environments now exist without protection rules, secrets or variables.
Add reviewer/tag protection and registry trusted publishers as release-owner
actions.

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

Windows x64 MSI remains the only package with a committed native install/run/
uninstall workflow. The Linux archive now has a committed prefix installer and
hash-checked uninstall implementation with user-file preservation tests; native
Linux launch, permission and removal captures are still open. The macOS bundle
emitter, other Windows architectures, signing, authenticated updates and browser
deployment remain separate target work. This command does not sign, publish or
release an application.
