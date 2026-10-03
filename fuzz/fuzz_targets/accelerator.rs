//! LibFuzzer target for `Accelerator::parse`, the user-configurable
//! shortcut text.

#![no_main]

use libfuzzer_sys::fuzz_target;
use metis_core::input::Accelerator;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data)
        && let Ok(accelerator) = Accelerator::parse(text)
    {
        // The canonical text form is accepted by `parse` and names the same shortcut.
        assert_eq!(
            Accelerator::parse(&accelerator.to_string()).ok(),
            Some(accelerator)
        );
    }
});
