//! LibFuzzer target for `WindowState::decode`, the persisted window
//! rectangle record read back from disk.

#![no_main]

use libfuzzer_sys::fuzz_target;
use metis_core::window_state::WindowState;

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data)
        && let Ok(state) = WindowState::decode(text)
    {
        // `decode` reads what `encode` writes, so a decoded value round-trips.
        assert_eq!(WindowState::decode(&state.encode()).ok(), Some(state));
    }
});
