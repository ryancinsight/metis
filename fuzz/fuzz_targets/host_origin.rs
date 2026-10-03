//! LibFuzzer target for `HostOrigin::parse`, the origin text a host policy
//! binds grants to.

#![no_main]

use libfuzzer_sys::fuzz_target;
use metis_core::HostOrigin;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data)
        && let Ok(origin) = HostOrigin::parse(text)
    {
        // Canonicalization is idempotent: the canonical text parses to itself.
        assert_eq!(HostOrigin::parse(&origin.to_string()).ok(), Some(origin));
    }
});
