//! Capability-witnessed access to the system text clipboard.
//!
//! This is the counterpart of Tauri's clipboard-manager plugin. The provider
//! handles UTF-8 text only, through Moirai's bounded `CF_UNICODETEXT`
//! mechanism: reads stop at the provider byte budget, writes refuse empty or
//! oversized text before the clipboard is touched, and every call needs a
//! [`VerifiedHostCapability`] for [`CapabilityScope::CLIPBOARD`] so authority
//! cannot be omitted accidentally. The provider never executes, transforms or
//! stores clipboard content.

use metis_core::capability::CapabilityScope;
use metis_core::host::VerifiedHostCapability;
use std::io;

/// Maximum UTF-8 bytes one provider read or write may carry. The Windows
/// mechanism's UTF-16 unit bound restricts the same text from its own side,
/// so the two bounds combined keep every journey allocation-finite.
pub const MAX_CLIPBOARD_TEXT_BYTES: usize = 1 << 20;

/// Usage of the system text clipboard under the [`CapabilityScope::CLIPBOARD`]
/// witness.
///
/// The provider keeps no state: authorization happens at each call, and the
/// platform mechanism has already bounded every operation before this module
/// admits it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScopedClipboard;

impl ScopedClipboard {
    /// Reads the clipboard's text content when it holds text.
    ///
    /// The capability witness must have been produced by the trusted
    /// [`metis_core::host::HostPolicy`] for `CapabilityScope::CLIPBOARD`.
    ///
    /// # Errors
    /// Returns [`io::ErrorKind::InvalidInput`] for a read budget over
    /// [`MAX_CLIPBOARD_TEXT_BYTES`], the platform mechanism error otherwise
    /// (an unavailable clipboard is `Ok(None)`, and text over the read
    /// budget is [`io::ErrorKind::Unsupported`]).
    pub fn read_text(
        &self,
        _capability: &VerifiedHostCapability<{ CapabilityScope::CLIPBOARD.0 }>,
        budget: usize,
    ) -> io::Result<Option<String>> {
        if budget > MAX_CLIPBOARD_TEXT_BYTES {
            return Err(invalid("clipboard read budget exceeds the provider bound"));
        }
        let text = clipboard_read_text()?;
        let Some(text) = text else {
            return Ok(None);
        };
        if text.len() > budget {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "clipboard text exceeds the read budget",
            ));
        }
        Ok(Some(text))
    }

    /// Places `text` on the system clipboard as plain text, replacing
    /// whatever was there, and returns the UTF-8 byte count carried.
    ///
    /// The text is admitted as supplied: no normalization, shell or command
    /// interpretation exists between the caller and the clipboard bytes.
    ///
    /// # Errors
    /// Returns [`io::ErrorKind::InvalidInput`] for empty text or text over
    /// [`MAX_CLIPBOARD_TEXT_BYTES`], and the platform mechanism error
    /// otherwise.
    pub fn write_text(
        &self,
        _capability: &VerifiedHostCapability<{ CapabilityScope::CLIPBOARD.0 }>,
        text: impl AsRef<str>,
    ) -> io::Result<usize> {
        let text = text.as_ref();
        if text.is_empty() || text.len() > MAX_CLIPBOARD_TEXT_BYTES {
            return Err(invalid(
                "clipboard text is empty or exceeds the provider bound",
            ));
        }
        clipboard_write_text(text)
    }
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(all(windows, feature = "clipboard"))]
fn clipboard_read_text() -> io::Result<Option<String>> {
    moirai_pal::windows::clipboard::read_text()
}

#[cfg(all(windows, feature = "clipboard"))]
fn clipboard_write_text(text: &str) -> io::Result<usize> {
    moirai_pal::windows::clipboard::write_text(text)
}

#[cfg(any(not(windows), not(feature = "clipboard")))]
fn clipboard_read_text() -> io::Result<Option<String>> {
    Err(unavailable())
}

#[cfg(any(not(windows), not(feature = "clipboard")))]
fn clipboard_write_text(_text: &str) -> io::Result<usize> {
    Err(unavailable())
}

#[cfg(any(not(windows), not(feature = "clipboard")))]
fn unavailable() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "the clipboard provider needs a Moirai release carrying the windows clipboard mechanism (enable the `clipboard` feature)",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use metis_core::capability::{CapabilityGrantSpec, CapabilityScope};
    use metis_core::host::{HostOrigin, HostPolicy, HostSessionId, WindowId};

    const KEY: &[u8] = b"metis-scoped-clipboard-test-key";

    fn capability(
        scope: CapabilityScope,
    ) -> VerifiedHostCapability<{ CapabilityScope::CLIPBOARD.0 }> {
        let policy = HostPolicy::new(
            HostOrigin::parse("http://127.0.0.1:8080").expect("test origin"),
            WindowId::new(1).expect("test window"),
        );
        let session = HostSessionId::new([9; 16]).expect("test session");
        let context = policy.context_for(session);
        let token = context
            .issue_capability(
                CapabilityGrantSpec {
                    token_id: 1,
                    principal_id: session.as_bytes(),
                    scope,
                    issued_at_secs: 10,
                    duration_secs: 100,
                    nonce: 1,
                },
                KEY,
            )
            .expect("test token");
        policy
            .authorize::<{ CapabilityScope::CLIPBOARD.0 }>(&token, &context, 11, KEY)
            .expect("clipboard capability")
    }

    #[test]
    fn oversized_values_are_refused_before_the_clipboard_is_touched() {
        let clipboard = ScopedClipboard;
        let capability = capability(CapabilityScope::CLIPBOARD);
        let budget = clipboard
            .read_text(&capability, MAX_CLIPBOARD_TEXT_BYTES + 1)
            .expect_err("budget over the provider bound");
        assert_eq!(budget.kind(), io::ErrorKind::InvalidInput);

        let long = "\u{2022}".repeat(MAX_CLIPBOARD_TEXT_BYTES + 1);
        let write = clipboard
            .write_text(&capability, long)
            .expect_err("text over the provider bound");
        assert_eq!(write.kind(), io::ErrorKind::InvalidInput);

        let empty = clipboard
            .write_text(&capability, "")
            .expect_err("empty text");
        assert_eq!(empty.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn the_read_budget_stays_inside_the_provider_bound() {
        let clipboard = ScopedClipboard;
        let capability = capability(CapabilityScope::CLIPBOARD);
        // The real clipboard may hold anything; the outcome space is the
        // provider contract, never a panic.
        match clipboard.read_text(&capability, MAX_CLIPBOARD_TEXT_BYTES) {
            Ok(None | Some(_)) => {}
            Err(error) => assert!(
                matches!(
                    error.kind(),
                    io::ErrorKind::Unsupported | io::ErrorKind::Interrupted | io::ErrorKind::Other
                ),
                "unexpected read error: {error}"
            ),
        }
    }

    #[test]
    fn host_policy_denies_clipboard_calls_without_clipboard_scope() {
        assert_eq!(CapabilityScope::CLIPBOARD, CapabilityScope(1 << 9));
        let policy = HostPolicy::new(
            HostOrigin::parse("http://127.0.0.1:8080").expect("test origin"),
            WindowId::new(1).expect("test window"),
        );
        let session = HostSessionId::new([8; 16]).expect("test session");
        let context = policy.context_for(session);
        let token = context
            .issue_capability(
                CapabilityGrantSpec {
                    token_id: 2,
                    principal_id: session.as_bytes(),
                    scope: CapabilityScope::UI_RENDER,
                    issued_at_secs: 10,
                    duration_secs: 100,
                    nonce: 2,
                },
                KEY,
            )
            .expect("test token");
        let error = policy
            .authorize::<{ CapabilityScope::CLIPBOARD.0 }>(&token, &context, 11, KEY)
            .expect_err("insufficient scope must not authorize");
        assert_eq!(error.code, metis_core::error::ErrorCode::InsufficientScope);
    }

    #[test]
    #[cfg(all(windows, feature = "clipboard"))]
    #[ignore = "replaces the real system clipboard; run explicitly on an owned host with --features clipboard and a Moirai lock carrying windows::clipboard"]
    fn text_round_trips_through_the_real_clipboard() {
        let clipboard = ScopedClipboard;
        let capability = capability(CapabilityScope::CLIPBOARD);
        let original = clipboard
            .read_text(&capability, MAX_CLIPBOARD_TEXT_BYTES)
            .expect("read original")
            .unwrap_or_default();
        let text = "M\u{e9}tis \u{1f9e9} scoped clipboard round trip";
        let carried = clipboard.write_text(&capability, text).expect("write");
        assert_eq!(carried, text.len());
        let read = clipboard
            .read_text(&capability, MAX_CLIPBOARD_TEXT_BYTES)
            .expect("read back");
        assert_eq!(read.as_deref(), Some(text));
        if original.is_empty() {
            let _ = clipboard.write_text(&capability, "M\u{e9}tis scoped clipboard sentinel");
        } else {
            let _ = clipboard.write_text(&capability, &original);
        }
    }
}
