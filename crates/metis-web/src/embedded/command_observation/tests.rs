//! Portable value contracts for the real codec and shared accounting owner.

mod bounds;
mod budget;
mod rejection;
mod roundtrip;
mod selection;

use crate::embedded::{PageIdentity, PageRole, SequenceId};
use metis_core::embedded_assets::Digest;

fn identity(mount: u64) -> PageIdentity {
    PageIdentity::new(
        PageRole::Form,
        SequenceId::new(mount).expect("positive observed mount"),
        Digest::hash(b"command observation identity through the real SHA-256 provider"),
    )
}

fn trace() -> tracing::subscriber::DefaultGuard {
    tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
            .with_test_writer()
            .finish(),
    )
}
