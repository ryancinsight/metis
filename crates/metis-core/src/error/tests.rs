//! Protocol discriminants preserve complete native representation and admission.
use super::{ErrorCode, MetisError, RedactedDiagnostic};

fn traced(test: impl FnOnce()) {
    let subscriber = tracing_subscriber::fmt()
        .with_test_writer()
        .without_time()
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        let _test = tracing::info_span!("error_wire_contract").entered();
        test();
    });
}

#[test]
fn debug_diagnostic_omits_untrusted_message_text() {
    traced(|| {
        let error = tracing::info_span!("arrange").in_scope(|| {
            MetisError::transport(
                ErrorCode::TransportBroken,
                "private path C:\\patients\\scan.bin",
            )
        });
        let (redacted, debug, display) = tracing::info_span!("act")
            .in_scope(|| (error.redacted(), format!("{error:?}"), format!("{error}")));
        tracing::info_span!("assert").in_scope(|| {
            assert_eq!(
                redacted,
                RedactedDiagnostic {
                    code: ErrorCode::TransportBroken,
                    trace_id: "REQ-METIS-IPC-005",
                }
            );
            assert!(debug.contains("TransportBroken"));
            assert!(debug.contains("REQ-METIS-IPC-005"));
            assert!(!debug.contains("scan.bin"));
            assert!(display.contains("scan.bin"));
        });
    });
}

#[test]
fn wire_codes_round_trip_and_reject_unknown_values() {
    traced(|| {
        let codes = tracing::info_span!("arrange").in_scope(|| {
            [
                ErrorCode::MagicMismatch,
                ErrorCode::VersionMismatch,
                ErrorCode::ChecksumMismatch,
                ErrorCode::PayloadTooLarge,
                ErrorCode::FrameTruncated,
                ErrorCode::UnexpectedMessageType,
                ErrorCode::MalformedPayload,
                ErrorCode::SequenceMismatch,
                ErrorCode::ReplayDetected,
                ErrorCode::MissingCapability,
                ErrorCode::InvalidCapabilitySignature,
                ErrorCode::CapabilityExpired,
                ErrorCode::InsufficientScope,
                ErrorCode::PrivilegeEscalationAttempt,
                ErrorCode::InvalidPrincipal,
                ErrorCode::InvalidOrigin,
                ErrorCode::InvalidWindow,
                ErrorCode::NavigationDenied,
                ErrorCode::InvalidPluginDescriptor,
                ErrorCode::PluginAlreadyRegistered,
                ErrorCode::PluginRegistryFull,
                ErrorCode::PluginNotFound,
                ErrorCode::PluginOperationNotFound,
                ErrorCode::PermissionDenied,
                ErrorCode::InvalidPatientWeight,
                ErrorCode::InvalidDrugConcentration,
                ErrorCode::InvalidTargetDose,
                ErrorCode::RateExceedsSafetyEnvelope,
                ErrorCode::PediatricRateExceeded,
                ErrorCode::NumericInstability,
                ErrorCode::ClinicalInterlockBlocked,
                ErrorCode::TransportBroken,
                ErrorCode::ConnectionClosed,
                ErrorCode::Timeout,
                ErrorCode::IoError,
                ErrorCode::QueueFull,
                ErrorCode::MalformedMarkup,
                ErrorCode::UnclosedTag,
                ErrorCode::TagMismatch,
                ErrorCode::InvalidCssStyle,
                ErrorCode::LayoutOverflow,
                ErrorCode::RenderFailure,
                ErrorCode::SurfaceAllocationError,
                ErrorCode::UnsupportedPlatformEvent,
            ]
        });
        let recovered = tracing::info_span!("act")
            .in_scope(|| codes.map(|code| ErrorCode::from_wire(code.to_wire())));
        tracing::info_span!("assert").in_scope(|| {
            for (code, actual) in codes.into_iter().zip(recovered) {
                assert_eq!(actual, Some(code));
            }
            assert_eq!(ErrorCode::from_wire(0), None);
            assert_eq!(ErrorCode::from_wire(u16::MAX), None);
        });
    });
}

#[test]
fn complete_wire_domain_preserves_native_bytes_and_rejects_every_undeclared_value() {
    traced(|| {
        let ranges = tracing::info_span!("arrange").in_scope(|| {
            [
                (0x1001_u16, 0x1009_u16),
                (0x2001, 0x200f),
                (0x3001, 0x3007),
                (0x4001, 0x4005),
                (0x5001, 0x5005),
                (0x6001, 0x6003),
            ]
        });
        let observed = tracing::info_span!("act").in_scope(|| {
            (u16::MIN..=u16::MAX)
                .map(|wire| {
                    let representation = ErrorCode::from_wire(wire).map(|code| {
                        let bytes: [u8; 2] = bytemuck::bytes_of(&code)
                            .try_into()
                            .expect("declared two-byte enum representation");
                        (code.to_wire(), bytes)
                    });
                    (wire, representation)
                })
                .collect::<Vec<_>>()
        });
        tracing::info_span!("assert").in_scope(|| {
            let domain_size = usize::from(u16::MAX) + 1;
            assert_eq!(observed.len(), domain_size);
            let expected_known = ranges
                .iter()
                .map(|(first, last)| usize::from(last - first) + 1)
                .sum::<usize>();
            let mut known = 0;
            let mut rejected = 0;
            for (wire, representation) in observed {
                let declared = ranges
                    .iter()
                    .any(|(first, last)| (*first..=*last).contains(&wire));
                if let Some((converted, bytes)) = representation {
                    assert!(declared, "unexpected admitted discriminant {wire:#06x}");
                    assert_eq!(converted, wire, "exact discriminant {wire:#06x}");
                    assert_eq!(bytes, wire.to_ne_bytes(), "native bits {wire:#06x}");
                    known += 1;
                } else {
                    assert!(!declared, "missing published discriminant {wire:#06x}");
                    rejected += 1;
                }
            }
            assert_eq!(known, expected_known);
            assert_eq!(rejected, domain_size - expected_known);
        });
    });
}

#[test]
fn wire_bit_copy_is_const_and_matches_the_two_byte_published_representation() {
    const PERMISSION: u16 = ErrorCode::PermissionDenied.to_wire();
    const TRANSPORT: u16 = ErrorCode::TransportBroken.to_wire();
    traced(|| {
        let expected = tracing::info_span!("arrange").in_scope(|| {
            [
                (ErrorCode::PermissionDenied, 0x200f),
                (ErrorCode::TransportBroken, 0x4001),
            ]
        });
        let computed = tracing::info_span!("act").in_scope(|| {
            expected.map(|(code, _)| {
                (
                    code.to_wire(),
                    bytemuck::bytes_of(&code).to_vec(),
                    ErrorCode::from_wire(code.to_wire()),
                )
            })
        });
        tracing::info_span!("assert").in_scope(|| {
            assert_eq!(std::mem::size_of::<ErrorCode>(), 2);
            assert_eq!(
                std::mem::align_of::<ErrorCode>(),
                std::mem::align_of::<u16>()
            );
            assert_eq!([PERMISSION, TRANSPORT], [0x200f, 0x4001]);
            for ((code, wire), (actual, bytes, recovered)) in expected.into_iter().zip(computed) {
                assert_eq!(actual, wire);
                assert_eq!(bytes, wire.to_ne_bytes());
                assert_eq!(recovered, Some(code));
            }
        });
    });
}
