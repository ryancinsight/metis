//! Versioned host-target identity and surface capability payloads.

use super::wire::{MAX_PAYLOAD_SIZE, finish, malformed, take};
use crate::error::{ErrorCode, MetisError, Result};

/// Maximum target surfaces advertised by one host.
pub const MAX_TARGET_CAPABILITIES: usize = 16;

/// Execution target that owns a Metis host boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[repr(u8)]
pub enum TargetPlatform {
    /// Microsoft Windows.
    Windows = 1,
    /// Apple macOS.
    MacOs = 2,
    /// Linux desktop or service host.
    Linux = 3,
    /// WebAssembly browser target.
    Wasm = 4,
    /// A target not assigned a dedicated wire identifier.
    Other = 255,
}

impl TargetPlatform {
    /// Resolves a target identifier from its wire representation.
    #[must_use]
    pub const fn from_wire(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Windows),
            2 => Some(Self::MacOs),
            3 => Some(Self::Linux),
            4 => Some(Self::Wasm),
            255 => Some(Self::Other),
            _ => None,
        }
    }

    /// Returns the target selected by the compiling executable.
    #[must_use]
    pub const fn current() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            Self::Wasm
        }
        #[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
        {
            Self::Windows
        }
        #[cfg(all(not(target_arch = "wasm32"), target_os = "macos"))]
        {
            Self::MacOs
        }
        #[cfg(all(not(target_arch = "wasm32"), target_os = "linux"))]
        {
            Self::Linux
        }
        #[cfg(not(any(
            target_arch = "wasm32",
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        )))]
        {
            Self::Other
        }
    }

    /// Returns the stable display name used by diagnostics and manuals.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::MacOs => "macos",
            Self::Linux => "linux",
            Self::Wasm => "wasm",
            Self::Other => "other",
        }
    }
}

/// Host surface that a connected target actually exposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[repr(u16)]
pub enum TargetCapability {
    /// A native executable hosts the service boundary.
    NativeProcess = 1,
    /// A supervised child process is connected through private IPC.
    PrivateProcessIpc = 2,
    /// A browser can connect through the authenticated WebSocket bridge.
    BrowserWebSocket = 3,
    /// The application is compiled to WebAssembly.
    WasmApplication = 4,
    /// The application renders through HTML DOM elements.
    DomRendering = 5,
    /// The application uses browser CSS styling.
    CssStyling = 6,
    /// A native top-level window is available.
    NativeWindow = 7,
    /// The host exposes an operating-system permission broker.
    OsPermissions = 8,
    /// The host exposes an accessibility integration.
    Accessibility = 9,
    /// The host exposes native text composition and IME integration.
    Ime = 10,
}

impl TargetCapability {
    /// Resolves a capability identifier from its wire representation.
    #[must_use]
    pub const fn from_wire(value: u16) -> Option<Self> {
        match value {
            1 => Some(Self::NativeProcess),
            2 => Some(Self::PrivateProcessIpc),
            3 => Some(Self::BrowserWebSocket),
            4 => Some(Self::WasmApplication),
            5 => Some(Self::DomRendering),
            6 => Some(Self::CssStyling),
            7 => Some(Self::NativeWindow),
            8 => Some(Self::OsPermissions),
            9 => Some(Self::Accessibility),
            10 => Some(Self::Ime),
            _ => None,
        }
    }

    /// Returns the stable display name used by diagnostics and manuals.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::NativeProcess => "native-process",
            Self::PrivateProcessIpc => "private-process-ipc",
            Self::BrowserWebSocket => "browser-websocket",
            Self::WasmApplication => "wasm-application",
            Self::DomRendering => "dom-rendering",
            Self::CssStyling => "css-styling",
            Self::NativeWindow => "native-window",
            Self::OsPermissions => "os-permissions",
            Self::Accessibility => "accessibility",
            Self::Ime => "ime",
        }
    }
}

/// Versioned target identity with a bounded list of supported host surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetCapabilityPayload {
    /// Wire protocol version used by this target descriptor.
    protocol_version: u16,
    /// Target that owns the advertised host surfaces.
    platform: TargetPlatform,
    /// Surfaces implemented by the target host.
    capabilities: Vec<TargetCapability>,
}

impl TargetCapabilityPayload {
    /// Builds a target descriptor after validating its bounded surface list.
    ///
    /// # Errors
    /// Returns a protocol error when the list exceeds [`MAX_TARGET_CAPABILITIES`]
    /// or contains a duplicate surface.
    pub fn new(
        platform: TargetPlatform,
        requested: impl IntoIterator<Item = TargetCapability>,
    ) -> Result<Self> {
        let mut capabilities = Vec::with_capacity(MAX_TARGET_CAPABILITIES);
        for capability in requested {
            if capabilities.len() == MAX_TARGET_CAPABILITIES {
                return Err(MetisError::protocol(
                    ErrorCode::PayloadTooLarge,
                    "Target capability list exceeds its surface bound",
                ));
            }
            capabilities.push(capability);
        }
        validate_capabilities(&capabilities)?;
        Ok(Self {
            protocol_version: super::wire::PROTOCOL_VERSION,
            platform,
            capabilities,
        })
    }

    /// Returns the native process descriptor used by a backend service.
    ///
    /// # Panics
    /// Panics only if the fixed native descriptor violates its compile-time
    /// surface bound or contains a duplicate capability.
    #[must_use]
    pub fn native_service() -> Self {
        Self::new(TargetPlatform::current(), [TargetCapability::NativeProcess])
            .expect("invariant: native target descriptor is bounded and unique")
    }

    /// Returns the browser-side surfaces compiled into the WASM application.
    ///
    /// # Panics
    /// Panics only if the fixed browser descriptor violates its surface bound
    /// or contains a duplicate capability.
    #[cfg(target_arch = "wasm32")]
    #[must_use]
    pub fn browser_application() -> Self {
        Self::new(
            TargetPlatform::current(),
            [
                TargetCapability::WasmApplication,
                TargetCapability::DomRendering,
                TargetCapability::CssStyling,
            ],
        )
        .expect("invariant: browser target descriptor is bounded and unique")
    }

    /// Adds one host surface while preserving the descriptor invariant.
    ///
    /// # Errors
    /// Returns [`ErrorCode::PayloadTooLarge`] when the descriptor is full.
    pub fn add_capability(&mut self, capability: TargetCapability) -> Result<()> {
        if self.capabilities.contains(&capability) {
            return Ok(());
        }
        if self.capabilities.len() == MAX_TARGET_CAPABILITIES {
            return Err(MetisError::protocol(
                ErrorCode::PayloadTooLarge,
                "Target capability list exceeds its surface bound",
            ));
        }
        self.capabilities.push(capability);
        Ok(())
    }

    /// Returns the descriptor's protocol version.
    #[must_use]
    pub const fn protocol_version(&self) -> u16 {
        self.protocol_version
    }

    /// Returns the target platform.
    #[must_use]
    pub const fn platform(&self) -> TargetPlatform {
        self.platform
    }

    /// Returns the validated surfaces in wire order.
    #[must_use]
    pub fn capabilities(&self) -> &[TargetCapability] {
        &self.capabilities
    }

    /// Returns whether a surface is advertised by the target.
    #[must_use]
    pub fn supports(&self, capability: TargetCapability) -> bool {
        self.capabilities.contains(&capability)
    }

    /// Encodes the bounded target descriptor.
    ///
    /// # Errors
    /// Returns a protocol error when the descriptor violates its bounds.
    pub fn encode(&self) -> Result<Vec<u8>> {
        validate_capabilities(&self.capabilities)?;
        let count = u8::try_from(self.capabilities.len()).map_err(|_| {
            MetisError::protocol(
                ErrorCode::PayloadTooLarge,
                "Target capability count exceeds its wire field",
            )
        })?;
        let payload_len = 4usize
            .checked_add(self.capabilities.len().checked_mul(2).ok_or_else(|| {
                MetisError::protocol(
                    ErrorCode::PayloadTooLarge,
                    "Target capability size overflows its resource bound",
                )
            })?)
            .ok_or_else(|| {
                MetisError::protocol(
                    ErrorCode::PayloadTooLarge,
                    "Target capability size overflows its resource bound",
                )
            })?;
        if payload_len > MAX_PAYLOAD_SIZE {
            return Err(MetisError::protocol(
                ErrorCode::PayloadTooLarge,
                "Target capability descriptor exceeds the payload resource bound",
            ));
        }
        let mut payload = Vec::with_capacity(payload_len);
        payload.extend_from_slice(&self.protocol_version.to_be_bytes());
        payload.push(self.platform as u8);
        payload.push(count);
        for capability in &self.capabilities {
            payload.extend_from_slice(&(*capability as u16).to_be_bytes());
        }
        Ok(payload)
    }

    /// Decodes a target descriptor from untrusted wire bytes.
    ///
    /// # Errors
    /// Rejects truncation, trailing bytes, unknown platform or surface
    /// identifiers, duplicate surfaces, and descriptors over the bound.
    pub fn decode(mut payload: &[u8]) -> Result<Self> {
        if payload.len() > MAX_PAYLOAD_SIZE {
            return Err(MetisError::protocol(
                ErrorCode::PayloadTooLarge,
                "Target capability descriptor exceeds the payload resource bound",
            ));
        }
        let protocol_version = u16::from_be_bytes(take(&mut payload)?);
        let platform = TargetPlatform::from_wire(take::<1>(&mut payload)?[0]).ok_or_else(|| {
            MetisError::protocol(
                ErrorCode::UnexpectedMessageType,
                "Target capability descriptor contains an unknown platform",
            )
        })?;
        let count = usize::from(take::<1>(&mut payload)?[0]);
        if count > MAX_TARGET_CAPABILITIES {
            return Err(MetisError::protocol(
                ErrorCode::PayloadTooLarge,
                "Target capability descriptor exceeds its surface bound",
            ));
        }
        let mut capabilities = Vec::with_capacity(count);
        for _ in 0..count {
            let raw = u16::from_be_bytes(take(&mut payload)?);
            let capability = TargetCapability::from_wire(raw).ok_or_else(|| {
                MetisError::protocol(
                    ErrorCode::UnexpectedMessageType,
                    "Target capability descriptor contains an unknown surface",
                )
            })?;
            capabilities.push(capability);
        }
        finish(payload)?;
        validate_capabilities(&capabilities)?;
        Ok(Self {
            protocol_version,
            platform,
            capabilities,
        })
    }
}

fn validate_capabilities(capabilities: &[TargetCapability]) -> Result<()> {
    if capabilities.len() > MAX_TARGET_CAPABILITIES {
        return Err(MetisError::protocol(
            ErrorCode::PayloadTooLarge,
            "Target capability descriptor exceeds its surface bound",
        ));
    }
    for (index, capability) in capabilities.iter().enumerate() {
        if capabilities[..index].contains(capability) {
            return Err(malformed(
                "Target capability descriptor contains a duplicate surface",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_descriptor_round_trip_preserves_platform_and_surfaces() {
        let descriptor = TargetCapabilityPayload::new(
            TargetPlatform::Windows,
            [
                TargetCapability::NativeProcess,
                TargetCapability::PrivateProcessIpc,
            ],
        )
        .expect("descriptor");
        let encoded = descriptor.encode().expect("encoded descriptor");
        let decoded = TargetCapabilityPayload::decode(&encoded).expect("decoded descriptor");
        assert_eq!(decoded, descriptor);
        assert_eq!(decoded.platform(), TargetPlatform::Windows);
        assert!(decoded.supports(TargetCapability::PrivateProcessIpc));
        assert!(!decoded.supports(TargetCapability::NativeWindow));
    }

    #[test]
    fn target_descriptor_rejects_unknown_duplicate_and_trailing_data() {
        for encoded in [
            [0x01, 0x00, 0x7f, 0x00].as_slice(),
            [0x01, 0x00, 0x01, 0x02, 0x00, 0x01, 0x00, 0x01].as_slice(),
            [0x01, 0x00, 0x01, 0x00, 0xaa].as_slice(),
        ] {
            let error = TargetCapabilityPayload::decode(encoded).expect_err("invalid descriptor");
            assert!(matches!(
                error.code,
                ErrorCode::UnexpectedMessageType | ErrorCode::MalformedPayload
            ));
        }
    }

    #[test]
    fn target_descriptor_rejects_more_than_the_bounded_surface_count() {
        let capabilities =
            std::iter::repeat_n(TargetCapability::NativeProcess, MAX_TARGET_CAPABILITIES + 1);
        let error = TargetCapabilityPayload::new(TargetPlatform::Other, capabilities)
            .expect_err("descriptor bound");
        assert_eq!(error.code, ErrorCode::PayloadTooLarge);
    }
}
