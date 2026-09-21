//! Capability-witnessed native HTTP requests with an explicit origin allowlist.

use metis_core::capability::CapabilityScope;
use metis_core::host::{HostOrigin, VerifiedHostCapability};
use moirai_async::timer::timeout;
use moirai_http::{HttpClient, Response};
use std::io;
use std::time::Duration;

/// Maximum number of configured HTTP origins.
pub const MAX_SCOPED_HTTP_ORIGINS: usize = 16;
/// Maximum encoded request URL length.
pub const MAX_SCOPED_HTTP_URL_BYTES: usize = 2 * 1024;
/// Maximum HTTP method length.
pub const MAX_SCOPED_HTTP_METHOD_BYTES: usize = 32;
/// Maximum number of request headers.
pub const MAX_SCOPED_HTTP_HEADERS: usize = 32;
/// Maximum encoded request-header bytes, including names and values.
pub const MAX_SCOPED_HTTP_HEADER_BYTES: usize = 16 * 1024;
/// Maximum request body bytes.
pub const MAX_SCOPED_HTTP_BODY_BYTES: usize = 8 * 1024 * 1024;
/// Maximum response bytes retained by the provider.
pub const MAX_SCOPED_HTTP_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
/// Maximum logical request deadline admitted by the provider.
pub const MAX_SCOPED_HTTP_DEADLINE: Duration = Duration::from_secs(30);

const FORBIDDEN_HEADERS: [&str; 9] = [
    "connection",
    "content-length",
    "host",
    "keep-alive",
    "proxy-authorization",
    "proxy-connection",
    "te",
    "trailer",
    "transfer-encoding",
];

/// A bounded HTTP request validated before it reaches the transport.
#[derive(Clone, PartialEq, Eq)]
pub struct ScopedHttpRequest {
    method: String,
    url: String,
    headers: Box<[(String, String)]>,
    body: Option<Box<[u8]>>,
}

impl ScopedHttpRequest {
    /// Creates a request from owned method, URL, headers and optional body.
    ///
    /// Header names and values are validated for HTTP token/control-byte
    /// safety. Transport-owned hop-by-hop fields are rejected so callers
    /// cannot replace the provider's host or framing decisions.
    ///
    /// # Errors
    /// Returns a typed error when a request component exceeds its bound or is
    /// not a valid HTTP field.
    pub fn new<I, K, V>(
        method: impl Into<String>,
        url: impl Into<String>,
        headers: I,
        body: Option<Vec<u8>>,
    ) -> Result<Self, ScopedHttpError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let method = method.into();
        validate_method(&method)?;
        let url = url.into();
        validate_url(&url)?;

        let mut checked_headers = Vec::new();
        let mut header_bytes = 0usize;
        for (name, value) in headers {
            if checked_headers.len() >= MAX_SCOPED_HTTP_HEADERS {
                return Err(ScopedHttpError::TooManyHeaders);
            }
            let name = name.into();
            let value = value.into();
            validate_header(&name, &value)?;
            header_bytes = header_bytes
                .checked_add(name.len())
                .and_then(|bytes| bytes.checked_add(value.len()))
                .ok_or(ScopedHttpError::HeadersTooLarge)?;
            if header_bytes > MAX_SCOPED_HTTP_HEADER_BYTES {
                return Err(ScopedHttpError::HeadersTooLarge);
            }
            checked_headers.push((name, value));
        }

        let body = body.map(|bytes| {
            if bytes.len() > MAX_SCOPED_HTTP_BODY_BYTES {
                Err(ScopedHttpError::BodyTooLarge)
            } else {
                Ok(bytes.into_boxed_slice())
            }
        });
        let body = body.transpose()?;

        Ok(Self {
            method,
            url,
            headers: checked_headers.into_boxed_slice(),
            body,
        })
    }

    /// Returns the validated HTTP method.
    #[must_use]
    pub fn method(&self) -> &str {
        &self.method
    }

    /// Returns the validated absolute HTTP(S) URL.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Returns the validated request headers.
    #[must_use]
    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }

    /// Returns the request body, when present.
    #[must_use]
    pub fn body(&self) -> Option<&[u8]> {
        self.body.as_deref()
    }
}

impl std::fmt::Debug for ScopedHttpRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ScopedHttpRequest")
            .field("method", &self.method)
            .field("header_count", &self.headers.len())
            .field(
                "body_bytes",
                &self.body.as_ref().map_or(0, |body| body.len()),
            )
            .finish_non_exhaustive()
    }
}

/// Bounded response returned by [`ScopedHttpProvider`].
#[derive(Clone, PartialEq, Eq)]
pub struct ScopedHttpResponse {
    status: u16,
    headers: Box<[(String, String)]>,
    body: Box<[u8]>,
}

impl ScopedHttpResponse {
    /// Returns the HTTP status code.
    #[must_use]
    pub const fn status(&self) -> u16 {
        self.status
    }

    /// Returns response headers in receive order.
    #[must_use]
    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }

    /// Returns the first response header matching `name`.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// Returns the response body bytes.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

impl std::fmt::Debug for ScopedHttpResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ScopedHttpResponse")
            .field("status", &self.status)
            .field("header_count", &self.headers.len())
            .field("body_bytes", &self.body.len())
            .finish_non_exhaustive()
    }
}

/// Failure from request validation, origin policy or the Moirai HTTP client.
#[derive(Debug)]
pub enum ScopedHttpError {
    /// No HTTP(S) origin was configured.
    NoOrigins,
    /// The configured origin count exceeds [`MAX_SCOPED_HTTP_ORIGINS`].
    TooManyOrigins,
    /// An origin is not a canonical HTTP(S) authority.
    InvalidOrigin,
    /// A URL is malformed, too long or outside the absolute HTTP(S) form.
    InvalidUrl,
    /// The method is empty, too long or contains a non-token byte.
    InvalidMethod,
    /// A header name or value is malformed.
    InvalidHeader,
    /// A transport-owned header was supplied by the caller.
    ForbiddenHeader,
    /// The request has too many headers.
    TooManyHeaders,
    /// Request headers exceed [`MAX_SCOPED_HTTP_HEADER_BYTES`].
    HeadersTooLarge,
    /// The request body exceeds [`MAX_SCOPED_HTTP_BODY_BYTES`].
    BodyTooLarge,
    /// The URL origin is not in the provider allowlist.
    OriginDenied,
    /// The request deadline is zero or exceeds [`MAX_SCOPED_HTTP_DEADLINE`].
    InvalidDeadline,
    /// The Moirai client rejected the request or transport operation.
    Transport(io::Error),
}

impl std::fmt::Display for ScopedHttpError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::NoOrigins => "scoped HTTP provider requires an origin allowlist",
            Self::TooManyOrigins => "scoped HTTP origin allowlist exceeds its bound",
            Self::InvalidOrigin => "scoped HTTP origin is not a canonical HTTP(S) authority",
            Self::InvalidUrl => "scoped HTTP URL is invalid or exceeds its bound",
            Self::InvalidMethod => "scoped HTTP method is not a bounded token",
            Self::InvalidHeader => "scoped HTTP header is invalid",
            Self::ForbiddenHeader => "scoped HTTP header is transport-owned",
            Self::TooManyHeaders => "scoped HTTP header count exceeds its bound",
            Self::HeadersTooLarge => "scoped HTTP headers exceed their byte bound",
            Self::BodyTooLarge => "scoped HTTP request body exceeds its byte bound",
            Self::OriginDenied => "scoped HTTP URL origin is outside the allowlist",
            Self::InvalidDeadline => "scoped HTTP request deadline is outside the provider bound",
            Self::Transport(_) => "scoped HTTP transport failed",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ScopedHttpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Transport(error) => Some(error),
            _ => None,
        }
    }
}

/// Native HTTP provider with a capability witness and origin allowlist.
///
/// Redirects are deliberately disabled. A caller must authorize every
/// destination explicitly by constructing a new request whose origin is in
/// this provider's allowlist; this prevents a server response from expanding
/// the host's network authority.
pub struct ScopedHttpProvider {
    client: HttpClient,
    allowed_origins: Box<[HostOrigin]>,
}

impl std::fmt::Debug for ScopedHttpProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ScopedHttpProvider")
            .field("allowed_origin_count", &self.allowed_origins.len())
            .finish_non_exhaustive()
    }
}

impl ScopedHttpProvider {
    /// Creates a provider with a non-empty HTTP(S) origin allowlist.
    ///
    /// The allowlist is trusted host configuration. Browser or frontend input
    /// must be validated into a [`ScopedHttpRequest`] and cannot add origins.
    ///
    /// # Errors
    /// Returns a typed error for an empty, oversized or non-HTTP origin list.
    pub fn new<I, S>(origins: I) -> Result<Self, ScopedHttpError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut allowed_origins = Vec::new();
        for origin in origins {
            if allowed_origins.len() >= MAX_SCOPED_HTTP_ORIGINS {
                return Err(ScopedHttpError::TooManyOrigins);
            }
            let origin =
                HostOrigin::parse(origin.as_ref()).map_err(|_| ScopedHttpError::InvalidOrigin)?;
            if !origin.as_str().starts_with("http://") && !origin.as_str().starts_with("https://") {
                return Err(ScopedHttpError::InvalidOrigin);
            }
            if !allowed_origins.contains(&origin) {
                allowed_origins.push(origin);
            }
        }
        if allowed_origins.is_empty() {
            return Err(ScopedHttpError::NoOrigins);
        }

        let mut client = HttpClient::new();
        client.set_max_redirects(0);
        client.set_max_response_bytes(MAX_SCOPED_HTTP_RESPONSE_BYTES);
        Ok(Self {
            client,
            allowed_origins: allowed_origins.into_boxed_slice(),
        })
    }

    /// Returns the configured HTTP(S) origins.
    #[must_use]
    pub fn allowed_origins(&self) -> &[HostOrigin] {
        &self.allowed_origins
    }

    /// Performs one bounded request through the Moirai HTTP client.
    ///
    /// The capability witness is required even though the request is already
    /// value-validated; this keeps network authority at the host boundary and
    /// prevents a frontend from calling the provider directly.
    ///
    /// # Errors
    /// Returns [`ScopedHttpError::OriginDenied`] for a destination outside the
    /// allowlist, validation errors from the request, or a transport error.
    pub async fn request(
        &self,
        capability: &VerifiedHostCapability<{ CapabilityScope::NETWORK.0 }>,
        request: ScopedHttpRequest,
    ) -> Result<ScopedHttpResponse, ScopedHttpError> {
        self.request_with_deadline(capability, request, MAX_SCOPED_HTTP_DEADLINE)
            .await
    }

    /// Performs one request with an explicit finite logical deadline.
    ///
    /// Dropping the returned future cancels the in-flight transport. An
    /// expired request drops the transport future as well, so no background
    /// connection task survives the deadline.
    ///
    /// # Errors
    /// Returns a validation, origin-policy or transport error. A deadline
    /// expiration is reported as [`io::ErrorKind::TimedOut`].
    pub async fn request_with_deadline(
        &self,
        _capability: &VerifiedHostCapability<{ CapabilityScope::NETWORK.0 }>,
        request: ScopedHttpRequest,
        deadline: Duration,
    ) -> Result<ScopedHttpResponse, ScopedHttpError> {
        if deadline.is_zero() || deadline > MAX_SCOPED_HTTP_DEADLINE {
            return Err(ScopedHttpError::InvalidDeadline);
        }
        let origin = request_origin(request.url())?;
        if !self
            .allowed_origins
            .iter()
            .any(|allowed| allowed == &origin)
        {
            return Err(ScopedHttpError::OriginDenied);
        }
        let headers = request
            .headers()
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect::<Vec<_>>();
        let response = timeout(
            deadline,
            self.client
                .request(request.method(), request.url(), &headers, request.body()),
        )
        .await
        .map_err(|_| {
            ScopedHttpError::Transport(io::Error::new(
                io::ErrorKind::TimedOut,
                "scoped HTTP request deadline elapsed",
            ))
        })?
        .map_err(ScopedHttpError::Transport)?;
        Ok(response.into())
    }
}

impl From<Response> for ScopedHttpResponse {
    fn from(response: Response) -> Self {
        Self {
            status: response.status,
            headers: response.headers.into_boxed_slice(),
            body: response.body.into_boxed_slice(),
        }
    }
}

fn validate_method(method: &str) -> Result<(), ScopedHttpError> {
    if method.is_empty()
        || method.len() > MAX_SCOPED_HTTP_METHOD_BYTES
        || !method.bytes().all(is_token_byte)
    {
        return Err(ScopedHttpError::InvalidMethod);
    }
    Ok(())
}

fn validate_header(name: &str, value: &str) -> Result<(), ScopedHttpError> {
    if name.is_empty() || !name.bytes().all(is_token_byte) {
        return Err(ScopedHttpError::InvalidHeader);
    }
    if value
        .bytes()
        .any(|byte| byte < 0x20 && byte != b'\t' || byte == 0x7f)
    {
        return Err(ScopedHttpError::InvalidHeader);
    }
    if FORBIDDEN_HEADERS
        .iter()
        .any(|forbidden| forbidden.eq_ignore_ascii_case(name))
    {
        return Err(ScopedHttpError::ForbiddenHeader);
    }
    Ok(())
}

fn is_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

fn validate_url(url: &str) -> Result<(), ScopedHttpError> {
    if url.is_empty()
        || url.len() > MAX_SCOPED_HTTP_URL_BYTES
        || url
            .bytes()
            .any(|byte| byte < 0x20 || byte == 0x7f || byte == b' ' || byte == b'#')
    {
        return Err(ScopedHttpError::InvalidUrl);
    }
    let _ = request_origin(url)?;
    Ok(())
}

fn request_origin(url: &str) -> Result<HostOrigin, ScopedHttpError> {
    let Some(scheme_end) = url.find("://") else {
        return Err(ScopedHttpError::InvalidUrl);
    };
    let authority_start = scheme_end
        .checked_add(3)
        .ok_or(ScopedHttpError::InvalidUrl)?;
    let remainder = url
        .get(authority_start..)
        .ok_or(ScopedHttpError::InvalidUrl)?;
    let authority_end = remainder.find(['/', '?']).unwrap_or(remainder.len());
    if authority_end == 0 {
        return Err(ScopedHttpError::InvalidUrl);
    }
    let origin_text = url
        .get(..authority_start + authority_end)
        .ok_or(ScopedHttpError::InvalidUrl)?;
    let origin = HostOrigin::parse(origin_text).map_err(|_| ScopedHttpError::InvalidOrigin)?;
    if !origin.as_str().starts_with("http://") && !origin.as_str().starts_with("https://") {
        return Err(ScopedHttpError::InvalidUrl);
    }
    Ok(origin)
}

#[cfg(test)]
mod tests;
