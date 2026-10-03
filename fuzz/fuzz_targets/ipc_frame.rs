//! LibFuzzer target for `read_frame`, the length-prefixed frame reader over
//! an untrusted byte stream.

#![no_main]

use libfuzzer_sys::fuzz_target;
use metis_ipc::frame::read_frame;

fuzz_target!(|data: &[u8]| {
    let mut stream = data;
    // Each accepted frame consumes at least its header, so the loop ends
    // within the input length.
    while let Ok((header, payload)) = read_frame(&mut stream) {
        assert_eq!(
            usize::try_from(header.payload_len).ok(),
            Some(payload.len()),
            "a returned payload has the length its header declares"
        );
    }
});
