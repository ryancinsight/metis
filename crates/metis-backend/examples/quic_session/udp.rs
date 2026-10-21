//! Caller-selected native queue requests; kernel defaults remain authoritative.

use moirai_quic::UdpBufferConfig;
use std::io;
use std::num::NonZeroUsize;

const RECEIVE: &str = "METIS_QUIC_RECEIVE_BUFFER_BYTES";
const SEND: &str = "METIS_QUIC_SEND_BUFFER_BYTES";

fn environment(name: &str) -> io::Result<Option<String>> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(io::Error::new(io::ErrorKind::InvalidInput, error)),
    }
}

fn request(name: &str, value: Option<&str>) -> io::Result<Option<NonZeroUsize>> {
    value
        .map(|value| {
            value.parse::<NonZeroUsize>().map_err(|error| {
                io::Error::new(io::ErrorKind::InvalidInput, format!("{name}: {error}"))
            })
        })
        .transpose()
}

fn parse(receive: Option<&str>, send: Option<&str>) -> io::Result<UdpBufferConfig> {
    let buffers = UdpBufferConfig {
        receive: request(RECEIVE, receive)?,
        send: request(SEND, send)?,
    };
    buffers.validate()?;
    Ok(buffers)
}

pub(super) fn buffers() -> io::Result<UdpBufferConfig> {
    let receive = environment(RECEIVE)?;
    let send = environment(SEND)?;
    parse(receive.as_deref(), send.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trace() -> tracing::subscriber::DefaultGuard {
        tracing::subscriber::set_default(
            tracing_subscriber::fmt()
                .with_test_writer()
                .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
                .finish(),
        )
    }

    #[test]
    fn independent_requests_and_absent_defaults_preserve_values() {
        let _trace = trace();
        let _test = tracing::info_span!("metis_udp_requests").entered();
        for (receive, send, expected_receive, expected_send) in [
            (None, None, None, None),
            (Some("65536"), None, Some(65_536), None),
            (None, Some("32768"), None, Some(32_768)),
            (Some("65536"), Some("32768"), Some(65_536), Some(32_768)),
            (
                Some("2147483647"),
                Some("1"),
                Some(usize::try_from(i32::MAX).expect("native maximum")),
                Some(1),
            ),
        ] {
            let actual = tracing::info_span!("act")
                .in_scope(|| parse(receive, send).expect("valid requests"));
            tracing::info_span!("assert").in_scope(|| {
                assert_eq!(actual.receive.map(NonZeroUsize::get), expected_receive);
                assert_eq!(actual.send.map(NonZeroUsize::get), expected_send);
            });
        }
    }

    #[test]
    fn invalid_requests_reject_zero_overflow_and_non_decimal_values() {
        let _trace = trace();
        for value in [
            "0",
            "-1",
            "",
            "3.5",
            "bytes",
            "2147483648",
            "18446744073709551616",
        ] {
            for (receive, send) in [(Some(value), None), (None, Some(value))] {
                let error = tracing::info_span!("act")
                    .in_scope(|| parse(receive, send).expect_err("invalid request"));
                tracing::info_span!("assert").in_scope(|| {
                    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
                    if value == "2147483648" {
                        assert_eq!(
                            error.to_string(),
                            "UDP buffer exceeds native signed integer"
                        );
                    } else {
                        let name = if receive.is_some() { RECEIVE } else { SEND };
                        assert!(
                            error.to_string().starts_with(name),
                            "input context: {error}"
                        );
                    }
                });
            }
        }
    }
}
