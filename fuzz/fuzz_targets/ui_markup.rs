//! LibFuzzer target for `parse_markup`, the application markup parser.

#![no_main]

use libfuzzer_sys::fuzz_target;
use metis_ui_lang::parse_markup;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        drop(std::hint::black_box(parse_markup(text)));
    }
});
