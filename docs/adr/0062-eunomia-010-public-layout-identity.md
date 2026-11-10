# 0062 — Eunomia public layout trait identity

Status: Accepted

Date: 2026-10-15

Change class: [major], pre-1.0 minor increment

Driver: METIS-CURRENT-PROVIDERS-001

Revision 2026-11-10: The selected cohort advances Eunomia 0.11 to 0.12.
Review of historical commit `406b13cb21cb6183fa29246af959463bd6cb09c4`,
the candidate prepared by [Metis PR #519], both provider sources, and the
affected public routes confirms one nominal trait-identity break exposed by
three Metis packages.
This revision replaces delivery history with the resulting contract,
migration, package disposition, and evidence limits.

## Context

Historical commit `406b13cb21cb6183fa29246af959463bd6cb09c4`
locks Eunomia 0.11.0 at
`d8ee31abf9e10c3aaa4efd67ed5912a612086115`. The candidate locks Eunomia
0.12.0 at `00ffd14a9e2791795ded32e3b4842e8eb79208ce`.

`metis_platform::Color` publicly derives `eunomia::Pod` and
`eunomia::Zeroable`. `metis-platform` exports that type from its crate root;
`metis_ui_lang::style::Color` and the `metis-ui-lang` crate root re-export the
same type. A generic consumer bound to Eunomia 0.11's traits therefore cannot
use the Eunomia 0.12 implementations, even though the trait definitions and
the implementing type's representation are unchanged.
`metis_frontend::FrontendApp::framebuffer` also returns the public platform
`Framebuffer`; its public `get_pixel` method yields this `Color`, so the
frontend access path exposes the same migrated implementation.

Direct comparison of the provider revisions above shows byte-identical
`layout/marker.rs` sources and identical derive-crate trees. `Color` remains
`#[repr(C)]` with four public `u8` fields in red, green, blue, alpha order,
and its compile-time size/alignment assertion remains four bytes at byte
alignment. The break is nominal Rust trait identity, not layout or value
behavior.

## Decision

- Classify the Eunomia 0.11 to 0.12 transition as a breaking public change for
  `metis-platform`, `metis-ui-lang`, and the `metis-frontend` framebuffer
  access path. Under the uniformly versioned pre-1.0 workspace policy,
  release classification is a minor increment.
- Classify the candidate relative to fetched main
  `318c83eca0e2a619b095075d419aed6292cbbec9` as a patch reconciliation. That
  base already selects Eunomia 0.12; the candidate does not introduce another
  trait-identity transition.
- Consumers with `eunomia::Pod` or `eunomia::Zeroable` bounds over Metis
  `Color` update their direct Eunomia dependency and bounds to 0.12 in the
  same migration. Consumers that only construct, compare, render, or marshal
  `Color` require no source change, but resolve the selected provider cohort.
- Do not add an old-trait alias, duplicate implementation, compatibility
  package, version rollback, or wider requirement to bridge the identities.

## Affected package disposition

| Package | Historical native result | Actual Eunomia contract |
| --- | --- | --- |
| `metis-platform` | reconstruction unsupported | Breaking: exports `Color` with the selected layout-trait impls. |
| `metis-ui-lang` | reconstruction unsupported | Breaking: re-exports the same `Color`. |
| `metis-backend` | reconstruction unsupported | No Eunomia type or bound in its public surface; dependency is transitive. |
| `metis-frontend` | reconstruction unsupported | Breaking access path: public `FrontendApp::framebuffer` reaches the migrated `Color`. |
| `metis-python` | reconstruction unsupported | Python boundary exposes RGBA bytes, not Rust layout traits. |
| `metis-web` | reconstruction unsupported | Browser boundary exposes RGBA bytes, not Rust layout traits. |

The six unsupported results are dependency-reconstruction failures, not six
API verdicts. Cargo-semver-checks 0.51 cannot resolve the historical 0.11 Git
requirement from the selected 0.12 provider state. It must not be reported as
a passing native comparison. The other five Rust library packages complete
229 checks with 229 passed and 31 skipped per package against this baseline.

## Consumer contracts

- `Color`'s Rustdoc example is an external-style consumer: its generic bound
  requires the selected `Pod` and `Zeroable` identities, checks exact RGBA byte
  order through `eunomia::layout::bytes_of`, and checks the zero value.
- `rgba_bytes_preserve_row_major_channels_and_reject_length_mismatch` and
  `admitted_color_forms_expand_to_exact_straight_channels` check the public
  RGBA conversion behavior used by UI and Python consumers.
- `locale_accepts_the_admitted_subset` and
  `locale_rejects_everything_outside_the_subset` check that the unrelated
  integer-conversion cleanup preserves locale behavior.
- `actual_quic_frames_retain_truncation_checksum_size_and_trailing_byte_errors`
  checks the wire behavior around the unchanged `65_560` byte frame limit.
- RITK's exact-C14 consumer preparation runs
  `cargo nextest run -p ritk-snap --locked`: 757 passed and none skipped. Its
  value contracts preserve provider event values and coordinates, exact RGBA
  conversion, one-frame host outcome counts, session framebuffer/navigation/
  gesture/overlay behavior, and browser semantic attributes and dimensions.
  This does not exercise Metis's Python binding or browser feature matrix.

## Rejected alternatives

- Treating identical trait source as compatible rejects Rust's nominal trait
  identity and leaves old generic consumers uncompilable.
- Calling all six unsupported comparisons breaking overstates transitive
  resolution failure as public exposure.
- Calling compiler artifacts behavioral evidence omits the byte, locale,
  transport, and downstream value contracts.
- Preserving both provider identities creates the compatibility layer this
  migration is intended to remove.

## Consequences and verification

The source migration is limited to provider selection, documentation, and
real consumers. It does not change `Color` fields, representation, pixel
packing, raster arithmetic, locale semantics, QUIC framing, or Python/browser
byte boundaries.

Candidate verification includes workspace format/check/Clippy, 845 passed and
3 skipped Nextest tests, doctests, cast and conformance gates, and the normal
push gate over `318c83ec..14f9eab`. The selected-layout Rustdoc consumer adds
one exact public-contract case; all 10 `metis-platform` doctests pass. The RITK
consumer result above supplies downstream behavior rather than compiler-only
evidence. Cargo-semver-checks passes all 11 library packages against the live
base. Against `406b13cb`, five packages pass and the six listed above remain
explicitly unsupported. Registry inventory finds no released 0.5.0 contract
for the affected Rust library packages; this does not turn unsupported
historical coverage green.

[Metis PR #519]: https://github.com/ryancinsight/metis/pull/519
