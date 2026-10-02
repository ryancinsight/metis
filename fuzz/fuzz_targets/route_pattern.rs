//! LibFuzzer target for `RoutePattern::parse`, the route table's pattern
//! text.

#![no_main]

use libfuzzer_sys::fuzz_target;
use metis_core::route::RoutePattern;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        drop(std::hint::black_box(RoutePattern::parse(text)));
    }
});
