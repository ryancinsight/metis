# ADR 0005: Application distribution

Status: Accepted

Date: 2026-09-06

Driver: [METIS-DISTRIBUTION-001](../../backlog.md#METIS-DISTRIBUTION-001).

Revision 2026-09-06: [METIS-APPLICATION-001](../../backlog.md#METIS-APPLICATION-001)
replaces the demonstration's mandatory executable pair with one application
entry. [ADR 0006](0006-application-entry.md) defines process-role dispatch and the
command migration. The manifest remains the single payload inventory.

Revision 2026-09-08: [METIS-RELEASE-001](../../backlog.md#METIS-RELEASE-001)
adds the Atlas-pinned crates.io release caller. Registry authentication remains
tokenless through GitHub Actions OIDC. [METIS-PYTHON-001](../../backlog.md#METIS-PYTHON-001)
adds the `metis-rs` PyPI caller; its publish job uses the same OIDC model and
does not carry a registry token or developer key.

Revision 2026-09-09: [METIS-DISTRIBUTION-002](../../backlog.md#METIS-DISTRIBUTION-002)
adds `init`, `dev` and `completions` to the same manifest-driven tool. `init`
emits a complete locked Cargo workspace, `dev --once` is a bounded run on every
host, and Windows `dev --watch` uses native directory notifications plus a
content fingerprint. A failed generation never selects an earlier executable;
the watcher excludes generated output from its fingerprint. Shell help and
completions are generated from one command table, so a second command grammar
cannot drift from the CLI. The generated entry is format-neutral: DICOM parsing,
decoding, geometry and viewer state remain RITK responsibilities, while Métis
provides the shell and presentation handoff.

Revision 2026-09-15: [METIS-DISTRIBUTION-005](../../backlog.md#METIS-DISTRIBUTION-005)
extends the package Cargo build deadline to 900 seconds. Hosted run
[34984471177](https://github.com/ryancinsight/ritk/actions/runs/34984471177)
reached the former 300-second limit while compiling the cold locked RITK graph,
before an executable artifact existed. The longer finite bound fits the
workflow's 30-minute job budget while preserving process-tree termination and
stale-artifact rejection. The local standalone gate at commit
`103e3e659da26af5036abae50c854c2c08401189` and Metis hosted Windows gate
[34987447904](https://github.com/ryancinsight/metis/actions/runs/34987447904)
passed the updated implementation.

Revision 2026-09-15 (consumer verification): RITK hosted run
[34990164849](https://github.com/ryancinsight/ritk/actions/runs/34990164849)
completed the locked Windows package build at Metis
`c4276f2586f1ae9a1e3c0fa1dcb1507be4555f24`. Package creation, inventory/hash
checks and executable `--help` passed within the 30-minute job budget. The
workflow artifact is package evidence; DICOM execution remains consumer-owned,
while the Metis Windows gate owns the MSI lifecycle acceptance.

Revision 2026-09-20: [METIS-DISTRIBUTION-003](../../backlog.md#METIS-DISTRIBUTION-003)
keeps portable staging host-native on Windows, macOS and Linux. The CLI selects
the host Cargo artifact for `build`, while the Windows x64 target remains an
explicit prerequisite for MSI authoring. This closes the cross-host executable
path without claiming a macOS bundle or Linux installer; those formats still
require their native packaging and install workflows.

Revision 2026-09-20 (hosted lifecycle): the Windows gate invokes the existing
bounded `scripts/verify.py --install` workflow. The hosted oracle now installs
the generated per-user MSI into an isolated directory, runs both input cases,
checks the Start Menu shortcut and HKCU registration, uninstalls the exact
ProductCode and verifies that only the user-created sentinel remains. This
makes hosted Windows lifecycle evidence part of the committed gate without
changing the MSI's per-user authority boundary or claiming macOS/Linux
installer support; the first green run binds the evidence to a revision.

Revision 2026-09-20 (service-visible paths): neutral verification may expose
the repository through a per-user `SUBST` drive so its Cargo resolution is
independent of the Atlas overlay. Windows Installer runs in its service
process and cannot inherit that mapping, so the verifier resolves the MSI
source and its test `INSTALLDIR` to the physical repository path before
launching `msiexec`. Registry rows use the MSI context-dependent root (`-1`)
for the package's per-user context; the package-table test and the hosted
lifecycle gate remain the acceptance oracles.

## Decision

One versioned application manifest declares identity, Cargo binary targets,
entry executable, launch arguments and individually named resources. The Rust `metis` CLI builds
the declared targets under Cargo's locked resolution and consumes its actual
artifact messages. The same validated inventory supplies portable directories
and platform installers. No renderer owns build or installation policy; a
headless process application can exercise distribution before a GUI host exists.

Separate application identity/version from the framework version. The
demonstration ships one application executable, relaunching itself for the
presentation role. Additional executables are optional manifest-declared sidecars;
the framework does not require a frontend/backend executable pair.
Do not create a second packaging configuration or infer assets from recursive
directory scans. Runtime user data is outside the owned installation inventory.

The first installer is a Windows per-user MSI, authored through native Windows
Installer APIs and the installed cabinet tool. Windows Installer owns installation,
registration, rollback and removal; Métis authors its exact payload and standard
actions. No custom elevated installer or destructive directory removal is needed.
Moirai owns bounded process execution and child cleanup. Its absence of a process
working-directory parameter is handled with absolute tool/input paths.

The CLI uses Serde with unknown/duplicate field rejection and `serde_json` for configuration and Cargo's JSON messages. No shared
Atlas general-purpose JSON parser exists in the inspected provider map or Consus
core. Reusing this maintained Rust parser avoids inventing a manifest grammar or
duplicating JSON parsing. This tooling-only dependency is an explicit exception
to the previous all-Atlas direct-dependency rule; runtime crates keep that rule.

## Scope and alternatives

Tauri's [Windows distribution](https://v2.tauri.app/distribute/windows-installer/)
uses MSI or NSIS setup executables. MSI provides the first Métis equivalent;
NSIS/WiX are not installed on the development host. The native MSI boundary is
isolated, with RAII handles and documented unsafe obligations. Unnecessary
framework dependencies and arbitrary installer scripts are rejected.

[Per-user installation](https://learn.microsoft.com/en-us/windows/win32/msi/installation-context)
avoids requesting machine-wide privileges. MSIX remains a separate format:
certificate trust and signing are not silently added to make a package install.
macOS bundles/DMG, Linux packages, signed distribution and authenticated update
recovery remain explicit target increments. Local package creation and testing
do not authorize a release, certificate-store modification or publication.

## Trust boundaries and failure behavior

Validate bounded manifest size, schema, unknown fields, identity, version and
file destinations before building. Reject traversal, absolute destinations,
reserved Windows names, alternate streams, case-insensitive collisions and
linked resource paths. Copy only the explicit inventory into a newly created
output directory; an existing output is an error, not permission to erase it.
Compilation and packaging errors cannot select stale artifacts as a success.

Cargo build scripts execute developer-selected code with developer authority;
this command is not an untrusted-code sandbox. Installer generation validates
native database strings and payload sizes. Installation owns only its declared
files and registration, and uninstall must preserve unrelated/user-created files.
Package hashes establish integrity evidence, not publisher authentication.
Developer input files and output ancestors must not change concurrently during
a build: path checks reject existing links but do not provide an OS sandbox
against a process racing filesystem mutations. MSI metadata uses code page 1252;
unrepresentable text rejects before native insertion, because an actual MSI
round-trip showed silent best-fit corruption for CJK, emoji and infinity. Full Unicode localization and non-ASCII cabinet staging paths require further
work. The native boundary converts only equivalent canonical drive/UNC paths
to legacy paths and rejects opaque namespaces, normalization-sensitive names and
paths beyond 259 UTF-16 units. Product/package/component GUIDs are fresh, so MSI bytes are not
reproducible; the payload inventory records exact source hashes.

Related packages with a different ProductCode reject, including a rebuild at
the same version. Uninstall the existing package first. Repair of the installed
ProductCode uses standard MSI maintenance. This is not an updater. Standard AppSearch/RegLocator restores an owned HKCU
InstallLocation before maintenance costing; missing location fails before file
removal. A real custom-directory uninstall exposed why the location must be
persisted instead of assuming the default directory.

## Verification

Tests cover manifest/path/version rejection, exact artifact selection and binary
inventory. The committed bounded workflow builds a real two-process application,
runs its portable bundle, creates an MSI, installs it into an isolated location,
compares installed bytes, runs input-sensitive IPC, and uninstalls. A user-created
sentinel survives uninstall. Inspect installer tables and actual output; producing
an `.msi` filename is not installation evidence. Signing/upgrade/platform support
claims require their own complete host workflows and cannot inherit this result.
