mod acceptance;
mod rejection;

use super::*;
use crate::clinical::SafetyEnvelope;
use crate::service::SystemClock;
use metis_core::host::{HostContext, HostOrigin, HostPolicy, HostSessionId, WindowId};
use metis_core::protocol::{ClinicalCalcRequestPayload, ClinicalCalcResponsePayload, MessageType};
use metis_ipc::{IpcClient, IpcTransport, QuicIpcTransport, quic_config};
use moirai_core::executor::TaskSpawner;
use moirai_executor::HybridExecutor;
use moirai_quic::{Identity, QuicConnection, QuicEndpoint, rustls};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
use std::sync::Arc;
use std::time::Duration;
use tracing::Instrument;

const PRINCIPAL: [u8; 16] = [0x66; 16];
const OTHER_PRINCIPAL: [u8; 16] = [0x67; 16];
const DEADLINE: Duration = Duration::from_secs(2);

fn executor() -> Arc<HybridExecutor> {
    Arc::new(
        moirai_executor::ExecutorBuilder::new()
            .worker_threads(2)
            .async_threads(1)
            .build()
            .expect("Moirai executor"),
    )
}

fn endpoints(executor: &Arc<HybridExecutor>) -> (QuicEndpoint, QuicEndpoint) {
    let certificates = CertificateDer::pem_slice_iter(include_bytes!(
        "../../../../tests/fixtures/localhost-cert.pem"
    ))
    .collect::<std::result::Result<Vec<_>, _>>()
    .expect("published certificate fixture");
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(certificates.last().expect("fixture root").clone())
        .expect("trusted root");
    let key = PrivateKeyDer::from_pem_slice(include_bytes!(
        "../../../../tests/fixtures/localhost-key.pem"
    ))
    .expect("published fixture key");
    let config =
        quic_config(4, 2, 65_536, DEADLINE, Duration::from_secs(10)).expect("bounded Metis config");
    let server = QuicEndpoint::server(
        Arc::clone(executor),
        "127.0.0.1:0".parse().expect("loopback"),
        Identity::new(certificates, key),
        None,
        config.clone(),
    )
    .expect("server");
    let client = QuicEndpoint::client(
        Arc::clone(executor),
        "127.0.0.1:0".parse().expect("loopback"),
        roots,
        None,
        config,
    )
    .expect("client");
    (server, client)
}

fn connections(server: &QuicEndpoint, client: &QuicEndpoint) -> (QuicConnection, QuicConnection) {
    let (server, client) = moirai_executor::block_on(futures::future::join(
        server.accept(),
        client.connect(server.local_addr().expect("server address"), "localhost"),
    ));
    (
        server.expect("server handshake"),
        client.expect("client handshake"),
    )
}

fn service(principal: [u8; 16], master_key: [u8; 32]) -> BackendService {
    let origin = HostOrigin::parse("https://localhost").expect("trusted host origin");
    let window = WindowId::new(1).expect("window");
    let context = HostContext::new(
        origin.clone(),
        window,
        HostSessionId::new(principal).expect("principal"),
    );
    BackendService::with_trusted_context(
        master_key,
        SafetyEnvelope::default(),
        SystemClock::default(),
        HostPolicy::new(origin, window),
        context,
    )
    .expect("trusted service")
}

fn calculation(
    token: metis_core::capability::CapabilityToken,
    weight: f64,
) -> ClinicalCalcRequestPayload {
    ClinicalCalcRequestPayload {
        token,
        patient_id: "PT-9042-ALPHA".to_owned(),
        weight_kg: weight,
        concentration_mg_ml: 4.0,
        target_dose_mcg_kg_min: 0.5,
    }
}

fn trace() -> tracing::subscriber::DefaultGuard {
    tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_test_writer()
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
            .with_max_level(tracing::Level::TRACE)
            .finish(),
    )
}
