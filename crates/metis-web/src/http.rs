//! Rust-owned authenticated HTTP presentation and request lifecycle.
mod codec;
mod lifecycle;

pub(crate) struct HttpTargets;
impl crate::fragment::TargetPolicy for HttpTargets {
    fn allows(target: &str) -> bool {
        matches!(target, "metis-events" | "metis-status")
    }
}
#[cfg(target_arch = "wasm32")]
mod application;
#[cfg(test)]
mod codec_tests;
#[cfg(test)]
mod lifecycle_tests;
#[cfg(target_arch = "wasm32")]
mod probes;
#[cfg(target_arch = "wasm32")]
mod request;
#[cfg(test)]
mod test_support;
#[cfg(target_arch = "wasm32")]
pub use application::{metis_http_start, metis_http_stop};
