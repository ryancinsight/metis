//! LibFuzzer target for the Linux package archive parser, the USTAR reader
//! `metis install` applies to a downloaded archive.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    std::hint::black_box(metis_fuzz::archive_file_count(data));
});
