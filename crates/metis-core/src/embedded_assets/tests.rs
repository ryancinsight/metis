//! Computed-value codec and boundary scenarios.
mod bounds;
mod codec;
#[cfg(not(target_arch = "wasm32"))]
mod directory_policy;
#[cfg(not(target_arch = "wasm32"))]
mod filesystem;
use super::*;

pub(super) fn traced(test: impl FnOnce()) {
    let subscriber = tracing_subscriber::fmt()
        .with_test_writer()
        .without_time()
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        let _test = tracing::info_span!("embedded_asset_contract").entered();
        test();
    });
}
fn identity(bytes: &[u8]) -> FileIdentity {
    FileIdentity::from_bytes(bytes).expect("bounded analytical bytes")
}
fn spec() -> InventorySpec {
    let provider = ProviderIdentity {
        package_id: "workspace:metis-web@0.5.0".into(),
        package: "metis-web".into(),
        version: "0.5.0".into(),
        source: ProviderSource::Workspace,
    };
    InventorySpec {
        module: ModuleIdentity {
            package: "metis-web".into(),
            package_id: provider.package_id.clone(),
            target: "metis_web".into(),
            wasm_target: "wasm32-unknown-unknown".into(),
            export: "metis_embedded_start".into(),
            native_platform: NativePlatform::Windows,
            page_schema: 1,
        },
        lock_digest: Digest::hash(b"version = 4\n"),
        providers: vec![provider],
        tools: [ToolKind::Cargo, ToolKind::Rustc, ToolKind::WasmBindgen].map(|kind| ToolIdentity {
            kind,
            version: match kind {
                ToolKind::Cargo => "cargo 1.97.0",
                ToolKind::Rustc => "rustc 1.97.0",
                ToolKind::WasmBindgen => "wasm-bindgen 0.2.125",
            }
            .into(),
            executable: identity(b"analytical executable identity"),
        }),
        configuration: BuildConfiguration {
            profile: "release".into(),
            features: vec!["embedded".into()],
            compiler_args: vec!["--target".into(), "wasm32-unknown-unknown".into()],
            environment: vec![("RUSTFLAGS".into(), None)],
        },
        directories: Vec::new(),
        inputs: vec![InputRecord {
            owner_id: "workspace".into(),
            path: "Cargo.lock".into(),
            identity: identity(b"version = 4\n"),
        }],
        assets: [
            AssetRecord {
                kind: AssetKind::JavaScript,
                destination: "metis_web.js".into(),
                identity: identity(b"export function metis_embedded_start() {}"),
            },
            AssetRecord {
                kind: AssetKind::WebAssembly,
                destination: "metis_web_bg.wasm".into(),
                identity: identity(b"\0asm\x01\0\0\0"),
            },
        ],
    }
}
