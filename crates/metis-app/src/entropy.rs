//! Ephemeral symmetric session MAC-key generation through the operating system.
use metis_core::error::Result;

/// Generates one ephemeral symmetric session MAC key with no deterministic fallback.
///
/// # Errors
/// Returns the OS entropy failure or an unsupported-platform error.
pub fn session_key() -> Result<[u8; 32]> {
    let mut key = [0; 32];
    fill(&mut key)?;
    Ok(key)
}

#[cfg(windows)]
#[expect(unsafe_code, reason = "Minimal BCrypt OS entropy boundary")]
fn fill(key: &mut [u8; 32]) -> Result<()> {
    use metis_core::error::{ErrorCode, MetisError};
    use std::ffi::c_void;
    #[link(name = "bcrypt")]
    unsafe extern "system" {
        fn BCryptGenRandom(algorithm: *mut c_void, buffer: *mut u8, length: u32, flags: u32)
        -> i32;
    }
    // SAFETY: exclusive live 32-byte buffer. Flag 2 requires a null handle and
    // selects the OS provider. See Microsoft BCryptGenRandom parameter contract.
    let status = unsafe { BCryptGenRandom(std::ptr::null_mut(), key.as_mut_ptr(), 32, 2) };
    if status < 0 {
        return Err(MetisError::capability(
            ErrorCode::IoError,
            format!("OS entropy failed: {status:#x}"),
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn fill(key: &mut [u8; 32]) -> Result<()> {
    use std::io::Read;
    std::fs::File::open("/dev/urandom")?.read_exact(key)?;
    Ok(())
}

#[cfg(not(any(windows, unix)))]
fn fill(_key: &mut [u8; 32]) -> Result<()> {
    Err(metis_core::error::MetisError::capability(
        metis_core::error::ErrorCode::IoError,
        "No OS entropy implementation on this target",
    ))
}
