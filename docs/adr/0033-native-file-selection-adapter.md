# ADR 0033: Native file-selection adapter

- Status: Accepted
- Date: 2026-09-14
- Revision: 2026-09-15 — RITK's native study-reopen session now consumes the
  adapter for an actual Windows folder selection. RITK scans and decodes the
  selected study; cancellation and decode failure preserve the prior frame.
- Revision: 2026-11-09 — re-export Moirai's `DialogSelection::Save` request
  types (`SaveRequest`, `FileFilter`, `MAX_DIALOG_FILTERS`), so a native
  application chooses an export destination through the same seam. Moirai PR
  [#671](https://github.com/ryancinsight/Moirai/pull/671) added the save mode.
  The re-exported `DialogSelection` is no longer `Copy` and is
  `#[non_exhaustive]`: callers clone a selection they reuse, and a match on
  it needs a wildcard arm.
- Item: [METIS-FILES-001](../backlog.md#METIS-FILES-001)
- Upstream decision: [Moirai ADR 0058](../../moirai/docs/adr/0058-bounded-native-file-selection.md)

## Context

The native Metis host needs one platform seam for user-selected files and
folders. The browser host already exposes bounded DOM file handles, while
native applications must not import an application-specific dialog dependency
for every consumer. RITK owns DICOM interpretation and remains responsible for
root confinement and parser budgets after selection.

## Decision

`metis-platform::native::pick` re-exports Moirai's Windows common-dialog
seam and its explicit `DialogSelection` mode, including the save mode's
bounded `SaveRequest` and `FileFilter` values. Metis owns the host-facing
adapter and does not read, write, parse or authorize the returned path.
Windows COM and task-memory lifetime remain in Moirai; browser builds continue
to use the bounded DOM file provider.

## Verification

The adapter has no independent unsafe or parsing surface. Its compile contract
is checked through the native Metis platform build and the Moirai PAL tests.
Unit tests in `native/dialog.rs` build save requests through the Metis path
and assert their name, their filters, acceptance at `MAX_DIALOG_FILTERS` and
rejection one filter past it.
RITK PR [#392](https://github.com/ryancinsight/ritk/pull/392), merged at
`3f46a08bf`, exercises `pick(DialogSelection::Folder)` from the native Métis
session; the locked `ritk-snap` suite passes 833/833 with strict native and
WASM checks. The actual MRI window and cancellation/decode behavior are
recorded in the [RITK DICOM manual](https://github.com/ryancinsight/ritk/blob/main/docs/manual/dicom-workflow.md).
Persistent stores, native permission policy and non-Windows providers remain
open under [METIS-FILES-001](../backlog.md#METIS-FILES-001).
