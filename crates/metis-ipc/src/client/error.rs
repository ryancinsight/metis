//! The typed failures each client operation can return.

use metis_core::error::MetisError;
use metis_core::protocol::ErrorResponsePayload;

/// Failure to establish a session, preserving local faults and peer rejections.
///
/// Remote codes retain their wire value, including codes unknown to this client.
/// A malformed rejection is a local decoding failure, not a remote error.
///
/// # Examples
/// ```
/// use metis_core::protocol::{ErrorResponsePayload, MessageType};
/// use metis_ipc::client::HandshakeError;
/// use metis_ipc::{IpcClient, IpcTransport, MemoryTransport};
///
/// let (transport, mut peer) = MemoryTransport::pair();
/// let rejection = ErrorResponsePayload {
///     error_code: 0xffff,
///     message: "Session rejected".into(),
/// };
/// peer.send_message(MessageType::ErrorResp, 1, &rejection.encode()?)?;
/// let mut client = IpcClient::new(transport);
/// assert_eq!(client.handshake(1, [1; 16]), Err(HandshakeError::Remote(rejection)));
/// # Ok::<(), metis_core::error::MetisError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum HandshakeError {
    /// Transport, correlation, decoding, or session validation fails locally.
    Local(MetisError),
    /// The correlated peer response rejects session initialization.
    Remote(ErrorResponsePayload),
}

impl From<MetisError> for HandshakeError {
    fn from(error: MetisError) -> Self {
        Self::Local(error)
    }
}

impl std::fmt::Display for HandshakeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local(error) => error.fmt(formatter),
            Self::Remote(error) => write!(
                formatter,
                "Peer rejected handshake [0x{:04X}]: {}",
                error.error_code, error.message
            ),
        }
    }
}

impl std::error::Error for HandshakeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // The standard error source API requires type erasure for diagnostics.
        match self {
            Self::Local(error) => Some(error),
            Self::Remote(_) => None,
        }
    }
}

/// Failure to discover a host catalog, preserving local faults and peer rejections.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CapabilityError {
    /// Transport, correlation, decoding, or version validation fails locally.
    Local(MetisError),
    /// The correlated peer response rejects capability discovery.
    Remote(ErrorResponsePayload),
}

impl From<MetisError> for CapabilityError {
    fn from(error: MetisError) -> Self {
        Self::Local(error)
    }
}

impl std::fmt::Display for CapabilityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local(error) => error.fmt(formatter),
            Self::Remote(error) => write!(
                formatter,
                "Peer rejected capability discovery [0x{:04X}]: {}",
                error.error_code, error.message
            ),
        }
    }
}

impl std::error::Error for CapabilityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Local(error) => Some(error),
            Self::Remote(_) => None,
        }
    }
}

/// Failure to discover the connected host target and its implemented surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TargetCapabilityError {
    /// Transport, correlation, decoding, or version validation fails locally.
    Local(MetisError),
    /// The correlated peer rejects target discovery.
    Remote(ErrorResponsePayload),
}

impl From<MetisError> for TargetCapabilityError {
    fn from(error: MetisError) -> Self {
        Self::Local(error)
    }
}

impl std::fmt::Display for TargetCapabilityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local(error) => error.fmt(formatter),
            Self::Remote(error) => write!(
                formatter,
                "Peer rejected target capability discovery [0x{:04X}]: {}",
                error.error_code, error.message
            ),
        }
    }
}

impl std::error::Error for TargetCapabilityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Local(error) => Some(error),
            Self::Remote(_) => None,
        }
    }
}

/// Failure to invoke a remote plugin, preserving local faults and peer errors.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PluginInvocationError {
    /// Transport, correlation, decoding, or response validation fails locally.
    Local(MetisError),
    /// The correlated peer rejects the plugin operation.
    Remote(ErrorResponsePayload),
}

impl From<MetisError> for PluginInvocationError {
    fn from(error: MetisError) -> Self {
        Self::Local(error)
    }
}

impl std::fmt::Display for PluginInvocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local(error) => error.fmt(formatter),
            Self::Remote(error) => write!(
                formatter,
                "Peer rejected plugin invocation [0x{:04X}]: {}",
                error.error_code, error.message
            ),
        }
    }
}

impl std::error::Error for PluginInvocationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Local(error) => Some(error),
            Self::Remote(_) => None,
        }
    }
}
