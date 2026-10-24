//! Error display tests for native host.

use crate::native::application::NativeHostError;
use crate::native::application::tests::probe::ProbeError;
use std::io;

#[test]
fn host_error_display_retains_source_chain() {
    let surface = NativeHostError::<ProbeError>::Surface(io::Error::other("surface failure"));
    assert_eq!(surface.to_string(), "native surface error: surface failure");
    let application = NativeHostError::Application(ProbeError);
    assert_eq!(
        application.to_string(),
        "native application error: probe application rejected the event batch"
    );
}
