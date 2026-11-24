//! Round-trip and rejection tests for the inventory codec.
//!
//! Hand-built byte vectors (not `encode` output) exercise every decode
//! rejection, so a decoder that merely echoes the encoder cannot pass.

use super::*;

fn sample() -> EmbeddedModuleInventory {
    EmbeddedModuleInventory {
        package: "metis-web".into(),
        package_id: "metis-web 0.5.0".into(),
        cdylib_target: "metis_web".into(),
        wasm_target: "wasm32-unknown-unknown".into(),
        export_name: "start".into(),
        platform_tag: "windows".into(),
        page_schema: 1,
        lock_digest: [9; 32],
        providers: vec![
            ProviderRecord {
                package_id: "zeta 2.0.0".into(),
                version: "2.0.0".into(),
                source: ProviderSource::Registry { checksum: [1; 32] },
            },
            ProviderRecord {
                package_id: "alpha 1.0.0".into(),
                version: "1.0.0".into(),
                source: ProviderSource::Git {
                    url: "https://example.invalid/alpha".into(),
                    revision: "abc123".into(),
                },
            },
            ProviderRecord {
                package_id: "mid 0.5.0".into(),
                version: "0.5.0".into(),
                source: ProviderSource::Path {
                    member: "mid".into(),
                },
            },
        ],
        tools: vec![
            ToolRecord {
                name: "wasm-bindgen".into(),
                version_output: "wasm-bindgen 0.2".into(),
                executable_digest: [3; 32],
            },
            ToolRecord {
                name: "cargo".into(),
                version_output: "cargo 1.97.0".into(),
                executable_digest: [4; 32],
            },
            ToolRecord {
                name: "rustc".into(),
                version_output: "rustc 1.97.0".into(),
                executable_digest: [5; 32],
            },
        ],
        config: BuildConfiguration {
            profile: "release".into(),
            features: vec!["b".into(), "a".into(), "a".into()],
            rustflags: vec!["-C debuginfo=line-tables-only".into()],
            environment: vec![("B".into(), "2".into()), ("A".into(), "1".into())],
        },
        inputs: vec![
            InputRecord {
                owner: "m".into(),
                path: "b/page.js".into(),
                length: 10,
                digest: [6; 32],
            },
            InputRecord {
                owner: "m".into(),
                path: "a/lib.rs".into(),
                length: 20,
                digest: [7; 32],
            },
        ],
        assets: vec![
            AssetRecord {
                kind: AssetKind::Wasm,
                destination: "b.wasm".into(),
                length: 100,
                digest: [8; 32],
            },
            AssetRecord {
                kind: AssetKind::JavaScript,
                destination: "a.js".into(),
                length: 50,
                digest: [9; 32],
            },
        ],
    }
}

fn is_err(result: &Result<EmbeddedModuleInventory>) -> bool {
    result.is_err()
}

#[test]
fn round_trip_canonicalizes_and_reencodes_stably() {
    let bytes = match encode(&sample()) {
        Ok(bytes) => bytes,
        Err(_) => {
            assert!(false, "sample must encode");
            return;
        }
    };
    assert_eq!(&bytes[..8], INVENTORY_MAGIC);
    assert_eq!(&bytes[8..10], &INVENTORY_SCHEMA.to_be_bytes());
    let decoded = match decode(&bytes) {
        Ok(inventory) => inventory,
        Err(_) => {
            assert!(false, "canonical bytes must decode");
            return;
        }
    };
    assert_eq!(decoded.providers[0].package_id, "alpha 1.0.0");
    assert_eq!(decoded.tools[0].name, "cargo");
    assert_eq!(
        decoded.config.features,
        vec!["a".to_string(), "b".to_string()]
    );
    assert_eq!(decoded.inputs[0].path, "a/lib.rs");
    assert_eq!(decoded.assets[0].destination, "a.js");
    assert_eq!(decoded.assets[1].destination, "b.wasm");
    let reencoded = match encode(&decoded) {
        Ok(bytes) => bytes,
        Err(_) => {
            assert!(false, "decoded inventory must re-encode");
            return;
        }
    };
    assert_eq!(bytes, reencoded);
}

#[test]
fn rejects_bad_magic_and_schema() {
    let bytes = match encode(&sample()) {
        Ok(bytes) => bytes,
        Err(_) => {
            assert!(false, "sample must encode");
            return;
        }
    };
    let mut bad = bytes.clone();
    bad[0] ^= 0xff;
    assert!(is_err(&decode(&bad)));
    let mut bad_schema = bytes;
    bad_schema[8] = 0;
    bad_schema[9] = 2;
    match decode(&bad_schema) {
        Err(InventoryError::UnknownSchema(2)) => {}
        _ => assert!(false, "schema 2 must be rejected by version"),
    }
}

#[test]
fn rejects_every_truncation() {
    let bytes = match encode(&sample()) {
        Ok(bytes) => bytes,
        Err(_) => {
            assert!(false, "sample must encode");
            return;
        }
    };
    for cut in 0..bytes.len() {
        assert!(
            decode(&bytes[..cut]).is_err(),
            "truncation at {cut} must fail"
        );
    }
}

#[test]
fn rejects_trailing_bytes() {
    let mut bytes = match encode(&sample()) {
        Ok(bytes) => bytes,
        Err(_) => {
            assert!(false, "sample must encode");
            return;
        }
    };
    bytes.push(0);
    assert!(matches!(decode(&bytes), Err(InventoryError::TrailingBytes)));
}

#[test]
fn rejects_duplicate_and_case_colliding_inputs_on_encode() {
    let mut dup = sample();
    dup.inputs.push(InputRecord {
        owner: "m".into(),
        path: "a/lib.rs".into(),
        length: 1,
        digest: [0; 32],
    });
    assert!(matches!(encode(&dup), Err(InventoryError::DuplicatePath)));
    let mut colliding = sample();
    colliding.inputs[1].path = "B/PAGE.JS".into();
    assert!(matches!(
        encode(&colliding),
        Err(InventoryError::CaseCollidingPaths)
    ));
}

#[test]
fn rejects_bad_tool_and_asset_sets_on_encode() {
    let mut two_tools = sample();
    two_tools.tools.pop();
    assert!(matches!(
        encode(&two_tools),
        Err(InventoryError::BadToolSet)
    ));
    let mut one_asset = sample();
    one_asset.assets.pop();
    assert!(matches!(
        encode(&one_asset),
        Err(InventoryError::BadAssetSet)
    ));
}

#[test]
fn rejects_oversize_asset_on_encode() {
    let mut big = sample();
    big.assets[0].length = MAX_MODULE_BYTES + 1;
    assert!(matches!(encode(&big), Err(InventoryError::OversizeAsset)));
}

#[test]
fn rejects_oversize_manifest_on_encode() {
    let mut big = sample();
    big.providers.clear();
    big.inputs = (0..MAX_RECORDS)
        .map(|i| InputRecord {
            owner: "o".into(),
            path: format!("p{i:06}/{}", "a".repeat(200)),
            length: 1,
            digest: [0; 32],
        })
        .collect();
    assert!(matches!(
        encode(&big),
        Err(InventoryError::OversizeManifest)
    ));
}

// --- Hand-built byte vectors: decode-only rejection paths. ---

fn u16be(value: u16, out: &mut Vec<u8>) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn u32be(value: u32, out: &mut Vec<u8>) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn field(out: &mut Vec<u8>, value: &[u8]) {
    let len = value.len() as u32;
    u32be(len, out);
    out.extend_from_slice(value);
}

fn tool(out: &mut Vec<u8>, name: &str) {
    field(out, name.as_bytes());
    field(out, b"v");
    out.extend_from_slice(&[0u8; 32]);
}

/// Canonical prefix through the lock digest: magic, schema, six empty
/// identity strings, zero page schema, zero lock digest.
fn head() -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(INVENTORY_MAGIC);
    u16be(INVENTORY_SCHEMA, &mut out);
    for _ in 0..6 {
        u32be(0, &mut out);
    }
    u16be(0, &mut out);
    out.extend_from_slice(&[0u8; 32]);
    out
}

fn providers_zero(out: &mut Vec<u8>) {
    u32be(0, out);
}

fn tools_named(out: &mut Vec<u8>, tool_names: &[&str]) {
    u32be(tool_names.len() as u32, out);
    for name in tool_names {
        tool(out, name);
    }
}

fn config_empty(out: &mut Vec<u8>) {
    field(out, b"");
    u32be(0, out);
    u32be(0, out);
    u32be(0, out);
}

fn inputs_section(out: &mut Vec<u8>, records: &[Vec<u8>]) {
    u32be(records.len() as u32, out);
    for record in records {
        out.extend_from_slice(record);
    }
}

fn input_rec(owner: &str, path: &str) -> Vec<u8> {
    let mut out = Vec::new();
    field(&mut out, owner.as_bytes());
    field(&mut out, path.as_bytes());
    out.extend_from_slice(&0u64.to_be_bytes());
    out.extend_from_slice(&[0u8; 32]);
    out
}

fn assets_js_wasm(js: &str, wasm: &str) -> Vec<u8> {
    let mut out = Vec::new();
    u32be(2, &mut out);
    out.push(0);
    field(&mut out, js.as_bytes());
    out.extend_from_slice(&0u64.to_be_bytes());
    out.extend_from_slice(&[0u8; 32]);
    out.push(1);
    field(&mut out, wasm.as_bytes());
    out.extend_from_slice(&0u64.to_be_bytes());
    out.extend_from_slice(&[0u8; 32]);
    out
}

#[test]
fn decode_rejects_unsorted_inputs() {
    let mut bytes = head();
    providers_zero(&mut bytes);
    tools_named(&mut bytes, &["cargo", "rustc", "wasm-bindgen"]);
    config_empty(&mut bytes);
    inputs_section(
        &mut bytes,
        &[input_rec("m", "b.js"), input_rec("m", "a.js")],
    );
    bytes.extend_from_slice(&assets_js_wasm("a.js", "b.wasm"));
    assert!(matches!(
        decode(&bytes),
        Err(InventoryError::UnsortedRecords)
    ));
}

#[test]
fn decode_rejects_unsorted_tools() {
    let mut bytes = head();
    providers_zero(&mut bytes);
    tools_named(&mut bytes, &["b", "a", "c"]);
    assert!(matches!(
        decode(&bytes),
        Err(InventoryError::UnsortedRecords)
    ));
}

#[test]
fn decode_rejects_bad_utf8() {
    let mut bytes = head();
    providers_zero(&mut bytes);
    tools_named(&mut bytes, &["cargo", "rustc", "wasm-bindgen"]);
    config_empty(&mut bytes);
    let mut rec = input_rec("m", "a.js");
    // Byte 10 is the '.' inside the "a.js" content: 0xFF is never valid UTF-8.
    let at = rec.len() - 43;
    rec[at] = 0xff;
    inputs_section(&mut bytes, &[rec]);
    bytes.extend_from_slice(&assets_js_wasm("a.js", "b.wasm"));
    assert!(matches!(decode(&bytes), Err(InventoryError::InvalidUtf8)));
}

#[test]
fn decode_rejects_huge_length_as_truncated() {
    let mut bytes = head();
    providers_zero(&mut bytes);
    tools_named(&mut bytes, &["cargo", "rustc", "wasm-bindgen"]);
    config_empty(&mut bytes);
    u32be(1, &mut bytes);
    field(&mut bytes, b"m");
    field(&mut bytes, b"a.js");
    bytes.extend_from_slice(&u64::MAX.to_be_bytes());
    assert!(matches!(decode(&bytes), Err(InventoryError::Truncated)));
}

#[test]
fn decode_rejects_count_overflow() {
    let mut bytes = head();
    u32be(MAX_RECORDS as u32 + 1, &mut bytes);
    assert!(matches!(
        decode(&bytes),
        Err(InventoryError::TooManyRecords)
    ));
}

#[test]
fn decode_rejects_bad_tool_count() {
    let mut bytes = head();
    providers_zero(&mut bytes);
    tools_named(&mut bytes, &["cargo", "rustc"]);
    assert!(matches!(decode(&bytes), Err(InventoryError::BadToolSet)));
}

#[test]
fn decode_rejects_unknown_tags() {
    let mut with_tag = head();
    u32be(1, &mut with_tag);
    field(&mut with_tag, b"p");
    field(&mut with_tag, b"1.0");
    with_tag.push(7);
    assert!(matches!(
        decode(&with_tag),
        Err(InventoryError::UnknownTag(7))
    ));
}

#[test]
fn wasm_artifact_validation() {
    let mut good = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
    good.extend_from_slice(&[0u8; 16]);
    match validate_wasm_artifact(&good) {
        Ok(1) => {}
        _ => assert!(false, "valid header must report version 1"),
    }
    assert!(validate_wasm_artifact(&good[..7]).is_err());
    let mut bad = good.clone();
    bad[0] = 0x7f;
    assert!(validate_wasm_artifact(&bad).is_err());
}

#[test]
fn sha256_known_answer_and_hex() {
    let digest = digest_inventory(b"abc");
    assert_eq!(
        hex_digest(&digest),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        hex_digest(&digest_inventory(b"")),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn path_rule_table() {
    for good in ["a", "a/b/c.js", "dir/file-name_2.WASM", "x/y"] {
        assert!(check_path_for_test(good), "accept {good}");
    }
    assert!(!check_path_for_test(&"a".repeat(241)));
    for bad in [
        "", "/a", "a\\b", "a//b", "a/./b", "a/../b", "a/", "a ", "a.", "con", "COM1.txt", "nul/x",
        "a/b/COM9", "a\x01b",
    ] {
        assert!(!check_path_for_test(bad), "reject {bad:?}");
    }
}

fn check_path_for_test(path: &str) -> bool {
    super::check_path(path).is_ok()
}
