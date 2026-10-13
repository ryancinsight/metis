//! Native QUIC service composition over the canonical IPC dispatcher.

use crate::service::{BackendService, Clock};
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::protocol::TargetCapability;
use metis_ipc::AsyncIpcServer;
use moirai_quic::QuicStream;

/// Serve one independently authorized native session over an authenticated stream.
///
/// The caller verifies the TLS peer's identity against its application policy
/// and constructs an independent trusted `HostContext` for this stream. The
/// service also requires a fresh session MAC key, as on the private pipe route.
/// connection's server certificate does not authenticate a client principal.
/// Commands never use early data or unreliable datagrams; capability, replay,
/// clinical, audit and event behavior use the existing dispatcher.
///
/// # Errors
/// Rejects a missing trusted host context before application I/O, wrong ALPN,
/// insufficient frame admission, or normal protocol/handler/transport failures.
pub async fn serve_quic_session<C: Clock>(
    stream: QuicStream,
    mut service: BackendService<C>,
) -> Result<()> {
    if !service.has_trusted_context() {
        return Err(MetisError::capability(
            ErrorCode::InvalidPrincipal,
            "QUIC service requires an independently authenticated trusted host context",
        ));
    }
    service.add_target_capability(TargetCapability::NativeQuic)?;
    let mut server = AsyncIpcServer::new(stream);
    server.run(&mut service).await
}

#[cfg(test)]
mod tests;
