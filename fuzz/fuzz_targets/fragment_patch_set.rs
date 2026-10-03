//! LibFuzzer target for `FragmentPatchSet::decode`, the untrusted
//! server-to-client fragment patch payload.

#![no_main]

use libfuzzer_sys::fuzz_target;
use metis_core::FragmentPatchSet;

fuzz_target!(|data: &[u8]| {
    if let Ok(set) = FragmentPatchSet::decode(data) {
        // A decoded set re-encodes to a payload that decodes to the same set.
        let encoded = set.encode().expect("invariant: a decoded set is encodable");
        assert_eq!(FragmentPatchSet::decode(&encoded).ok(), Some(set));
    }
});
