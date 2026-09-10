use super::*;
use crate::UiFragmentPlugin;
use metis_core::capability::CapabilityScope;
use metis_core::crc32;
use metis_core::protocol::{
    CapabilityCatalogPayload, FragmentAction, FragmentPatchSet, FrameHeader,
    HandshakeRequestPayload, HandshakeResponsePayload, MessageType, PROTOCOL_VERSION,
    PluginDescriptor, PluginInvocationPayload, PluginInvocationResponsePayload, PluginOperation,
    TargetCapability, TargetCapabilityPayload, TargetPlatform,
};

const KEY: [u8; 32] = [7; 32];
const PRINCIPAL: [u8; 16] = [3; 16];

static ECHO_COMMANDS: [PluginOperation; 1] = [PluginOperation::new(
    "increment",
    CapabilityScope::SUBMIT_CALCULATION,
)];
static TELEMETRY_COMMANDS: [PluginOperation; 1] = [PluginOperation::new(
    "telemetry",
    CapabilityScope::STREAM_TELEMETRY,
)];

struct EchoPlugin;

impl Plugin for EchoPlugin {
    const DESCRIPTOR: PluginDescriptor = PluginDescriptor::new("echo", 1, &ECHO_COMMANDS, &[]);
}

impl PluginExecutor for EchoPlugin {
    fn invoke(&mut self, operation_name: &str, body: &[u8]) -> Result<Vec<u8>> {
        if operation_name != "increment" {
            return Err(MetisError::capability(
                ErrorCode::PluginOperationNotFound,
                "Test executor received an undeclared operation",
            ));
        }
        Ok(body.iter().map(|byte| byte.wrapping_add(1)).collect())
    }
}

struct TelemetryPlugin;

impl Plugin for TelemetryPlugin {
    const DESCRIPTOR: PluginDescriptor =
        PluginDescriptor::new("telemetry", 1, &TELEMETRY_COMMANDS, &[]);
}

impl PluginExecutor for TelemetryPlugin {
    fn invoke(&mut self, operation_name: &str, body: &[u8]) -> Result<Vec<u8>> {
        if operation_name != "telemetry" {
            return Err(MetisError::capability(
                ErrorCode::PluginOperationNotFound,
                "Test executor received an undeclared operation",
            ));
        }
        Ok(body.to_vec())
    }
}

fn header(message_type: MessageType, sequence: u64, payload: &[u8]) -> FrameHeader {
    FrameHeader {
        msg_type: message_type,
        sequence_id: sequence,
        payload_crc32: crc32(payload),
        payload_len: u32::try_from(payload.len()).expect("test payload fits wire field"),
    }
}

#[test]
fn capability_catalog_requires_handshake_and_advertises_supported_commands() {
    let mut service = BackendService::new(KEY, SafetyEnvelope::default());
    let empty = service
        .handle_request(&header(MessageType::CapabilityReq, 1, &[]), &[])
        .expect("typed missing-session response");
    assert_eq!(empty.0, MessageType::ErrorResp);
    let rejection = ErrorResponsePayload::decode(&empty.1).expect("error payload");
    assert_eq!(rejection.error_code, ErrorCode::MissingCapability as u16);

    let handshake = HandshakeRequestPayload {
        client_version: PROTOCOL_VERSION,
        client_process_id: 42,
        principal_id: PRINCIPAL,
    }
    .encode();
    let accepted = service
        .handle_request(
            &header(MessageType::HandshakeReq, 2, &handshake),
            &handshake,
        )
        .expect("handshake");
    assert_eq!(accepted.0, MessageType::HandshakeResp);

    let catalog = service
        .handle_request(&header(MessageType::CapabilityReq, 3, &[]), &[])
        .expect("catalog");
    assert_eq!(catalog.0, MessageType::CapabilityResp);
    let catalog = CapabilityCatalogPayload::decode(&catalog.1).expect("catalog payload");
    assert!(catalog.supports(MessageType::CapabilityReq));
    assert!(catalog.supports(MessageType::TargetCapabilityReq));
    assert!(catalog.supports(MessageType::HeartbeatReq));
    assert!(catalog.supports(MessageType::ClinicalCalcReq));
    assert!(catalog.supports(MessageType::PluginInvokeReq));
    assert!(!catalog.supports(MessageType::AuditQueryReq));
}

#[test]
fn known_but_unadvertised_command_returns_a_typed_protocol_error() {
    let mut service = BackendService::new(KEY, SafetyEnvelope::default());
    let handshake = HandshakeRequestPayload {
        client_version: PROTOCOL_VERSION,
        client_process_id: 42,
        principal_id: PRINCIPAL,
    }
    .encode();
    service
        .handle_request(
            &header(MessageType::HandshakeReq, 1, &handshake),
            &handshake,
        )
        .expect("handshake");
    let response = service
        .handle_request(&header(MessageType::AuditQueryReq, 2, &[]), &[])
        .expect("typed unsupported response");
    assert_eq!(response.0, MessageType::ErrorResp);
    let error = ErrorResponsePayload::decode(&response.1).expect("error payload");
    assert_eq!(error.error_code, ErrorCode::UnexpectedMessageType as u16);
}

#[test]
fn target_capability_discovery_reports_only_installed_host_surfaces() {
    let mut service = BackendService::new(KEY, SafetyEnvelope::default());
    let handshake = HandshakeRequestPayload {
        client_version: PROTOCOL_VERSION,
        client_process_id: 42,
        principal_id: PRINCIPAL,
    }
    .encode();
    service
        .handle_request(
            &header(MessageType::HandshakeReq, 1, &handshake),
            &handshake,
        )
        .expect("handshake");

    let response = service
        .handle_request(&header(MessageType::TargetCapabilityReq, 2, &[]), &[])
        .expect("target descriptor");
    assert_eq!(response.0, MessageType::TargetCapabilityResp);
    let descriptor = TargetCapabilityPayload::decode(&response.1).expect("target payload");
    assert_eq!(descriptor.platform(), TargetPlatform::current());
    assert!(descriptor.supports(TargetCapability::NativeProcess));
    assert!(!descriptor.supports(TargetCapability::PrivateProcessIpc));
    assert!(!descriptor.supports(TargetCapability::NativeWindow));

    service
        .add_target_capability(TargetCapability::PrivateProcessIpc)
        .expect("surface");
    let response = service
        .handle_request(&header(MessageType::TargetCapabilityReq, 3, &[]), &[])
        .expect("updated target descriptor");
    let descriptor = TargetCapabilityPayload::decode(&response.1).expect("target payload");
    assert!(descriptor.supports(TargetCapability::PrivateProcessIpc));
}

#[test]
fn plugin_invocation_authorizes_scope_and_returns_typed_response() {
    let mut service = BackendService::new(KEY, SafetyEnvelope::default());
    service.register_plugin(EchoPlugin).expect("echo plugin");
    service
        .register_plugin(TelemetryPlugin)
        .expect("telemetry plugin");
    let handshake = HandshakeRequestPayload {
        client_version: PROTOCOL_VERSION,
        client_process_id: 42,
        principal_id: PRINCIPAL,
    }
    .encode();
    let handshake_response = service
        .handle_request(
            &header(MessageType::HandshakeReq, 1, &handshake),
            &handshake,
        )
        .expect("handshake");
    let token = HandshakeResponsePayload::decode(&handshake_response.1)
        .expect("handshake payload")
        .initial_token;

    let request = PluginInvocationPayload::new(token.clone(), "echo", "increment", [1, 4, 9])
        .expect("invocation");
    let encoded = request.encode().expect("invocation payload");
    let response = service
        .handle_request(&header(MessageType::PluginInvokeReq, 2, &encoded), &encoded)
        .expect("plugin response");
    assert_eq!(response.0, MessageType::PluginInvokeResp);
    assert_eq!(
        PluginInvocationResponsePayload::decode(&response.1)
            .expect("plugin response payload")
            .body(),
        [2, 5, 10]
    );

    let request = PluginInvocationPayload::new(token.clone(), "telemetry", "telemetry", [])
        .expect("scoped invocation");
    let encoded = request.encode().expect("scoped invocation payload");
    let response = service
        .handle_request(&header(MessageType::PluginInvokeReq, 3, &encoded), &encoded)
        .expect("scope response");
    assert_eq!(response.0, MessageType::ErrorResp);
    assert_eq!(
        ErrorResponsePayload::decode(&response.1)
            .expect("scope error payload")
            .error_code,
        ErrorCode::InsufficientScope as u16
    );

    let request = PluginInvocationPayload::new(token.clone(), "echo", "missing", [])
        .expect("unknown operation invocation");
    let encoded = request.encode().expect("unknown operation payload");
    let response = service
        .handle_request(&header(MessageType::PluginInvokeReq, 4, &encoded), &encoded)
        .expect("unknown operation response");
    assert_eq!(response.0, MessageType::ErrorResp);
    assert_eq!(
        ErrorResponsePayload::decode(&response.1)
            .expect("unknown operation error payload")
            .error_code,
        ErrorCode::PluginOperationNotFound as u16
    );

    let request = PluginInvocationPayload::new(token, "missing", "increment", [])
        .expect("unknown plugin invocation");
    let encoded = request.encode().expect("unknown plugin payload");
    let response = service
        .handle_request(&header(MessageType::PluginInvokeReq, 5, &encoded), &encoded)
        .expect("unknown plugin response");
    assert_eq!(response.0, MessageType::ErrorResp);
    assert_eq!(
        ErrorResponsePayload::decode(&response.1)
            .expect("unknown plugin error payload")
            .error_code,
        ErrorCode::PluginNotFound as u16
    );
}

#[test]
fn browser_ui_plugin_returns_a_scoped_typed_fragment() {
    let mut service = BackendService::new(KEY, SafetyEnvelope::default());
    service
        .register_plugin(UiFragmentPlugin)
        .expect("UI plugin");
    let handshake = HandshakeRequestPayload {
        client_version: PROTOCOL_VERSION,
        client_process_id: 42,
        principal_id: PRINCIPAL,
    }
    .encode();
    let handshake_response = service
        .handle_request(
            &header(MessageType::HandshakeReq, 1, &handshake),
            &handshake,
        )
        .expect("handshake");
    let token = HandshakeResponsePayload::decode(&handshake_response.1)
        .expect("handshake payload")
        .initial_token;
    assert!(token.scope.contains(CapabilityScope::UI_RENDER));

    let action =
        FragmentAction::new(2, "status.describe", "metis-events", "session").expect("action");
    let request =
        PluginInvocationPayload::new(token, "ui", "action", action.encode().expect("action body"))
            .expect("invocation");
    let encoded = request.encode().expect("invocation body");
    let response = service
        .handle_request(&header(MessageType::PluginInvokeReq, 2, &encoded), &encoded)
        .expect("fragment response");
    assert_eq!(response.0, MessageType::PluginInvokeResp);
    let body = PluginInvocationResponsePayload::decode(&response.1)
        .expect("response envelope")
        .body()
        .to_vec();
    let patches = FragmentPatchSet::decode(&body).expect("typed patch set");
    assert_eq!(patches.generation(), 2);
    assert_eq!(patches.patches().len(), 1);
}
