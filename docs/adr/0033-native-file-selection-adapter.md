# ADR 0033: Native file-selection adapter

- Status: Accepted
- Date: 2026-09-14
- Item: [METIS-FILES-001](../backlog.md#METIS-FILES-001)
- Upstream decision: [Moirai ADR 0058](../../moirai/docs/adr/0058-bounded-native-file-selection.md)

## Context

The native Metis host needs one platform seam for user-selected files and
folders. The browser host already exposes bounded DOM file handles, while
native applications must not import an application-specific dialog dependency
for every consumer. RITK owns DICOM interpretation and remains responsible for
root confinement and parser budgets after selection.

## Decision

`metis-platform::native::pick` re-exports Moirai's Windows common-dialog seam
and its explicit `DialogSelection` mode. Metis owns the host-facing adapter and
does not read, parse or authorize the returned path. Windows COM and task-memory
lifetime remain in Moirai; browser builds continue to use the bounded DOM file
provider.

## Verification

The adapter has no independent unsafe or parsing surface. Its compile contract
is checked through the native Metis platform build and the Moirai PAL tests;
the RITK consumer must exercise a real saved-study selection before this item
closes.
