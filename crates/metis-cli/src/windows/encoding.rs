//! A package's legacy codepage cannot change names through best-fit conversion.
//!
//! [WideCharToMultiByte](https://learn.microsoft.com/en-us/windows/win32/api/stringapiset/nf-stringapiset-widechartomultibyte)
//! requires `WC_NO_BEST_FIT_CHARS` for identity-bearing input. Without it the
//! native MSI writer persists infinity as `8` and CJK/emoji as replacement bytes.
use super::ffi;
use std::{error::Error, fmt, ptr};

pub(super) const CODE_PAGE: u32 = 1252;
const NO_BEST_FIT_CHARACTERS: u32 = 0x400;

#[derive(Debug)]
pub(super) struct UnsupportedMetadata;

impl fmt::Display for UnsupportedMetadata {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("MSI metadata cannot be represented losslessly in Windows-1252")
    }
}

impl Error for UnsupportedMetadata {}

pub(super) fn validate(encoded: &[u16]) -> Result<(), Box<dyn Error>> {
    if encoded.is_empty() {
        return Ok(());
    }
    let mut used_default = 0;
    // SAFETY: Input is live for its checked UTF-16-unit length. Zero output
    // capacity requests sizing only, so the null output pointer is never used.
    // used_default has BOOL-compatible writable storage. The fixed SBCS code
    // page permits this flag and reports every lossy/best-fit substitution.
    let length = unsafe {
        ffi::WideCharToMultiByte(
            CODE_PAGE,
            NO_BEST_FIT_CHARACTERS,
            encoded.as_ptr(),
            i32::try_from(encoded.len())?,
            ptr::null_mut(),
            0,
            ptr::null(),
            &raw mut used_default,
        )
    };
    if length == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    if used_default != 0 {
        return Err(UnsupportedMetadata.into());
    }
    Ok(())
}
