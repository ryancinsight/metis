//! LibFuzzer target for `DeepLink::parse`, the operating-system-supplied
//! custom-scheme link.

#![no_main]

use libfuzzer_sys::fuzz_target;
use metis_core::deep_link::{DeepLink, DeepLinkScheme};
use std::sync::LazyLock;

/// Declared schemes: a reverse-domain scheme and a short one, so the target
/// reaches both the scheme match and the `//` authority-prefix branches.
static SCHEMES: LazyLock<Vec<DeepLinkScheme>> = LazyLock::new(|| {
    ["org.example.viewer", "viewer"]
        .into_iter()
        .map(|scheme| DeepLinkScheme::new(scheme).expect("invariant: a valid scheme"))
        .collect()
});

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        drop(std::hint::black_box(DeepLink::parse(text, &SCHEMES)));
    }
});
