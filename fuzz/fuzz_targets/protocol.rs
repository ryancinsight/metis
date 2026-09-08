//! LibFuzzer target for every public Metis wire decoder.
//!
//! All decoders validate their own byte and allocation bounds. The target
//! intentionally accepts arbitrary bytes and records no successful value; a
//! panic, abort, or unbounded allocation is the failure oracle.

#![no_main]

use libfuzzer_sys::fuzz_target;
use metis_core::protocol::{
    CapabilityCatalogPayload, ClinicalCalcRequestPayload, ClinicalCalcResponsePayload,
    ErrorResponsePayload, FrameHeader, HEADER_SIZE, HandshakeRequestPayload,
    HandshakeResponsePayload, PluginInvocationPayload, PluginInvocationResponsePayload,
    RemoteEventPayload, TargetCapabilityPayload,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// Include the packaging parser itself so this campaign exercises the exact
// source that admits user-supplied SVG resources without adding the CLI's
// Windows-only binary to the fuzz workspace.
#[path = "../../crates/metis-cli/src/manifest/svg.rs"]
#[expect(
    dead_code,
    reason = "fuzz input calls byte validation, not file loading"
)]
mod svg;

fn consume<T>(value: T) {
    drop(std::hint::black_box(value));
}

fuzz_target!(|data: &[u8]| {
    consume(svg::validate(data));
    consume(HandshakeRequestPayload::decode(data));
    consume(HandshakeResponsePayload::decode(data));
    consume(ClinicalCalcRequestPayload::decode(data));
    consume(ClinicalCalcResponsePayload::decode(data));
    consume(ErrorResponsePayload::decode(data));
    consume(PluginInvocationPayload::decode(data));
    consume(PluginInvocationResponsePayload::decode(data));
    consume(CapabilityCatalogPayload::decode(data));
    consume(TargetCapabilityPayload::decode(data));
    if let Ok(event) = RemoteEventPayload::decode(data) {
        consume(event.decode_as::<ClinicalCalcResponsePayload>());
    }
    if data.len() >= HEADER_SIZE {
        let header: [u8; HEADER_SIZE] = data[..HEADER_SIZE]
            .try_into()
            .expect("invariant: the header slice has HEADER_SIZE bytes");
        consume(FrameHeader::decode(&header));
    }
});
