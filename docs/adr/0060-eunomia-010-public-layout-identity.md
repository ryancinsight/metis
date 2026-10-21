# 0060 — Eunomia 0.10 public layout trait identity

Status: Accepted

Date: 2026-10-15

Change class: [major], pre-1.0 minor increment

Driver: METIS-CURRENT-PROVIDERS-001

## Context

The isolated current-provider preparation changes Eunomia from 0.9.2 to 0.10.
`metis_platform::framebuffer::Color` derives `eunomia::Pod` and
`eunomia::Zeroable`. The platform root exports Color, and
`metis_ui_lang::style::Color` re-exports the same canonical type.

Both published Eunomia versions define these traits in their own
`src/layout/marker.rs`; they are not aliases of a stable third-party trait.
Matching trait source text does not preserve identity across incompatible
package versions. A caller using Eunomia 0.9's `Pod` bound cannot treat the
replacement 0.10 trait implementation as that implementation. The proposed
Metis 0.4.2 patch therefore understates this public contract change.

The cached published archives record 0.9.2 source `06848422be64b878a5c205a976e4643d2a3e48a5`
and 0.10.0 source `a231e54dd3311bc916e78c91d994b00d651c1646`.
Their marker files have the identical SHA-256
`425e2e3371a936609f8c296714f91d3ea309a6c9007c170ffbad80fc5e37ee18`;
this is an identity change, not a changed marker definition or pixel layout.

Hermes is private implementation detail in the gradient rasterizer. Gaia is
only a root dev dependency used by the mesh-viewer example in this checkout;
no production signature exposes Gaia. Those uses do not independently force
this public version increment. The public Eunomia marker implementations do.

## Decision

- Prepare the existing uniformly versioned workspace at 0.5.0, including its
  internal path requirements. Commit versions, provider requirements, final
  locks and migration records together after exact-head gates pass.
- Admit Eunomia 0.10 and owner-confirmed Hermes 0.9 through the existing Git
  plus version policy. Admit the coordinated Gaia 0.6 line for the example.
  Incoming RITK requirements must admit Metis 0.5.
- Admit published Moirai 0.9.3 through all seven existing source declarations,
  preserving URLs/features/defaults. Its merged PR623 `44a962f0` admits the
  published memory 0.10 facade. Leto 0.46 and Gaia 0.6 remain upstream gates;
  isolated provider validation does not establish Metis acceptance.
- Security renewal (2026-10-19) remains blocked: Git Gaia0.6.0 locks vulnerable
  ZIP2.4.2 (RustSec2026-0367/8/9). Current Git Gaia0.6.9 at
  `ea7411df38a8d9794e236c9ced302ab0ef872756` requires ZIP^9.0.4 and Git
  Moirai0.10. Updating it fails normal Cargo resolution because the public
  Metis/Consus0.3 cohort requires Moirai0.9.3. Coordinate that provider migration
  before acceptance; retain the current Git declarations and existing locks
  as a held preparation, never as release evidence.
- Reject ebc13104's registry Gaia alternative. Its archive matches current Git
  source, and the example exchanges primitive geometry/pixels, so the candidate
  preserves public identities and removes the ZIP advisories. However, the
  executable source policy in scripts/verify.py covers every direct dependency,
  including dev dependencies; Gaia is not an admitted registry boundary.
  Restore the existing Git graph. An ADR does not override that gate. No source
  alias, older pin, registry allowlist change, advisory ignore, feature reduction
  or reduced verification scope is authorized to make the candidate pass.
- Consumers using Color with layout bounds select the same Eunomia 0.10
  provider identity. No duplicate marker implementations, old-line aliases,
  feature reductions or compatibility packages bridge the old trait identity.
- Keep framebuffer layout, raster kernels, all Rust source and existing
  value/byte/frame-hash oracles unchanged. Preserve peer-owned PR503/PR504.
- Package publication of repaired packages/prerequisites is authorized after
  exact-revision gates and reviews pass. This supersedes the prior hold. No new
  credentials or publisher configuration is authorized; retain the separate
  Consus first-upload owner-authentication handoff. Upstream Miri and Aequitas
  documentation deployment remain with their existing owner. This consumer
  change does not touch protected allocation functions.

## Alternatives

- A 0.4.2 patch would omit the loss of the old public trait implementations.
- A second Eunomia dependency/compatibility alias would keep incompatible
  contracts in parallel instead of completing the intended provider migration.
- Changing pixel storage or adding a consumer-owned layout trait would duplicate
  provider ownership and alter a numerical/layout contract without need.

- Admit Consus0.3 after PR175's executed public async provider regression.
  AssetError's public From<consus_raster::DecodeError> implementation also
  follows the new provider identity; the existing0.5 release boundary covers
  this alongside the Eunomia marker migration. Both older raster upload
  commands are retired; await the verified0.3 human-authentication handoff.

## Verification

The source review checks both canonical trait definitions and both public
Color export routes. Manifest-only metadata must show every workspace package
and internal requirement at 0.5.0 without changing the existing lock. All source
files remain byte-identical to the preserved draft head. These static checks
do not replace dependency resolution or cargo-semver-checks.

After the current provider defaults are available, resolve root and fuzz locks,
then run the unchanged native/scalar/frame-hash, feature/target, strict Clippy,
docs/doctest, API, package and exact-head hosted gates. Earlier 0.9.2 results
remain historical evidence. No package is uploaded before these gates and reviews pass.
