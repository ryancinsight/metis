//! The trusted clipboard plugin: the bounded `clipboard.write` operation.
//!
//! This is the counterpart of Tauri's clipboard-manager plugin inside the
//! Metis desktop authority model. The unprivileged frontend never touches
//! the operating system clipboard: it sends a bounded
//! [`ClipboardWritePayload`] through the plugin invocation boundary, the
//! service verifies the session's `CLIPBOARD` scope, and this executor
//! performs the write through [`metis_platform::ScopedClipboard`] with the
//! minted [`VerifiedHostCapability`] witness.

use crate::plugins::PluginExecutor;
use metis_core::capability::CapabilityScope;
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::host::VerifiedHostCapability;
use metis_core::protocol::{
    ClipboardWritePayload, ClipboardWriteResponse, Plugin, PluginDescriptor, PluginOperation,
};
use metis_platform::ScopedClipboard;

/// The single trusted operation this plugin declares.
pub const CLIPBOARD_OPERATIONS: [PluginOperation; 1] =
    [PluginOperation::new("write", CapabilityScope::CLIPBOARD)];

/// Host-side executor for the `clipboard.write` operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClipboardPlugin;

impl Plugin for ClipboardPlugin {
    const DESCRIPTOR: PluginDescriptor =
        PluginDescriptor::new("clipboard", 1, &CLIPBOARD_OPERATIONS, &[]);
}

impl PluginExecutor for ClipboardPlugin {
    fn invoke(&mut self, _operation_name: &str, _body: &[u8]) -> Result<Vec<u8>> {
        // The router only reaches this executor through `invoke_authorized`;
        // an unwitnessed clipboard call would bypass the scope check.
        Err(MetisError::capability(
            ErrorCode::MissingCapability,
            "Clipboard operations require an authorized invocation path",
        ))
    }

    fn invoke_authorized(
        &mut self,
        operation_name: &str,
        body: &[u8],
        clipboard: Option<&VerifiedHostCapability<{ CapabilityScope::CLIPBOARD.0 }>>,
    ) -> Result<Vec<u8>> {
        if operation_name != "write" {
            return Err(MetisError::capability(
                ErrorCode::PluginOperationNotFound,
                "Clipboard plugin declares only the write operation",
            ));
        }
        let witness = clipboard.ok_or_else(|| {
            MetisError::capability(
                ErrorCode::MissingCapability,
                "Clipboard write requires the minted CLIPBOARD witness",
            )
        })?;
        let request = ClipboardWritePayload::decode(body)?;
        let provider = ScopedClipboard;
        let carried = provider.write_text(witness, request.text())?;
        let count = u32::try_from(carried).map_err(|_| {
            MetisError::protocol(
                ErrorCode::PayloadTooLarge,
                "Clipboard write count cannot fit its response field",
            )
        })?;
        Ok(ClipboardWriteResponse::new(count)?.encode())
    }
}

#[cfg(test)]
mod tests {
    use super::ClipboardPlugin;
    use crate::plugins::PluginExecutor;
    use metis_core::capability::{CapabilityGrantSpec, CapabilityScope};
    use metis_core::error::ErrorCode;
    use metis_core::host::{
        HostOrigin, HostPolicy, HostSessionId, VerifiedHostCapability, WindowId,
    };
    use metis_core::protocol::{
        ClipboardWritePayload, ClipboardWriteResponse, MAX_CLIPBOARD_WRITE_BYTES,
    };
    use metis_platform::MAX_CLIPBOARD_TEXT_BYTES;

    const KEY: &[u8] = b"metis-clipboard-plugin-test-key";

    fn witness() -> VerifiedHostCapability<{ CapabilityScope::CLIPBOARD.0 }> {
        let policy = HostPolicy::new(
            HostOrigin::parse("http://127.0.0.1:8080").expect("test origin"),
            WindowId::new(1).expect("test window"),
        );
        let session = HostSessionId::new([5; 16]).expect("test session");
        let context = policy.context_for(session);
        let token = context
            .issue_capability(
                CapabilityGrantSpec {
                    token_id: 1,
                    principal_id: session.as_bytes(),
                    scope: CapabilityScope::CLIPBOARD,
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
    fn the_write_bound_matches_the_provider_bound() {
        assert_eq!(MAX_CLIPBOARD_WRITE_BYTES, MAX_CLIPBOARD_TEXT_BYTES);
    }

    #[test]
    fn unwitnessed_and_unexpected_invocations_fail_closed() {
        let body = ClipboardWritePayload::new("Métis clipboard")
            .expect("bounded request")
            .encode()
            .expect("encode");
        let unwitnessed = ClipboardPlugin
            .invoke_authorized("write", &body, None)
            .expect_err("unwitnessed clipboard call");
        assert_eq!(unwitnessed.code, ErrorCode::MissingCapability);

        let witnessed_elsewhere =
            ClipboardPlugin.invoke_authorized("read", &body, Some(&witness()));
        assert_eq!(
            witnessed_elsewhere.expect_err("undeclared operation").code,
            ErrorCode::PluginOperationNotFound
        );

        let unwitnessed_path = ClipboardPlugin.invoke("write", &body);
        assert_eq!(
            unwitnessed_path.expect_err("unscoped invocation path").code,
            ErrorCode::MissingCapability
        );
    }

    #[test]
    #[cfg(windows)]
    #[ignore = "replaces the real system clipboard; run explicitly on an owned host"]
    fn witnessed_write_carries_the_declared_byte_count() {
        let witness = witness();
        let text = "Métis 🧩 clipboard plugin round trip";
        let body = ClipboardWritePayload::new(text)
            .expect("bounded request")
            .encode()
            .expect("encode");
        let carried = ClipboardPlugin
            .invoke_authorized("write", &body, Some(&witness))
            .expect("authorized clipboard write");
        assert_eq!(
            ClipboardWriteResponse::decode(&carried)
                .expect("decode response")
                .bytes(),
            u32::try_from(text.len()).expect("bounded text fits u32")
        );
    }
}
