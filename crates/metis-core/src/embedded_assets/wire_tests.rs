//! Inventory contract tests using hand-built wire records.

use super::tests::sample;
use super::*;

fn u16be(value: u16, out: &mut Vec<u8>) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn u32be(value: u32, out: &mut Vec<u8>) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn field(out: &mut Vec<u8>, value: &[u8]) {
    let len = u32::try_from(value.len()).expect("test value fits u32");
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
    u32be(
        u32::try_from(tool_names.len()).expect("test value fits u32"),
        out,
    );
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
    u32be(
        u32::try_from(records.len()).expect("test value fits u32"),
        out,
    );
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
    tools_named(&mut bytes, &["rustc", "cargo", "wasm-bindgen"]);
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
    let over = u32::try_from(MAX_RECORDS).expect("test value fits u32") + 1;
    u32be(over, &mut bytes);
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
fn provider_ids_must_be_unique_on_both_paths() {
    let mut inv = sample();
    inv.providers[0].package_id = inv.providers[1].package_id.clone();
    assert_eq!(encode(&inv), Err(InventoryError::UnsortedRecords));

    let mut bytes = head();
    u32be(2, &mut bytes);
    for version in [b"1", b"2"] {
        field(&mut bytes, b"same-id");
        field(&mut bytes, version);
        bytes.push(1); // Registry
        bytes.extend_from_slice(&[0; 32]);
    }
    tools_named(&mut bytes, &["cargo", "rustc", "wasm-bindgen"]);
    config_empty(&mut bytes);
    inputs_section(&mut bytes, &[]);
    bytes.extend_from_slice(&assets_js_wasm("a.js", "b.wasm"));
    assert_eq!(decode(&bytes), Err(InventoryError::UnsortedRecords));
}

#[test]
fn tool_identities_are_checked_on_both_paths() {
    for (replacement, error) in [
        ("wasm-bindgen-cli", InventoryError::BadToolSet),
        ("rustc", InventoryError::UnsortedRecords),
    ] {
        let mut inv = sample();
        inv.tools[0].name = replacement.into();
        assert_eq!(encode(&inv), Err(error.clone()));
        let mut bytes = head();
        providers_zero(&mut bytes);
        tools_named(&mut bytes, &["cargo", "rustc", replacement]);
        config_empty(&mut bytes);
        inputs_section(&mut bytes, &[]);
        bytes.extend_from_slice(&assets_js_wasm("a.js", "b.wasm"));
        assert_eq!(decode(&bytes), Err(error));
    }
}

#[test]
fn duplicate_environment_pairs_are_rejected_on_both_paths() {
    let mut inv = sample();
    inv.config
        .environment
        .push(inv.config.environment[0].clone());
    assert_eq!(encode(&inv), Err(InventoryError::UnsortedRecords));

    let mut bytes = head();
    providers_zero(&mut bytes);
    tools_named(&mut bytes, &["cargo", "rustc", "wasm-bindgen"]);
    field(&mut bytes, b"release");
    u32be(0, &mut bytes); // Features
    u32be(0, &mut bytes); // Flags
    u32be(2, &mut bytes);
    for _ in 0..2 {
        field(&mut bytes, b"B");
        field(&mut bytes, b"2");
    }
    inputs_section(&mut bytes, &[]);
    bytes.extend_from_slice(&assets_js_wasm("a.js", "b.wasm"));
    assert_eq!(decode(&bytes), Err(InventoryError::UnsortedRecords));
}

/// Construct a complete manifest of the requested size with a single long
/// package name. The remaining fields are identical at both size boundaries.
fn sized_manifest(size: usize) -> Vec<u8> {
    let mut bytes = head();
    providers_zero(&mut bytes);
    tools_named(&mut bytes, &["cargo", "rustc", "wasm-bindgen"]);
    config_empty(&mut bytes);
    inputs_section(&mut bytes, &[]);
    bytes.extend_from_slice(&assets_js_wasm("a.js", "b.wasm"));
    let added = size - bytes.len();
    let field_start = INVENTORY_MAGIC.len() + size_of::<u16>();
    let payload_start = field_start + size_of::<u32>();
    bytes[field_start..payload_start].copy_from_slice(
        &u32::try_from(added)
            .expect("test size fits u32")
            .to_be_bytes(),
    );
    bytes.splice(
        payload_start..payload_start,
        std::iter::repeat_n(b'x', added),
    );
    bytes
}

#[test]
fn manifest_budget_is_inclusive_on_both_paths() {
    let bytes = sized_manifest(MAX_MANIFEST_BYTES);
    let mut inv = decode(&bytes).expect("exact limit");
    assert_eq!(encode(&inv).expect("re-encode exact limit"), bytes);
    inv.package.push('x');
    assert_eq!(encode(&inv).err(), Some(InventoryError::OversizeManifest));
    let oversized = sized_manifest(MAX_MANIFEST_BYTES + 1);
    assert_eq!(
        decode(&oversized).err(),
        Some(InventoryError::OversizeManifest)
    );
}
