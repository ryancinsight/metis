# 0063 — Release scope for 1.0

Status: Accepted

Date: 2026-11-15

## Context

Metis is the stack's restricted desktop and browser host (native shells,
WebView pages, input/text/accessibility, assets, scoped files, desktop
integration, distribution). The board carries thirty-two open items across
platform hosts, input, content, system services, release readiness, and
board hygiene. No milestone ladder orders them toward a stable 1.0, and no
record states what 1.0 freezes.

## Decision

Adopt a seven-rung ladder to 1.0. The ladder opens `backlog.md`; every open
item carries exactly one `milestone:` assignment.

- **M-platform**: native restricted desktops on Windows, macOS, and Linux,
  the mobile capability matrix, and the Rust-owned WebView page lifecycle
  (METIS-DESKTOP-001, METIS-MACOS-001, METIS-LINUX-001, METIS-MOBILE-001,
  METIS-WEBVIEW-RUST-001). Exit: each host runs the restricted desktop
  behind its OS boundary with no behavior loss.
- **M-input**: controls, text, accessibility, and view machinery
  (METIS-INPUT-001, METIS-TEXT-001, METIS-A11Y-001, METIS-I18N-001,
  METIS-LIST-002, METIS-LIST-003, METIS-ANIMATION-001,
  METIS-COMPONENT-001). Exit: shared UI state, IME, accessible controls,
  localized strings, virtualized lists, and bounded transitions hold in
  both hosts.
- **M-content**: reads, assets, graphics, files, and presentation
  (METIS-BROWSER-READ-001, METIS-ASSETS-001, METIS-GRAPHICS-001,
  METIS-FILES-001, METIS-WORKBENCH-DESIGN-001, METIS-MEM-WEB-001). Exit:
  selected-file reads diagnose cleanly, assets are hostile-content safe,
  custom graphics conform, and the browser writes only changed DOM state.
- **M-system**: integration, services, migration, and distribution
  (METIS-INTEGRATION-001, METIS-SERVICES-001, METIS-MIGRATION-001,
  METIS-DISTRIBUTION-003, METIS-DISTRIBUTION-004). Exit: desktop services
  are scoped, network/shell/sidecars sandboxed, egui/Tauri applications
  migrated without behavior loss, and installers plus signed updates ship
  per platform.
- **M-release**: provider coherence and publication readiness
  (METIS-CURRENT-PROVIDERS-001, METIS-PYPI-FIRST-RELEASE-001,
  METIS-RELEASE-001, METIS-PERF-001, METIS-CONFORMANCE-001). Exit: the
  provider graph is coherent at head, the genuine Python first release is
  complete, comparative resource/latency evidence is committed, and
  capability/target closure holds.
- **M-board**: record hygiene (METIS-DOCS-ADR-001, METIS-BOARD-001,
  METIS-RESIDUAL-001, METIS-INTEG-DEBT-001). Exit: ADRs within budget, a
  closed priority set with an item-block budget, every cited residual
  owned, and no unlanded peer commits or stale tree dirt.
- **M-1.0**: stable host release (**authority-gated**). Exit: `cargo
  semver-checks`, publish dry-run, `cargo doc`, the full native/browser
  verification matrix green, and zero open correctness items.
  **Veto point:** release authority sets the API-freeze contents; this
  milestone never forces `publish = true` without it.
- **M-ecosystem**: consumer-owned embedding (ritk viewer panes, helios
  clinical validation). Exit: consumer receipts on their own boards; does
  not gate 1.0.

### Veto points

1. The 1.0 API-freeze contents (release authority sets them).
2. Trusted-publisher registration and release authority are external
  (METIS-RELEASE-001, METIS-DISTRIBUTION-004); they stay recorded holds,
  not 1.0 blockers this repo can clear alone.

## Rejected alternatives

- Freezing 1.0 on the current provider checkpoint alone: the checkpoint
  is historical evidence, not head acceptance; M-release requires fresh
  exact-head verification.
- Treating mobile as a 1.0 gate: the matrix is capability-scoped per
  platform; only the matrix decision gates, not full mobile parity.

## Overturning evidence

A consumer failure traceable to a frozen API, or a hostile-content escape
through the asset boundary, reopens the affected rung.
