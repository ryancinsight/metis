//! Session and fragment dispatch through bounded Moirai Fetch.
use super::{
    HttpTargets,
    application::{SharedState, current, element},
    codec,
    lifecycle::Lease,
};
use crate::fragment::apply_patch_set;
use metis_core::{
    capability::CapabilityToken,
    protocol::{ErrorResponsePayload, FragmentPatchSet, MAX_FRAGMENT_BODY_BYTES},
};
use moirai_pal::wasm::{WebDocument, WebHttp, WebHttpMethod, WebHttpResponse};
use std::{io, time::Duration};

const DEADLINE: Duration = Duration::from_secs(5);

pub(super) async fn binary(
    state: &SharedState,
    path: &str,
    body: &[u8],
) -> io::Result<WebHttpResponse> {
    let url = format!("{}{path}", state.borrow().origin.as_str());
    WebHttp::request(
        WebHttpMethod::Post,
        &url,
        Some("application/metis"),
        body,
        MAX_FRAGMENT_BODY_BYTES,
        DEADLINE,
    )
    .await
}
pub(super) async fn health(state: &SharedState) -> io::Result<()> {
    let url = format!("{}/health", state.borrow().origin.as_str());
    let response = WebHttp::request(
        WebHttpMethod::Get,
        &url,
        None,
        &[],
        MAX_FRAGMENT_BODY_BYTES,
        DEADLINE,
    )
    .await?;
    if response.status != 200 || response.body != b"metis-http-ready\n" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "health {}: response violates readiness contract",
                response.status
            ),
        ));
    }
    Ok(())
}
pub(super) async fn session(state: &SharedState, lease: Lease) -> io::Result<CapabilityToken> {
    current(state, lease)?;
    if let Some(token) = state.borrow().token.clone() {
        return Ok(token);
    }
    let response = binary(state, "/v1/session", &codec::handshake()).await?;
    current(state, lease)?;
    success(&response, "handshake")?;
    let token = codec::accept_handshake(&response.body)?;
    state.borrow_mut().token = Some(token.clone());
    Ok(token)
}
pub(super) async fn dispatch(
    state: &SharedState,
    lease: Lease,
    generation: u64,
    target: &str,
    value: &str,
) -> io::Result<FragmentPatchSet> {
    let token = session(state, lease).await?;
    current(state, lease)?;
    let response = binary(
        state,
        "/v1/fragments",
        &codec::invocation(token, generation, target, value)?,
    )
    .await?;
    current(state, lease)?;
    success(&response, "fragment")?;
    FragmentPatchSet::decode(&response.body).map_err(codec::invalid_data)
}
pub(super) async fn render(
    document: &WebDocument,
    state: &SharedState,
    lease: Lease,
) -> io::Result<()> {
    let value = element(document, "fragment-input")?
        .value()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Fragment input is not a text control",
            )
        })?;
    let patches = dispatch(state, lease, lease.mount(), "metis-events", &value).await?;
    current(state, lease)?;
    apply_patch_set::<HttpTargets>(document, lease.mount(), &patches)?;
    element(document, "http-response")?.set_text(&format!(
        "200 metis-http-ready · handshake 200 · fragment 200 ({} patch)",
        patches.patches().len()
    ));
    element(document, "metis-status")?.set_text("Authenticated fragment boundary ready");
    Ok(())
}
fn success(response: &WebHttpResponse, operation: &str) -> io::Result<()> {
    if response.status == 200 {
        return Ok(());
    }
    let error = ErrorResponsePayload::decode(&response.body).map_err(codec::invalid_data)?;
    let kind = match response.status {
        401 | 403 => io::ErrorKind::PermissionDenied,
        404 => io::ErrorKind::NotFound,
        408 | 504 => io::ErrorKind::TimedOut,
        409 => io::ErrorKind::AlreadyExists,
        400 | 413 | 422 => io::ErrorKind::InvalidData,
        _ => io::ErrorKind::Other,
    };
    Err(io::Error::new(
        kind,
        format!(
            "{operation} {}: error {} {}",
            response.status, error.error_code, error.message
        ),
    ))
}
