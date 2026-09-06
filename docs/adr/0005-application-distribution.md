# ADR 0005: Application distribution

Status: Accepted

Date: 2026-09-06

Driver: [METIS-DISTRIBUTION-001](../../backlog.md#METIS-DISTRIBUTION-001).

## Decision

One versioned application manifest declares identity, Cargo binary targets,
entry executable and individually named resources. The Rust `metis` CLI builds
the declared targets under Cargo's locked resolution and consumes its actual
artifact messages. The same validated inventory supplies portable directories
and platform installers. No renderer owns build or installation policy; a
headless process application can exercise distribution before a GUI host exists.

Separate application identity/version from the framework version. Keep frontend
and backend executables distinct and preserve their sibling lookup contract.
Do not create a second packaging configuration or infer assets from recursive
directory scans. Runtime user data is outside the owned installation inventory.

The first installer is a Windows per-user MSI, authored through native Windows
Installer APIs and the installed cabinet tool. Windows Installer owns installation,
registration, rollback and removal; Métis authors its exact payload and standard
actions. No custom elevated installer or destructive directory removal is needed.
Moirai owns bounded process execution and child cleanup. Its absence of a process
working-directory parameter is handled with absolute tool/input paths.

The CLI uses `serde_json` for configuration and Cargo's JSON messages. No shared
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

## Verification

Tests cover manifest/path/version rejection, exact artifact selection and binary
inventory. The committed bounded workflow builds a real two-process application,
runs its portable bundle, creates an MSI, installs it into an isolated location,
compares installed bytes, runs input-sensitive IPC, and uninstalls. A user-created
sentinel survives uninstall. Inspect installer tables and actual output; producing
an `.msi` filename is not installation evidence. Signing/upgrade/platform support
claims require their own complete host workflows and cannot inherit this result.
