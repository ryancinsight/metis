//! Closed command descriptors and bounded capability catalog payloads.

use super::wire::{MAX_PAYLOAD_SIZE, MessageType, finish, malformed, take};
use crate::error::{ErrorCode, MetisError, Result};

/// Maximum number of command descriptors in one capability catalog.
pub const MAX_COMMANDS: usize = 16;

/// Stable metadata for one typed request/response command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandDescriptor {
    request: MessageType,
    response: MessageType,
    name: &'static str,
}

impl CommandDescriptor {
    /// Creates metadata for a request and its successful response.
    #[must_use]
    const fn new(request: MessageType, response: MessageType, name: &'static str) -> Self {
        Self {
            request,
            response,
            name,
        }
    }

    /// Returns the request message identifier.
    #[must_use]
    pub const fn request(self) -> MessageType {
        self.request
    }

    /// Returns the successful response message identifier.
    #[must_use]
    pub const fn response(self) -> MessageType {
        self.response
    }

    /// Returns the stable human-readable command name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }
}

impl MessageType {
    /// Returns the descriptor for a request message.
    #[must_use]
    pub const fn descriptor(self) -> Option<CommandDescriptor> {
        match self {
            Self::HandshakeReq => Some(CommandDescriptor::new(
                Self::HandshakeReq,
                Self::HandshakeResp,
                "session.handshake",
            )),
            Self::CapabilityReq => Some(CommandDescriptor::new(
                Self::CapabilityReq,
                Self::CapabilityResp,
                "host.capabilities",
            )),
            Self::TargetCapabilityReq => Some(CommandDescriptor::new(
                Self::TargetCapabilityReq,
                Self::TargetCapabilityResp,
                "host.target_capabilities",
            )),
            Self::HeartbeatReq => Some(CommandDescriptor::new(
                Self::HeartbeatReq,
                Self::HeartbeatResp,
                "session.heartbeat",
            )),
            Self::ClinicalCalcReq => Some(CommandDescriptor::new(
                Self::ClinicalCalcReq,
                Self::ClinicalCalcResp,
                "clinical.calculate",
            )),
            Self::PluginInvokeReq => Some(CommandDescriptor::new(
                Self::PluginInvokeReq,
                Self::PluginInvokeResp,
                "plugin.invoke",
            )),
            Self::AuditQueryReq => Some(CommandDescriptor::new(
                Self::AuditQueryReq,
                Self::AuditQueryResp,
                "audit.query",
            )),
            Self::HandshakeResp
            | Self::CapabilityResp
            | Self::TargetCapabilityResp
            | Self::HeartbeatResp
            | Self::ClinicalCalcResp
            | Self::AuditQueryResp
            | Self::TelemetryStreamEvent
            | Self::PluginInvokeResp
            | Self::ErrorResp => None,
        }
    }

    /// Returns whether this identifier selects a request command.
    #[must_use]
    pub const fn is_request(self) -> bool {
        self.descriptor().is_some()
    }
}

/// Commands implemented by the current backend service after handshake.
pub const SUPPORTED_COMMANDS: &[MessageType] = &[
    MessageType::CapabilityReq,
    MessageType::TargetCapabilityReq,
    MessageType::HeartbeatReq,
    MessageType::ClinicalCalcReq,
    MessageType::PluginInvokeReq,
];

/// Version and bounded command identifiers advertised by a host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityCatalogPayload {
    /// Wire protocol version used by the advertised commands.
    protocol_version: u16,
    /// Request identifiers accepted by the connected host.
    commands: Vec<MessageType>,
}

impl CapabilityCatalogPayload {
    /// Builds a catalog after validating every command descriptor.
    ///
    /// # Errors
    /// Returns a protocol error when the list is over [`MAX_COMMANDS`], contains
    /// a response-only identifier, or contains a duplicate command.
    pub fn new(requests: impl IntoIterator<Item = MessageType>) -> Result<Self> {
        let mut commands = Vec::with_capacity(MAX_COMMANDS);
        for command in requests {
            if commands.len() == MAX_COMMANDS {
                return Err(MetisError::protocol(
                    ErrorCode::PayloadTooLarge,
                    "Capability catalog exceeds the command count bound",
                ));
            }
            commands.push(command);
        }
        validate_commands(&commands)?;
        Ok(Self {
            protocol_version: super::wire::PROTOCOL_VERSION,
            commands,
        })
    }

    /// Returns whether the catalog advertises a request command.
    #[must_use]
    pub fn supports(&self, request: MessageType) -> bool {
        self.commands.contains(&request)
    }

    /// Returns the catalog's advertised wire protocol version.
    #[must_use]
    pub const fn protocol_version(&self) -> u16 {
        self.protocol_version
    }

    /// Returns the validated request identifiers in wire order.
    #[must_use]
    pub fn commands(&self) -> &[MessageType] {
        &self.commands
    }

    /// Encodes the bounded catalog payload.
    ///
    /// # Errors
    /// Returns a protocol error when the command list violates the catalog
    /// bound or descriptor contract.
    pub fn encode(&self) -> Result<Vec<u8>> {
        validate_commands(&self.commands)?;
        let command_count = u16::try_from(self.commands.len()).map_err(|_| {
            MetisError::protocol(
                ErrorCode::PayloadTooLarge,
                "Capability catalog command count exceeds its wire field",
            )
        })?;
        let payload_len = 4usize
            .checked_add(self.commands.len().checked_mul(2).ok_or_else(|| {
                MetisError::protocol(
                    ErrorCode::PayloadTooLarge,
                    "Capability catalog size overflows its resource bound",
                )
            })?)
            .ok_or_else(|| {
                MetisError::protocol(
                    ErrorCode::PayloadTooLarge,
                    "Capability catalog size overflows its resource bound",
                )
            })?;
        if payload_len > MAX_PAYLOAD_SIZE {
            return Err(MetisError::protocol(
                ErrorCode::PayloadTooLarge,
                "Capability catalog exceeds the payload resource bound",
            ));
        }
        let mut payload = Vec::with_capacity(payload_len);
        payload.extend_from_slice(&self.protocol_version.to_be_bytes());
        payload.extend_from_slice(&command_count.to_be_bytes());
        for command in &self.commands {
            payload.extend_from_slice(&(*command as u16).to_be_bytes());
        }
        Ok(payload)
    }

    /// Decodes and validates a catalog from untrusted wire bytes.
    ///
    /// # Errors
    /// Rejects truncation, trailing bytes, unknown or response-only identifiers,
    /// duplicates, and catalogs over the bounded command count.
    pub fn decode(mut payload: &[u8]) -> Result<Self> {
        if payload.len() > MAX_PAYLOAD_SIZE {
            return Err(MetisError::protocol(
                ErrorCode::PayloadTooLarge,
                "Capability catalog exceeds the payload resource bound",
            ));
        }
        let protocol_version = u16::from_be_bytes(take(&mut payload)?);
        let command_count = usize::from(u16::from_be_bytes(take(&mut payload)?));
        if command_count > MAX_COMMANDS {
            return Err(MetisError::protocol(
                ErrorCode::PayloadTooLarge,
                "Capability catalog exceeds the command count bound",
            ));
        }
        let mut commands = Vec::with_capacity(command_count);
        for _ in 0..command_count {
            let raw = u16::from_be_bytes(take(&mut payload)?);
            let command = MessageType::from_wire(raw).ok_or_else(|| {
                MetisError::protocol(
                    ErrorCode::UnexpectedMessageType,
                    "Capability catalog contains an unknown command identifier",
                )
            })?;
            commands.push(command);
        }
        finish(payload)?;
        validate_commands(&commands)?;
        Ok(Self {
            protocol_version,
            commands,
        })
    }
}

fn validate_commands(commands: &[MessageType]) -> Result<()> {
    if commands.len() > MAX_COMMANDS {
        return Err(MetisError::protocol(
            ErrorCode::PayloadTooLarge,
            "Capability catalog exceeds the command count bound",
        ));
    }
    for (index, command) in commands.iter().enumerate() {
        if !command.is_request() {
            return Err(MetisError::protocol(
                ErrorCode::UnexpectedMessageType,
                "Capability catalog contains a response-only message",
            ));
        }
        if commands[..index].contains(command) {
            return Err(malformed("Capability catalog contains a duplicate command"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptors_pair_each_request_with_its_response() {
        for command in [
            MessageType::HandshakeReq,
            MessageType::CapabilityReq,
            MessageType::TargetCapabilityReq,
            MessageType::HeartbeatReq,
            MessageType::ClinicalCalcReq,
            MessageType::PluginInvokeReq,
            MessageType::AuditQueryReq,
        ] {
            let descriptor = command.descriptor().expect("request descriptor");
            assert_eq!(descriptor.request(), command);
            assert_eq!(
                descriptor.response(),
                command.response_type().expect("response")
            );
            assert!(!descriptor.name().is_empty());
        }
        assert!(MessageType::ErrorResp.descriptor().is_none());
    }

    #[test]
    fn catalog_round_trip_preserves_commands_and_version() {
        let catalog =
            CapabilityCatalogPayload::new(SUPPORTED_COMMANDS.iter().copied()).expect("catalog");
        let encoded = catalog.encode().expect("encoded catalog");
        let decoded = CapabilityCatalogPayload::decode(&encoded).expect("decoded catalog");
        assert_eq!(decoded, catalog);
        assert!(decoded.supports(MessageType::ClinicalCalcReq));
        assert!(decoded.supports(MessageType::PluginInvokeReq));
        assert!(!decoded.supports(MessageType::AuditQueryReq));
    }

    #[test]
    fn catalog_rejects_unknown_response_and_duplicate_entries() {
        for encoded in [
            [0x01, 0x00, 0x00, 0x01, 0x00, 0xff].as_slice(),
            [0x01, 0x00, 0x00, 0x02, 0x00, 0x03, 0x00, 0x03].as_slice(),
            [0x01, 0x00, 0x00, 0x01, 0x7f, 0xff].as_slice(),
        ] {
            let error = CapabilityCatalogPayload::decode(encoded).expect_err("invalid catalog");
            assert!(matches!(
                error.code,
                ErrorCode::UnexpectedMessageType | ErrorCode::MalformedPayload
            ));
        }
    }

    #[test]
    fn catalog_rejects_more_than_the_bounded_command_count() {
        let commands = std::iter::repeat_n(MessageType::HeartbeatReq, MAX_COMMANDS + 1);
        let error = CapabilityCatalogPayload::new(commands).expect_err("catalog bound");
        assert_eq!(error.code, ErrorCode::PayloadTooLarge);
    }
}
