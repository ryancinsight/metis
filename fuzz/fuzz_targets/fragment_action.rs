//! LibFuzzer target for `FragmentAction::decode`, the untrusted
//! client-to-server fragment action payload.

#![no_main]

use libfuzzer_sys::fuzz_target;
use metis_core::FragmentAction;

fuzz_target!(|data: &[u8]| {
    if let Ok(action) = FragmentAction::decode(data) {
        // A decoded action re-encodes to a payload that decodes to the same action.
        let encoded = action
            .encode()
            .expect("invariant: a decoded action is encodable");
        assert_eq!(FragmentAction::decode(&encoded).ok(), Some(action));
    }
});
