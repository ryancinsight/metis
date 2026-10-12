//! Real loopback denial and stale-patch oracles.
use super::{
    HttpTargets,
    application::{SharedState, current, element},
    codec,
    lifecycle::Lease,
    request,
};
use crate::fragment::apply_patch_set;
use metis_core::{error::ErrorCode, protocol::ErrorResponsePayload};
use moirai_pal::wasm::WebDocument;
use std::io;

pub(super) async fn probe(
    document: &WebDocument,
    state: &SharedState,
    lease: Lease,
) -> io::Result<()> {
    request::health(state).await?;
    current(state, lease)?;
    request::render(document, state, lease).await?;
    let response = request::binary(state, "/v1/fragments", &[]).await?;
    current(state, lease)?;
    require(
        response.status == 400,
        "Malformed fragment did not return 400",
    )?;
    let value = element(document, "fragment-input")?
        .value()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Fragment input is not a text control",
            )
        })?;
    let token = request::session(state, lease).await?;
    let mut missing = token.clone();
    missing.principal_id[0] ^= 1;
    let response = request::binary(
        state,
        "/v1/fragments",
        &codec::invocation(missing, lease.mount(), "metis-events", &value)?,
    )
    .await?;
    current(state, lease)?;
    denial(&response, 401, ErrorCode::MissingCapability)?;
    let mut tampered = token;
    tampered.signature[0] ^= 1;
    let response = request::binary(
        state,
        "/v1/fragments",
        &codec::invocation(tampered, lease.mount(), "metis-events", &value)?,
    )
    .await?;
    current(state, lease)?;
    // The backend binds the complete issued grant before signature/scope
    // verification. Any changed grant is an InvalidPrincipal denial.
    denial(&response, 403, ErrorCode::InvalidPrincipal)?;
    let events = element(document, "metis-events")?;
    let before = events.text_content().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Fragment target has no textContent",
        )
    })?;
    let forbidden =
        request::dispatch(state, lease, lease.mount(), "metis-forbidden", &value).await?;
    current(state, lease)?;
    let result = apply_patch_set::<HttpTargets>(document, lease.mount(), &forbidden);
    require(
        matches!(result, Err(error) if error.kind() == io::ErrorKind::PermissionDenied)
            && events.text_content().as_deref() == Some(before.as_str()),
        "Unallowlisted target changed presentation",
    )?;
    let stale_generation = lease
        .mount()
        .checked_add(1)
        .ok_or_else(|| io::Error::other("HTTP mount generation exhausted"))?;
    let mismatched_patches =
        request::dispatch(state, lease, stale_generation, "metis-events", &value).await?;
    current(state, lease)?;
    let result = apply_patch_set::<HttpTargets>(document, lease.mount(), &mismatched_patches);
    require(
        matches!(result, Err(error) if error.kind() == io::ErrorKind::InvalidInput)
            && events.text_content().as_deref() == Some(before.as_str()),
        "Stale fragment changed presentation",
    )?;
    element(document, "metis-negative")?.set_text(
        "malformed 400 · unauthorized 401 · changed grant 403 · target rejected · stale unchanged",
    );
    Ok(())
}
fn denial(
    response: &moirai_pal::wasm::WebHttpResponse,
    status: u16,
    code: ErrorCode,
) -> io::Result<()> {
    let error = ErrorResponsePayload::decode(&response.body).map_err(codec::invalid_data)?;
    if response.status == status && ErrorCode::from_wire(error.error_code) == Some(code) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Denial {} error {} {} differs from {status} {code:?}",
                response.status, error.error_code, error.message
            ),
        ))
    }
}
fn require(condition: bool, message: &'static str) -> io::Result<()> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::InvalidData, message))
    }
}
