//! HTTP envelopes reuse authoritative Metis protocol codecs.
use metis_core::{
    capability::{CapabilityScope, CapabilityToken},
    protocol::{
        FragmentAction, HandshakeRequestPayload, HandshakeResponsePayload, PROTOCOL_VERSION,
        PluginInvocationPayload,
    },
};
use std::io;

pub(super) fn endpoint_origin(endpoint: &str) -> io::Result<metis_core::host::HostOrigin> {
    let origin = metis_core::host::HostOrigin::from_url(endpoint).map_err(invalid_data)?;
    if (!origin.as_str().starts_with("http://") && !origin.as_str().starts_with("https://"))
        || endpoint != format!("{}/health", origin.as_str())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "HTTP endpoint must name the canonical HTTP(S) /health URL",
        ));
    }
    Ok(origin)
}
/// Public demonstration principal matching the trusted loopback launcher.
pub(super) const PRINCIPAL: [u8; 16] = [0x66; 16];
pub(super) fn handshake() -> Vec<u8> {
    HandshakeRequestPayload {
        client_version: PROTOCOL_VERSION,
        client_process_id: 42,
        principal_id: PRINCIPAL,
    }
    .encode()
}
pub(super) fn accept_handshake(body: &[u8]) -> io::Result<CapabilityToken> {
    let response = HandshakeResponsePayload::decode(body).map_err(invalid_data)?;
    if response.server_version != PROTOCOL_VERSION
        || response.initial_token.principal_id != PRINCIPAL
        || !response
            .initial_token
            .scope
            .contains(CapabilityScope::UI_RENDER)
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "HTTP handshake version, principal or UI scope is invalid",
        ));
    }
    Ok(response.initial_token)
}
pub(super) fn invocation(
    token: CapabilityToken,
    generation: u64,
    target: &str,
    value: &str,
) -> io::Result<Vec<u8>> {
    let action =
        FragmentAction::new(generation, "status.describe", target, value).map_err(invalid_data)?;
    PluginInvocationPayload::new(
        token,
        "ui",
        "action",
        action.encode().map_err(invalid_data)?,
    )
    .and_then(|invocation| invocation.encode())
    .map_err(invalid_data)
}
pub(super) fn invalid_data(error: metis_core::error::MetisError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}
