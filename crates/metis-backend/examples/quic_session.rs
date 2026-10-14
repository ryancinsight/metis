//! One mutually authenticated native Metis session using caller-owned TLS files.

#[path = "quic_session/udp.rs"]
mod udp;

use metis_backend::{
    BackendService, clinical::SafetyEnvelope, quic::serve_quic_session, service::SystemClock,
};
use metis_core::host::{HostContext, HostOrigin, HostPolicy, HostSessionId, WindowId};
use metis_core::protocol::{
    ClinicalCalcRequestPayload, ClinicalCalcResponsePayload, ErrorResponsePayload, MessageType,
    TargetCapability,
};
use metis_ipc::{IpcClient, QuicIpcTransport, quic_config};
use moirai_quic::{Identity, QuicConfig, QuicEndpoint, rustls};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

// Condition 1: CLI I/O, TLS and application errors have distinct concrete types;
// error erasure occurs only at the executable's diagnostic boundary.
type ExampleResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug)]
struct PeerRejection(ErrorResponsePayload);

impl std::fmt::Display for PeerRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "backend rejects request with {:#06x}: {}",
            self.0.error_code, self.0.message
        )
    }
}

impl std::error::Error for PeerRejection {}

const USAGE: &str = "quic_session server ADDR SERVER_NAME CERT KEY CLIENT_ROOTS AUTHORIZED_CLIENT_CERT SESSION_MAC_KEY\nquic_session client ADDR SERVER_NAME CERT KEY SERVER_ROOTS WEIGHT_KG CONCENTRATION_MG_ML DOSE_MCG_KG_MIN";

fn certificates(path: &Path) -> ExampleResult<Vec<CertificateDer<'static>>> {
    let bytes = std::fs::read(path)?;
    let chain = CertificateDer::pem_slice_iter(&bytes).collect::<Result<Vec<_>, _>>()?;
    if chain.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "certificate file contains no certificates",
        )
        .into());
    }
    Ok(chain)
}

fn principal(certificate: &CertificateDer<'_>) -> [u8; 16] {
    let digest = moirai_crypto::sha256(certificate.as_ref());
    let mut principal = [0; 16];
    principal.copy_from_slice(&digest[..16]);
    principal
}

struct EndpointParameters<'a> {
    executor: Arc<moirai_executor::HybridExecutor>,
    address: std::net::SocketAddr,
    name: &'a str,
    identity: Identity,
    roots: rustls::RootCertStore,
    config: QuicConfig,
}

fn run_server(
    parameters: EndpointParameters<'_>,
    authorized_client: &Path,
    session_key: &Path,
) -> ExampleResult<QuicEndpoint> {
    let allowed = certificates(authorized_client)?;
    let master_key = std::fs::read(session_key)?
        .try_into()
        .map_err(|bytes: Vec<u8>| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "session MAC key contains {} bytes; requires 32",
                    bytes.len()
                ),
            )
        })?;
    let endpoint = QuicEndpoint::server(
        parameters.executor,
        parameters.address,
        parameters.identity,
        Some(parameters.roots),
        parameters.config,
    )?;
    tracing::info!(address = %endpoint.local_addr()?, udp_receive_bytes = endpoint.udp_buffer_sizes().receive, udp_send_bytes = endpoint.udp_buffer_sizes().send, "QUIC listener bound");
    let connection = moirai_executor::block_on(endpoint.accept())?;
    let peer = connection.peer_certificates().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "mutual TLS client identity is absent",
        )
    })?;
    if peer.first() != allowed.first() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "client certificate is outside the authorized identity",
        )
        .into());
    }
    let origin = HostOrigin::parse(&format!("https://{}", parameters.name))?;
    let window = WindowId::new(1)?;
    // Certificate equality authorizes the peer; the hash is only a label.
    // A fresh MAC key is supplied for this single service lifetime.
    let context = HostContext::new(
        origin.clone(),
        window,
        HostSessionId::new(principal(&peer[0]))?,
    );
    let service = BackendService::with_trusted_context(
        master_key,
        SafetyEnvelope::default(),
        SystemClock::default(),
        HostPolicy::new(origin, window),
        context,
    )?;
    let result: ExampleResult<()> = moirai_executor::block_on(async {
        let stream = connection.accept_stream().await?;
        serve_quic_session(stream, service).await?;
        Ok(())
    });
    result?;
    Ok(endpoint)
}

fn run_client(
    parameters: EndpointParameters<'_>,
    own_principal: [u8; 16],
    calculation: &[String],
) -> ExampleResult<QuicEndpoint> {
    let bind = if parameters.address.is_ipv6() {
        "[::]:0"
    } else {
        "0.0.0.0:0"
    };
    let endpoint = QuicEndpoint::client(
        parameters.executor,
        bind.parse()?,
        parameters.roots,
        Some(parameters.identity),
        parameters.config,
    )?;
    tracing::info!(
        udp_receive_bytes = endpoint.udp_buffer_sizes().receive,
        udp_send_bytes = endpoint.udp_buffer_sizes().send,
        "QUIC client bound"
    );
    let connection =
        moirai_executor::block_on(endpoint.connect(parameters.address, parameters.name))?;
    let stream = moirai_executor::block_on(connection.open_stream())?;
    let mut client = IpcClient::new(QuicIpcTransport::new(stream)?);
    let token = client.handshake(std::process::id(), own_principal)?;
    let targets = client.discover_target_capabilities()?;
    if !targets.supports(TargetCapability::NativeQuic) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "peer omits its negotiated native QUIC capability",
        )
        .into());
    }
    let request = ClinicalCalcRequestPayload {
        token,
        patient_id: "QUIC-DEMONSTRATION".to_owned(),
        weight_kg: calculation[0].parse()?,
        concentration_mg_ml: calculation[1].parse()?,
        target_dose_mcg_kg_min: calculation[2].parse()?,
    };
    let (kind, payload) = client.send_and_recv(MessageType::ClinicalCalcReq, &request.encode()?)?;
    if kind == MessageType::ErrorResp {
        return Err(PeerRejection(ErrorResponsePayload::decode(&payload)?).into());
    }
    if kind != MessageType::ClinicalCalcResp {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "backend sends an unexpected calculation response type",
        )
        .into());
    }
    let result = ClinicalCalcResponsePayload::decode(&payload)?;
    let event = client.recv_event()?;
    if event.decode_as::<ClinicalCalcResponsePayload>()? != result {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "clinical response and audited event disagree",
        )
        .into());
    }
    tracing::info!(
        rate_ml_hr = result.rate_ml_hr,
        drug_rate_mg_hr = result.drug_rate_mg_hr,
        audit_sequence = result.audit_sequence_id,
        "QUIC backend calculation completed"
    );
    client.into_transport().close()?;
    Ok(endpoint)
}

fn main() -> ExampleResult<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if !matches!(
        arguments.first().map(String::as_str),
        Some("server" | "client")
    ) || arguments.len() != if arguments[0] == "server" { 8 } else { 9 }
    {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, USAGE).into());
    }
    let executor = Arc::new(
        moirai_executor::ExecutorBuilder::new()
            .worker_threads(2)
            .async_threads(1)
            .build()?,
    );
    let chain = certificates(Path::new(&arguments[3]))?;
    let own_principal = principal(&chain[0]);
    let key = PrivateKeyDer::from_pem_slice(&std::fs::read(&arguments[4])?)?;
    let mut roots = rustls::RootCertStore::empty();
    for root in certificates(Path::new(&arguments[5]))? {
        roots.add(root)?;
    }
    // One connection/session admits the entire Metis frame domain. Operator
    // deadlines are finite, not protocol timing guesses.
    let parameters = EndpointParameters {
        executor,
        address: arguments[1].parse()?,
        name: &arguments[2],
        identity: Identity::new(chain, key),
        roots,
        config: quic_config(
            1,
            1,
            65_536,
            Duration::from_secs(30),
            Duration::from_mins(1),
        )?
        .with_udp_buffers(udp::buffers()?)?,
    };
    let endpoint = if arguments[0] == "server" {
        run_server(
            parameters,
            Path::new(&arguments[6]),
            Path::new(&arguments[7]),
        )?
    } else {
        run_client(parameters, own_principal, &arguments[6..])?
    };
    endpoint.close();
    moirai_executor::block_on(endpoint.wait_idle())?;
    Ok(())
}
