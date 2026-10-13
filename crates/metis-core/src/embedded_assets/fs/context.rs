//! Producer configuration and native asset consumption have distinct authority.
mod sealed {
    pub trait Sealed {}
}

/// Statically selects the owning configuration context during asset admission.
///
/// Both contexts verify current source/configuration files, locked providers,
/// directory membership, tool executable bytes/version/pin and retained assets.
/// Only the producer interprets its current environment as WASM compiler input.
pub trait VerificationContext: sealed::Sealed {
    /// Whether current command selectors/environment belong to the WASM producer.
    const PRODUCER_ENVIRONMENT: bool;
}

/// Current WASM producer configuration, including recorded unset/empty inputs.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProducerContext;
impl sealed::Sealed for ProducerContext {}
impl VerificationContext for ProducerContext {
    const PRODUCER_ENVIRONMENT: bool = true;
}

/// Native compilation consumes the inventory's fixed WASM configuration.
///
/// Native Cargo's derived flags/wrappers describe native compilation and cannot
/// reinterpret the previously compiled module's producer configuration.
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeContext;
impl sealed::Sealed for NativeContext {}
impl VerificationContext for NativeContext {
    const PRODUCER_ENVIRONMENT: bool = false;
}

const _: () = {
    assert!(std::mem::size_of::<ProducerContext>() == 0);
    assert!(std::mem::size_of::<NativeContext>() == 0);
};
