//! `prepare` unit tests: manifest validation, fixture-driven resolution,
//! assembly round-trips and the readable report. Subprocess orchestration in
//! `run` stays thin by construction and is covered by the pre-push gate.

use super::providers::{LockedChecksums, hex_to_digest, split_git_source};
use super::*;
use metis_core::embedded_assets::{ProviderRecord, ProviderSource};
use std::sync::atomic::{AtomicU64, Ordering};

/// Hermetic scratch tree, removed on drop.
struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("metis-prepare-{}-{id}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("fixture root");
        Self { root }
    }

    fn file(&self, rel: &str, bytes: &[u8]) -> PathBuf {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture parent");
        }
        fs::write(&path, bytes).expect("fixture file");
        path
    }

    fn manifest(&self, embedded: &str) -> PathBuf {
        let document = format!(
            concat!(
                "{{\"schema\": 1, \"id\": \"com.example.app\", \"name\": \"App\", ",
                "\"version\": \"1.0.0\", \"manufacturer\": \"Example\", ",
                "\"upgrade_code\": \"{{12345678-1234-5678-ABCD-EF1234567890}}\", ",
                "\"cargo_manifest\": \"Cargo.toml\", \"entry\": \"app\", ",
                "\"binaries\": [{{\"package\": \"p\", \"bin\": \"app\"}}]{embedded}}}",
            ),
            embedded = embedded
        );
        self.file("metis.json", document.as_bytes())
    }
}

fn valid_embedded() -> &'static str {
    r#", "embedded_module": {"package": "page", "platform": "windows", "export": "start", "page_schema": 1}"#
}

#[test]
fn manifest_accepts_embedded_module() {
    let fixture = Fixture::new("manifest-ok");
    let manifest = fixture.manifest(valid_embedded());
    let (application, _root) = Application::read(&manifest).expect("valid manifest");
    let module = application.embedded_module.expect("module present");
    assert_eq!(module.package, "page");
    assert_eq!(module.platform, "windows");
    assert_eq!(module.export, "start");
    assert_eq!(module.page_schema, 1);
}

#[test]
fn manifest_without_embedded_module_stays_native() {
    let fixture = Fixture::new("manifest-plain");
    let manifest = fixture.manifest("");
    let (application, _root) = Application::read(&manifest).expect("plain manifest");
    assert!(application.embedded_module.is_none());
}

#[test]
fn manifest_rejects_unknown_platform() {
    let fixture = Fixture::new("manifest-platform");
    let manifest = fixture.manifest(
        r#", "embedded_module": {"package": "page", "platform": "linux", "export": "start", "page_schema": 1}"#,
    );
    assert!(Application::read(&manifest).is_err());
}

#[test]
fn manifest_rejects_bad_export() {
    let fixture = Fixture::new("manifest-export");
    let manifest = fixture.manifest(
        r#", "embedded_module": {"package": "page", "platform": "windows", "export": "9bad", "page_schema": 1}"#,
    );
    assert!(Application::read(&manifest).is_err());
}

#[test]
fn manifest_rejects_unknown_embedded_fields() {
    let fixture = Fixture::new("manifest-extra");
    let manifest = fixture.manifest(
        r#", "embedded_module": {"package": "page", "platform": "windows", "export": "start", "page_schema": 1, "mode": "x"}}"#,
    );
    assert!(Application::read(&manifest).is_err());
}

#[test]
fn manifest_rejects_browser_document_with_embedded_module() {
    let fixture = Fixture::new("manifest-mixed");
    let path = fixture.file(
        "metis.json",
        br#"{"schema": 1, "frontend": {}, "embedded_module": {"package": "page", "platform": "windows", "export": "start", "page_schema": 1}}"#,
    );
    assert!(crate::manifest::Manifest::read(&path).is_err());
}

fn fixture_metadata() -> serde_json::Value {
    serde_json::json!({
        "workspace_root": "/tmp/ws",
        "packages": [
            {
                "id": "page 0.1.0 (path+file:///tmp/ws/page)",
                "name": "page",
                "version": "0.1.0",
                "manifest_path": "/tmp/ws/page/Cargo.toml",
                "source": null,
                "targets": [{"name": "page", "kind": ["cdylib"]}],
            },
            {
                "id": "registry+https://github.com/rust-lang/crates.io-index#wasm-bindgen#0.2.100",
                "name": "wasm-bindgen",
                "version": "0.2.100",
                "manifest_path": "/registry/wasm-bindgen-0.2.100/Cargo.toml",
                "source": "registry+https://github.com/rust-lang/crates.io-index",
                "checksum": "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
                "targets": [{"name": "wasm-bindgen", "kind": ["bin"]}],
            },
            {
                "id": "git+https://example.invalid/dep#abc123",
                "name": "dep",
                "version": "0.2.0",
                "manifest_path": "/git/dep/Cargo.toml",
                "source": "git+https://example.invalid/dep#abc123",
                "targets": [{"name": "dep", "kind": ["lib"]}],
            }
        ],
        "workspace_members": ["page 0.1.0 (path+file:///tmp/ws/page)"],
        "resolve": {
            "nodes": [
                {"id": "page 0.1.0 (path+file:///tmp/ws/page)", "dependencies": [
                    "registry+https://github.com/rust-lang/crates.io-index#wasm-bindgen#0.2.100",
                    "git+https://example.invalid/dep#abc123"
                ]},
                {"id": "registry+https://github.com/rust-lang/crates.io-index#wasm-bindgen#0.2.100", "dependencies": []},
                {"id": "git+https://example.invalid/dep#abc123", "dependencies": []}
            ]
        }
    })
}

fn fixture_document() -> serde_json::Value {
    fixture_metadata()
}

fn digest_bytes(data: &[u8]) -> [u8; 32] {
    use moirai_crypto::Sha256;
    let mut hash = Sha256::new();
    hash.update(data);
    hash.finalize()
}

#[test]
fn package_dir_resolves_member_manifest() {
    let document = fixture_document();
    let dir = package_dir(&document, "page 0.1.0 (path+file:///tmp/ws/page)").expect("member");
    assert_eq!(dir, Path::new("/tmp/ws/page"));
}

#[test]
fn package_dir_rejects_outsider() {
    let document = fixture_document();
    assert!(
        package_dir(
            &document,
            "outsider 9.9.9 (registry+https://example.invalid)"
        )
        .is_err()
    );
}

fn fixture_lock() -> &'static str {
    concat!(
        "[[package]]\n",
        "name = \"wasm-bindgen\"\n",
        "version = \"0.2.100\"\n",
        "source = \"registry+https://github.com/rust-lang/crates.io-index\"\n",
        "checksum = \"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\"\n",
    )
}

fn fixture_checksums() -> LockedChecksums {
    lock_checksums(fixture_lock()).expect("fixture lock")
}

#[test]
fn providers_map_all_source_kinds() {
    let document = fixture_document();
    let providers = providers(
        &document,
        "page 0.1.0 (path+file:///tmp/ws/page)",
        &fixture_checksums(),
    )
    .expect("providers");
    assert_eq!(providers.len(), 3);
    let kinds: Vec<_> = providers
        .iter()
        .map(|provider| match &provider.source {
            ProviderSource::Path { .. } => "path",
            ProviderSource::Registry { .. } => "registry",
            ProviderSource::Git { .. } => "git",
        })
        .collect();
    assert!(kinds.contains(&"path"));
    assert!(kinds.contains(&"registry"));
    assert!(kinds.contains(&"git"));
}

#[test]
fn providers_reject_unknown_source() {
    let mut document = fixture_metadata();
    document["packages"][1]["source"] = serde_json::Value::String("ftp://example.invalid/x".into());
    assert!(
        providers(
            &document,
            "page 0.1.0 (path+file:///tmp/ws/page)",
            &fixture_checksums()
        )
        .is_err()
    );
}

#[test]
fn providers_take_registry_checksums_from_the_lock() {
    // The metadata graph carries a decoy checksum; the lock wins.
    let document = fixture_document();
    let providers = providers(
        &document,
        "page 0.1.0 (path+file:///tmp/ws/page)",
        &fixture_checksums(),
    )
    .expect("providers");
    let registry = providers
        .iter()
        .find_map(|provider| match &provider.source {
            ProviderSource::Registry { checksum } => Some(*checksum),
            _ => None,
        })
        .expect("registry provider");
    let expected =
        hex_to_digest("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
            .expect("fixture checksum");
    assert_eq!(registry, expected);
}

#[test]
fn providers_reject_registry_package_missing_from_the_lock() {
    let document = fixture_document();
    assert!(
        providers(
            &document,
            "page 0.1.0 (path+file:///tmp/ws/page)",
            &LockedChecksums::new()
        )
        .is_err()
    );
}

#[test]
fn lock_checksums_reject_unpinned_registry_entry() {
    let lock = concat!(
        "[[package]]\n",
        "name = \"orphan\"\n",
        "version = \"1.0.0\"\n",
        "source = \"registry+https://github.com/rust-lang/crates.io-index\"\n",
    );
    assert!(lock_checksums(lock).is_err());
}

#[test]
fn lock_checksums_reject_malformed_hex() {
    let lock = concat!(
        "[[package]]\n",
        "name = \"broken\"\n",
        "version = \"1.0.0\"\n",
        "source = \"registry+https://github.com/rust-lang/crates.io-index\"\n",
        "checksum = \"not-hex\"\n",
    );
    assert!(lock_checksums(lock).is_err());
}

#[test]
fn hash_generated_streams_length_and_digest() {
    let fixture = Fixture::new("hash-generated");
    let module = &[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
    let path = fixture.file("page_bg.wasm", module);
    let hashed = hash_generated(&path, "page_bg.wasm").expect("hash");
    assert_eq!(hashed.length, 8);
    assert_eq!(hashed.digest, digest_bytes(module));
    let empty = fixture.file("empty.js", &[]);
    assert!(hash_generated(&empty, "empty.js").is_err());
    assert!(hash_generated(&fixture.root.join("missing.js"), "missing.js").is_err());
}

#[test]
fn split_git_source_shapes() {
    let (url, revision) =
        split_git_source("git+https://example.invalid/dep?rev=main#abc123").expect("shaped");
    assert_eq!(url, "https://example.invalid/dep");
    assert_eq!(revision, "abc123");
    assert!(split_git_source("git+https://example.invalid/dep").is_err());
    assert!(split_git_source("https://example.invalid/dep#abc123").is_err());
}

#[test]
fn hex_to_digest_round_trip() {
    let digest = hex_to_digest("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
        .expect("valid checksum");
    assert_eq!(digest[0], 0x01);
    assert_eq!(digest[31], 0xef);
    assert!(hex_to_digest("xyz").is_err());
    assert!(hex_to_digest(&"ab".repeat(31)).is_err());
}

#[test]
fn parse_bindgen_version_shapes() {
    assert_eq!(
        parse_bindgen_version(b"wasm-bindgen 0.2.100\n").expect("shaped"),
        "0.2.100"
    );
    assert!(parse_bindgen_version(b"wasm-bindgen\n").is_err());
    assert!(parse_bindgen_version(b"\xff\xfe").is_err());
}

#[test]
fn collect_inputs_walks_sorted_relative_files() {
    let fixture = Fixture::new("inputs");
    fixture.file("Cargo.toml", b"[package]");
    fixture.file("src/lib.rs", b"// lib");
    fixture.file("data.bin", &[]);
    fixture.file("target/skip.me", b"x");
    fixture.file(".hidden", b"x");
    let inputs = collect_inputs(&fixture.root, "m").expect("inputs");
    let paths: Vec<_> = inputs.iter().map(|input| input.path.as_str()).collect();
    assert_eq!(paths, ["Cargo.toml", "data.bin", "src/lib.rs"]);
    assert!(inputs.iter().all(|input| input.owner == "m"));
    let empty = inputs
        .iter()
        .find(|input| input.path == "data.bin")
        .expect("empty file");
    assert_eq!(empty.length, 0);
    assert_eq!(
        hex::encode(&empty.digest),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn assemble_round_trips_through_canonical_bytes() {
    let parts = Assembly {
        package: "page".into(),
        package_id: "page 0.1.0".into(),
        target: "page".into(),
        export: "start".into(),
        platform: "windows".into(),
        page_schema: 1,
        lock_digest: [7u8; 32],
        providers: vec![ProviderRecord {
            package_id: "dep 0.2.0".into(),
            version: "0.2.0".into(),
            source: ProviderSource::Git {
                url: "https://example.invalid/dep".into(),
                revision: "abc123".into(),
            },
        }],
        tools: vec![
            ToolRecorded {
                name: "cargo".into(),
                version: "cargo 1".into(),
                digest: [1u8; 32],
            },
            ToolRecorded {
                name: "rustc".into(),
                version: "rustc 2".into(),
                digest: [2u8; 32],
            },
            ToolRecorded {
                name: "wasm-bindgen".into(),
                version: "wasm-bindgen 3".into(),
                digest: [9u8; 32],
            },
        ],
        rustflags: vec!["-C debuginfo=line-tables-only".into()],
        inputs: vec![InputRecord {
            owner: "m".into(),
            path: "a.rs".into(),
            length: 3,
            digest: [3u8; 32],
        }],
        loader: HashedFile {
            length: 6,
            digest: digest_bytes(b"loader"),
        },
        wasm: HashedFile {
            length: 8,
            digest: digest_bytes(&[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]),
        },
    };
    let inventory = assemble(parts);
    assert_eq!(inventory.tools[0].name, "cargo");
    let encoded = encode(&inventory).expect("canonical bytes");
    let decoded = decode(&encoded).expect("round trip");
    assert_eq!(decoded.export_name, "start");
    assert_eq!(decoded.assets.len(), 2);
    let report = report_json(&decoded, &digest_bytes(&encoded));
    assert_eq!(report["magic"], serde_json::Value::from("METISAS1"));
    assert_eq!(report["package"], serde_json::Value::from("page"));
    assert_eq!(report["assets"][1]["kind"], serde_json::Value::from("wasm"));
    assert_eq!(
        report["inventory_sha256"],
        serde_json::Value::from(hex::encode(&digest_bytes(&encoded)))
    );
}

#[test]
fn write_new_refuses_existing_files() {
    let fixture = Fixture::new("write-new");
    let path = fixture.file("report.json", b"{}");
    assert!(write_new(&path, b"{}").is_err());
    let fresh = fixture.root.join("fresh.json");
    write_new(&fresh, b"{}").expect("create");
    assert_eq!(std::fs::read(&fresh).expect("read back"), b"{}");
}

mod hex {
    pub(crate) fn encode(bytes: &[u8]) -> String {
        use std::fmt::Write;
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            write!(output, "{byte:02x}").expect("formatting cannot fail");
        }
        output
    }
}
