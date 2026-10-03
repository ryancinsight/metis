//! LibFuzzer target for `ComputedStyle::parse`, the inline style
//! declaration parser.

#![no_main]

use libfuzzer_sys::fuzz_target;
use metis_ui_lang::ComputedStyle;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        drop(std::hint::black_box(ComputedStyle::parse(text)));
    }
});
