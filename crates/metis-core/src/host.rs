//! Host-origin, window, and session binding for privileged commands.
//!
//! A browser or `WebView` is untrusted content. The host must observe its origin
//! and window identity at the boundary, validate them here, and then pair that
//! context with a backend-issued capability before dispatching a command.

use crate::capability::{CapabilityGrantSpec, CapabilityScope, CapabilityToken};
use crate::error::{ErrorCode, MetisError, Result};
use moirai_crypto::sha256;
use std::fmt;
use std::num::NonZeroU64;
use std::str::FromStr;

const MAX_ORIGIN_BYTES: usize = 256;
const NATIVE_ORIGIN: &str = "metis://native";
const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'";
const HOST_BINDING_DOMAIN: &[u8; 8] = b"METIS-H1";
const NATIVE_WINDOW_ID: WindowId = WindowId(NonZeroU64::MIN);

/// A canonical origin accepted by a Metis host boundary.
///
/// HTTP, HTTPS, `metis`, and `tauri` schemes are accepted. Credentials, paths,
/// wildcards, opaque origins, and non-network schemes are rejected so a host
/// policy cannot accidentally treat a file or data URL as an application.
#[repr(transparent)]
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HostOrigin(String);

impl HostOrigin {
    /// Parses and canonicalizes a host origin.
    ///
    /// Scheme and host letters are lowercased. The result contains only the
    /// scheme and authority; a path, query, or fragment is never accepted.
    ///
    /// # Errors
    /// Returns [`ErrorCode::InvalidOrigin`] for an empty, oversized, malformed,
    /// or unsupported origin.
    pub fn parse(value: &str) -> Result<Self> {
        if value.is_empty() || value.len() > MAX_ORIGIN_BYTES || value.trim() != value {
            return Err(invalid_origin());
        }
        if !value.is_ascii() {
            return Err(invalid_origin());
        }
        let Some((scheme, authority)) = value.split_once("://") else {
            return Err(invalid_origin());
        };
        let scheme = scheme.to_ascii_lowercase();
        if !matches!(scheme.as_str(), "http" | "https" | "metis" | "tauri") {
            return Err(invalid_origin());
        }
        let authority = canonical_authority(authority)?;
        Ok(Self(format!("{scheme}://{authority}")))
    }

    /// Returns the host-controlled native application origin.
    #[must_use]
    pub fn native() -> Self {
        Self(NATIVE_ORIGIN.to_owned())
    }

    /// Returns the canonical serialized origin.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for HostOrigin {
    type Error = MetisError;

    fn try_from(value: &str) -> Result<Self> {
        Self::parse(value)
    }
}

impl TryFrom<String> for HostOrigin {
    type Error = MetisError;

    fn try_from(value: String) -> Result<Self> {
        Self::parse(&value)
    }
}

impl FromStr for HostOrigin {
    type Err = MetisError;

    fn from_str(value: &str) -> Result<Self> {
        Self::parse(value)
    }
}

impl fmt::Debug for HostOrigin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("HostOrigin").field(&self.0).finish()
    }
}

impl fmt::Display for HostOrigin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Non-zero host window identity used to bind a command grant.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId(NonZeroU64);

impl WindowId {
    /// Constructs a window identity.
    ///
    /// # Errors
    /// Returns [`ErrorCode::InvalidWindow`] for zero, which is reserved as the
    /// absence of a host window.
    pub fn new(value: u64) -> Result<Self> {
        NonZeroU64::new(value).map(Self).ok_or_else(|| {
            MetisError::capability(
                ErrorCode::InvalidWindow,
                "Host window identity must be nonzero",
            )
        })
    }

    /// Returns the numeric window identity.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Non-zero session principal bound to a host context.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HostSessionId([u8; 16]);

impl HostSessionId {
    /// Constructs a session identity from its fixed-width principal label.
    ///
    /// # Errors
    /// Returns [`ErrorCode::InvalidPrincipal`] for the all-zero label.
    pub fn new(value: [u8; 16]) -> Result<Self> {
        if value == [0; 16] {
            return Err(MetisError::capability(
                ErrorCode::InvalidPrincipal,
                "Host session identity must be nonzero",
            ));
        }
        Ok(Self(value))
    }

    /// Returns the fixed-width principal label.
    #[must_use]
    pub const fn as_bytes(self) -> [u8; 16] {
        self.0
    }
}

/// Host identity observed at a transport boundary.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HostContext {
    origin: HostOrigin,
    window_id: WindowId,
    session_id: HostSessionId,
}

impl HostContext {
    /// Binds an origin and window to a validated session identity.
    #[must_use]
    pub fn new(origin: HostOrigin, window_id: WindowId, session_id: HostSessionId) -> Self {
        Self {
            origin,
            window_id,
            session_id,
        }
    }

    /// Returns the observed origin.
    #[must_use]
    pub const fn origin(&self) -> &HostOrigin {
        &self.origin
    }

    /// Returns the observed window identity.
    #[must_use]
    pub const fn window_id(&self) -> WindowId {
        self.window_id
    }

    /// Returns the session identity bound to this host context.
    #[must_use]
    pub const fn session_id(&self) -> HostSessionId {
        self.session_id
    }

    /// Issues a capability whose signature is bound to this host context.
    ///
    /// The context is authenticated as associated data and is reconstructed by
    /// the trusted host during verification. It is not copied into the token,
    /// so the wire format remains fixed-width and carries no browser-controlled
    /// authority fields.
    ///
    /// # Errors
    /// Returns [`ErrorCode::InvalidPrincipal`] when the token principal does
    /// not equal this context's session identity.
    pub fn issue_capability(
        &self,
        spec: CapabilityGrantSpec,
        backend_master_key: &[u8],
    ) -> Result<CapabilityToken> {
        if spec.principal_id != self.session_id.as_bytes() {
            return Err(MetisError::capability(
                ErrorCode::InvalidPrincipal,
                "Capability principal is not bound to the host session",
            ));
        }
        let mut token = CapabilityToken::issue(
            spec.token_id,
            spec.principal_id,
            spec.scope,
            spec.issued_at_secs,
            spec.duration_secs,
            spec.nonce,
            backend_master_key,
        );
        token.sign_for_host(backend_master_key, self);
        Ok(token)
    }

    /// Returns the fixed-width context binding authenticated with a grant.
    #[must_use]
    pub(crate) fn capability_binding(&self) -> [u8; 48] {
        let origin_digest = sha256(self.origin.as_str().as_bytes());
        let mut binding = [0u8; 48];
        binding[..HOST_BINDING_DOMAIN.len()].copy_from_slice(HOST_BINDING_DOMAIN);
        binding[HOST_BINDING_DOMAIN.len()..HOST_BINDING_DOMAIN.len() + origin_digest.len()]
            .copy_from_slice(&origin_digest);
        binding[40..].copy_from_slice(&self.window_id.get().to_be_bytes());
        binding
    }
}

/// Exact host policy for one privileged connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostPolicy {
    allowed_origin: HostOrigin,
    allowed_window: WindowId,
}

impl HostPolicy {
    /// Constructs a deny-by-default policy for one origin and window.
    #[must_use]
    pub const fn new(allowed_origin: HostOrigin, allowed_window: WindowId) -> Self {
        Self {
            allowed_origin,
            allowed_window,
        }
    }

    /// Constructs the policy used by the contained native demonstration.
    #[must_use]
    pub fn native() -> Self {
        Self::new(HostOrigin::native(), NATIVE_WINDOW_ID)
    }

    /// Returns the exact origin admitted by this policy.
    #[must_use]
    pub const fn allowed_origin(&self) -> &HostOrigin {
        &self.allowed_origin
    }

    /// Returns the exact window admitted by this policy.
    #[must_use]
    pub const fn allowed_window(&self) -> WindowId {
        self.allowed_window
    }

    /// Returns the strict document policy for a browser or `WebView` shell.
    #[must_use]
    pub const fn content_security_policy(&self) -> &'static str {
        CONTENT_SECURITY_POLICY
    }

    /// Creates the context a trusted host observed for a session.
    #[must_use]
    pub fn context_for(&self, session_id: HostSessionId) -> HostContext {
        HostContext::new(self.allowed_origin.clone(), self.allowed_window, session_id)
    }

    /// Checks that a host context matches the exact origin and window policy.
    ///
    /// This method must receive origin and window values observed by the host
    /// boundary. Values copied from an untrusted message are not observations.
    ///
    /// # Errors
    /// Returns [`ErrorCode::NavigationDenied`] for an origin mismatch or
    /// [`ErrorCode::InvalidWindow`] for a window mismatch.
    pub fn check_context(&self, context: &HostContext) -> Result<()> {
        if context.origin != self.allowed_origin {
            return Err(MetisError::capability(
                ErrorCode::NavigationDenied,
                "Host origin is outside the connection policy",
            ));
        }
        if context.window_id != self.allowed_window {
            return Err(MetisError::capability(
                ErrorCode::InvalidWindow,
                "Host window is outside the connection policy",
            ));
        }
        Ok(())
    }

    /// Authorizes one capability token for the observed host context.
    ///
    /// The returned witness carries the exact token, origin, window, and
    /// session relationship that was checked. It does not extend token expiry.
    ///
    /// # Errors
    /// Rejects a mismatched origin/window/session, invalid token signature,
    /// expired token, or missing command scope.
    pub fn authorize<const SCOPE: u32>(
        &self,
        token: &CapabilityToken,
        context: &HostContext,
        current_time_secs: u64,
        backend_master_key: &[u8],
    ) -> Result<VerifiedHostCapability<SCOPE>> {
        self.check_context(context)?;
        if token.principal_id != context.session_id.as_bytes() {
            return Err(MetisError::capability(
                ErrorCode::InvalidPrincipal,
                "Capability principal is not bound to the host session",
            ));
        }
        token.verify_for_host(
            CapabilityScope(SCOPE),
            current_time_secs,
            backend_master_key,
            context,
        )?;
        Ok(VerifiedHostCapability {
            token_id: token.token_id,
            context: context.clone(),
        })
    }
}

/// Witness that a command capability passed host, window, and session checks.
#[must_use = "retain the witness until the authorized operation completes"]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedHostCapability<const SCOPE: u32> {
    token_id: u64,
    context: HostContext,
}

impl<const SCOPE: u32> VerifiedHostCapability<SCOPE> {
    /// Returns the verified token identity.
    #[must_use]
    pub const fn token_id(&self) -> u64 {
        self.token_id
    }

    /// Returns the host context covered by this witness.
    #[must_use]
    pub const fn context(&self) -> &HostContext {
        &self.context
    }
}

fn invalid_origin() -> MetisError {
    MetisError::capability(
        ErrorCode::InvalidOrigin,
        "Origin must be an allowed ASCII network origin without credentials or a path",
    )
}

fn canonical_authority(authority: &str) -> Result<String> {
    if authority.is_empty()
        || authority.len() > MAX_ORIGIN_BYTES
        || authority
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
        || authority
            .bytes()
            .any(|byte| matches!(byte, b'/' | b'?' | b'#' | b'@' | b'*'))
    {
        return Err(invalid_origin());
    }

    if authority.starts_with('[') {
        let Some(close) = authority.find(']') else {
            return Err(invalid_origin());
        };
        let host = &authority[1..close];
        if host.is_empty()
            || !host.contains(':')
            || host
                .bytes()
                .any(|byte| !(byte.is_ascii_hexdigit() || matches!(byte, b':' | b'.')))
        {
            return Err(invalid_origin());
        }
        let suffix = &authority[close + 1..];
        if !suffix.is_empty() {
            let Some(port) = suffix.strip_prefix(':') else {
                return Err(invalid_origin());
            };
            validate_port(port)?;
        }
    } else {
        let mut parts = authority.split(':');
        let Some(host) = parts.next() else {
            return Err(invalid_origin());
        };
        let port = parts.next();
        if parts.next().is_some() || host.is_empty() || host.starts_with('.') || host.ends_with('.')
        {
            return Err(invalid_origin());
        }
        if host.contains("..")
            || host
                .bytes()
                .any(|byte| !(byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_')))
        {
            return Err(invalid_origin());
        }
        if let Some(port) = port {
            validate_port(port)?;
        }
    }

    Ok(authority.to_ascii_lowercase())
}

fn validate_port(port: &str) -> Result<()> {
    let Ok(port) = port.parse::<u16>() else {
        return Err(invalid_origin());
    };
    if port == 0 {
        return Err(invalid_origin());
    }
    Ok(())
}
