//! Stable error classifications and requirement identifiers for Metis.

use std::fmt;

/// Discriminant error codes classified by subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum ErrorCode {
    // Protocol / Framing Errors (0x1000 - 0x1FFF)
    /// Frame magic does not identify Metis.
    MagicMismatch = 0x1001,
    /// Peer protocol version is unsupported.
    VersionMismatch = 0x1002,
    /// Stored checksum differs from computed bytes.
    ChecksumMismatch = 0x1003,
    /// Payload exceeds the wire limit.
    PayloadTooLarge = 0x1004,
    /// End of stream interrupts a frame.
    FrameTruncated = 0x1005,
    /// Message type is not valid in this operation.
    UnexpectedMessageType = 0x1006,
    /// Payload fails its canonical encoding contract.
    MalformedPayload = 0x1007,
    /// A response does not match its outstanding request.
    SequenceMismatch = 0x1008,
    /// A request repeats or precedes the last accepted sequence.
    ReplayDetected = 0x1009,

    // Capability / Security Errors (0x2000 - 0x2FFF)
    /// Operation has no session capability.
    MissingCapability = 0x2001,
    /// Capability authentication fails.
    InvalidCapabilitySignature = 0x2002,
    /// Capability is outside its validity interval.
    CapabilityExpired = 0x2003,
    /// Capability does not authorize the operation.
    InsufficientScope = 0x2004,
    /// Operation attempts an unauthorized authority change.
    PrivilegeEscalationAttempt = 0x2005,
    /// Claimed principal does not match the session.
    InvalidPrincipal = 0x2006,

    // Clinical / Safety Interlock Errors (0x3000 - 0x3FFF)
    /// Weight is outside the demonstration input bounds.
    InvalidPatientWeight = 0x3001,
    /// Concentration is outside the demonstration input bounds.
    InvalidDrugConcentration = 0x3002,
    /// Dose is outside the demonstration input bounds.
    InvalidTargetDose = 0x3003,
    /// Rate exceeds the configured adult demonstration limit.
    RateExceedsSafetyEnvelope = 0x3004,
    /// Rate exceeds the configured pediatric demonstration limit.
    PediatricRateExceeded = 0x3005,
    /// Numeric input or output is not finite.
    NumericInstability = 0x3006,
    /// Configuration or operation violates an interlock.
    ClinicalInterlockBlocked = 0x3007,

    // IPC / Transport Errors (0x4000 - 0x4FFF)
    /// Transport fails before orderly end of stream.
    TransportBroken = 0x4001,
    /// Peer closes an otherwise complete stream.
    ConnectionClosed = 0x4002,
    /// Operation exceeds its deadline.
    Timeout = 0x4003,
    /// Operating system IO operation fails.
    IoError = 0x4004,
    /// The bounded transport has no remaining queue capacity.
    QueueFull = 0x4005,

    // UI / Layout Errors (0x5000 - 0x5FFF)
    /// Markup violates the supported grammar.
    MalformedMarkup = 0x5001,
    /// Markup element has no matching close tag.
    UnclosedTag = 0x5002,
    /// Opening and closing tag names differ.
    TagMismatch = 0x5003,
    /// Style declaration is unsupported or malformed.
    InvalidCssStyle = 0x5004,
    /// Layout exceeds representable bounds.
    LayoutOverflow = 0x5005,

    // Platform / Rendering Errors (0x6000 - 0x6FFF)
    /// Rendering cannot complete.
    RenderFailure = 0x6001,
    /// Surface dimensions or memory exceed supported bounds.
    SurfaceAllocationError = 0x6002,
    /// Event cannot be represented by this platform.
    UnsupportedPlatformEvent = 0x6003,
}

impl ErrorCode {
    /// Returns the symbolic string for the error code.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::MagicMismatch => "ERR_MAGIC_MISMATCH",
            Self::VersionMismatch => "ERR_VERSION_MISMATCH",
            Self::ChecksumMismatch => "ERR_CHECKSUM_MISMATCH",
            Self::PayloadTooLarge => "ERR_PAYLOAD_TOO_LARGE",
            Self::FrameTruncated => "ERR_FRAME_TRUNCATED",
            Self::UnexpectedMessageType => "ERR_UNEXPECTED_MESSAGE_TYPE",
            Self::MalformedPayload => "ERR_MALFORMED_PAYLOAD",
            Self::SequenceMismatch => "ERR_SEQUENCE_MISMATCH",
            Self::ReplayDetected => "ERR_REPLAY_DETECTED",
            Self::MissingCapability => "ERR_MISSING_CAPABILITY",
            Self::InvalidCapabilitySignature => "ERR_INVALID_CAPABILITY_SIGNATURE",
            Self::CapabilityExpired => "ERR_CAPABILITY_EXPIRED",
            Self::InsufficientScope => "ERR_INSUFFICIENT_SCOPE",
            Self::PrivilegeEscalationAttempt => "ERR_PRIVILEGE_ESCALATION_ATTEMPT",
            Self::InvalidPrincipal => "ERR_INVALID_PRINCIPAL",
            Self::InvalidPatientWeight => "ERR_INVALID_PATIENT_WEIGHT",
            Self::InvalidDrugConcentration => "ERR_INVALID_DRUG_CONCENTRATION",
            Self::InvalidTargetDose => "ERR_INVALID_TARGET_DOSE",
            Self::RateExceedsSafetyEnvelope => "ERR_RATE_EXCEEDS_SAFETY_ENVELOPE",
            Self::PediatricRateExceeded => "ERR_PEDIATRIC_RATE_EXCEEDED",
            Self::NumericInstability => "ERR_NUMERIC_INSTABILITY",
            Self::ClinicalInterlockBlocked => "ERR_CLINICAL_INTERLOCK_BLOCKED",
            Self::TransportBroken => "ERR_TRANSPORT_BROKEN",
            Self::ConnectionClosed => "ERR_CONNECTION_CLOSED",
            Self::Timeout => "ERR_TIMEOUT",
            Self::IoError => "ERR_IO_ERROR",
            Self::QueueFull => "ERR_QUEUE_FULL",
            Self::MalformedMarkup => "ERR_MALFORMED_MARKUP",
            Self::UnclosedTag => "ERR_UNCLOSED_TAG",
            Self::TagMismatch => "ERR_TAG_MISMATCH",
            Self::InvalidCssStyle => "ERR_INVALID_CSS_STYLE",
            Self::LayoutOverflow => "ERR_LAYOUT_OVERFLOW",
            Self::RenderFailure => "ERR_RENDER_FAILURE",
            Self::SurfaceAllocationError => "ERR_SURFACE_ALLOCATION_ERROR",
            Self::UnsupportedPlatformEvent => "ERR_UNSUPPORTED_PLATFORM_EVENT",
        }
    }
}

/// The core error type for Metis medical device architecture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetisError {
    /// Error category code.
    pub code: ErrorCode,
    /// Detailed diagnostic message.
    pub message: String,
    /// Traceability reference (e.g. IEC 62304 safety requirement tag).
    pub trace_id: &'static str,
}

impl MetisError {
    /// Constructs a new `MetisError` with explicit traceability reference.
    pub fn new(code: ErrorCode, message: impl Into<String>, trace_id: &'static str) -> Self {
        Self {
            code,
            message: message.into(),
            trace_id,
        }
    }

    /// Convenience helper for protocol errors.
    pub fn protocol(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::new(code, message, "REQ-METIS-IPC-001")
    }

    /// Convenience helper for capability/security errors.
    pub fn capability(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::new(code, message, "REQ-METIS-SEC-002")
    }

    /// Convenience helper for clinical safety errors.
    pub fn clinical(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::new(code, message, "REQ-METIS-CLIN-003")
    }

    /// Convenience helper for UI errors.
    pub fn ui(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::new(code, message, "REQ-METIS-UI-004")
    }

    /// Convenience helper for transport/IO errors.
    pub fn transport(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::new(code, message, "REQ-METIS-IPC-005")
    }
}

impl fmt::Display for MetisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}:0x{:04X} | {}] {}",
            self.code.as_str(),
            self.code as u16,
            self.trace_id,
            self.message
        )
    }
}

impl std::error::Error for MetisError {}

impl From<std::io::Error> for MetisError {
    fn from(err: std::io::Error) -> Self {
        Self::new(ErrorCode::IoError, err.to_string(), "REQ-METIS-IPC-005")
    }
}

/// Type alias for Results carrying `MetisError`.
pub type Result<T, E = MetisError> = std::result::Result<T, E>;
