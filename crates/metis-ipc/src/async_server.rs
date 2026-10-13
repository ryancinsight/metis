//! Canonical asynchronous request dispatch over a bounded native message stream.

use crate::frame::split_frame;
use crate::server::{FailureContext, IpcHandler, dispatch_request};
use crate::session_transport::NativeSessionTransport;
use crate::transport::check_wire_size;
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::protocol::{MessageType, RemoteEventPayload, build_frame};
use std::time::Duration;

/// Largest response delay admitted by the browser conformance probe.
pub const MAX_CLINICAL_RESPONSE_DELAY: Duration = Duration::from_secs(30);

/// One ordered Metis IPC session carried by a bounded native message transport.
///
/// The server owns one receive/send stream and consumes each request sequence
/// before invoking the handler. A connection is therefore single-consumer and
/// replay-safe just like the synchronous pipe server.
pub struct AsyncIpcServer<T> {
    stream: T,
    last_sequence: u64,
    last_event_id: Option<u64>,
    clinical_response_delay: Option<Duration>,
    frame: Vec<u8>,
}

impl<T> AsyncIpcServer<T> {
    /// Takes ownership of an admitted native transport; callers establish peer trust.
    #[must_use]
    pub const fn new(stream: T) -> Self {
        Self {
            stream,
            last_sequence: 0,
            last_event_id: None,
            clinical_response_delay: None,
            frame: Vec::new(),
        }
    }

    /// Adds a bounded delay before a successful clinical response is sent.
    ///
    /// This probe exercises cancellation at a real service boundary. It uses
    /// Moirai's asynchronous timer, so the delay never blocks the executor;
    /// dropping the browser transport prevents the eventual frame from
    /// reaching a stopped host.
    /// Handshake, capability, rejection and event frames remain immediate.
    ///
    /// # Errors
    /// Returns [`ErrorCode::Timeout`] when `delay` exceeds
    /// [`MAX_CLINICAL_RESPONSE_DELAY`].
    pub fn with_clinical_response_delay(mut self, delay: Duration) -> Result<Self> {
        self.clinical_response_delay = validate_clinical_response_delay(delay)?;
        Ok(self)
    }
}

fn validate_clinical_response_delay(delay: Duration) -> Result<Option<Duration>> {
    if delay > MAX_CLINICAL_RESPONSE_DELAY {
        return Err(MetisError::transport(
            ErrorCode::Timeout,
            format!(
                "Clinical response delay exceeds the {} second probe bound",
                MAX_CLINICAL_RESPONSE_DELAY.as_secs()
            ),
        ));
    }
    Ok((!delay.is_zero()).then_some(delay))
}

impl<T: NativeSessionTransport> AsyncIpcServer<T> {
    /// Processes one request, or returns `false` for a peer close frame.
    ///
    /// The transport owns message bounds, framing cancellation and deadlines.
    /// This layer decodes exactly one Metis wire frame per message and applies
    /// the same sequence and response
    /// contracts as [`crate::IpcServer`].
    ///
    /// # Errors
    /// Returns malformed-frame, replay, handler, response, transport, or
    /// deadline failures. Clean peer closure is reported as `Ok(false)`.
    pub async fn step<H: IpcHandler>(&mut self, handler: &mut H) -> Result<bool> {
        let bytes = match self.stream.recv_frame().await {
            Ok(Some(bytes)) => bytes,
            Ok(None) => return Ok(false),
            Err(error) => {
                handler.handle_failure(FailureContext::Receive, error.code)?;
                return Err(error);
            }
        };
        let (header, payload) = match split_frame(bytes) {
            Ok(message) => message,
            Err(error) => {
                handler.handle_failure(FailureContext::Receive, error.code)?;
                return Err(error);
            }
        };
        let dispatched = dispatch_request(&mut self.last_sequence, handler, header, &payload)?;
        if let Err(error) = build_frame(
            dispatched.message_type,
            dispatched.identity.sequence,
            &dispatched.payload,
            &mut self.frame,
        ) {
            handler.handle_failure(FailureContext::Response(dispatched.identity), error.code)?;
            return Err(error);
        }
        if let Err(error) = check_wire_size(&self.frame) {
            handler.handle_failure(FailureContext::Response(dispatched.identity), error.code)?;
            return Err(error);
        }
        if dispatched.message_type == MessageType::ClinicalCalcResp
            && let Some(delay) = self.clinical_response_delay
        {
            moirai_async::timer::sleep(delay).await;
        }
        if let Err(error) = self.stream.send_frame(&self.frame).await {
            handler.handle_failure(FailureContext::Response(dispatched.identity), error.code)?;
            return Err(error);
        }
        if let Some(event) = handler.take_event()
            && let Err(error) = self.send_event(&event).await
        {
            handler.handle_failure(FailureContext::Event(event.event_id()), error.code)?;
            return Err(error);
        }
        Ok(true)
    }

    /// Services requests until the peer closes the session.
    ///
    /// The handler remains synchronous by contract. It runs only after one
    /// bounded native message has been received, so browser/event-loop
    /// callers use this API only on a native service task.
    ///
    /// # Errors
    /// Returns the first protocol, handler, response, transport, or deadline
    /// failure reported by [`Self::step`].
    pub async fn run<H: IpcHandler>(&mut self, handler: &mut H) -> Result<()> {
        while self.step(handler).await? {}
        self.stream.finish().await
    }

    /// Sends one unsolicited event with a strictly increasing identifier.
    ///
    /// The event uses the same bounded binary frame as pipe transports. Its
    /// identifier is consumed before the asynchronous write so an uncertain
    /// transmission cannot be retried as the same event.
    ///
    /// # Errors
    /// Returns payload, replay, frame-size, or native transport failures.
    pub async fn send_event(&mut self, event: &RemoteEventPayload) -> Result<()> {
        let event_id = event.event_id().get();
        if self.last_event_id.is_some_and(|last| event_id <= last) {
            return Err(MetisError::protocol(
                ErrorCode::ReplayDetected,
                "Remote event identifiers must increase strictly",
            ));
        }
        build_frame(
            MessageType::TelemetryStreamEvent,
            event_id,
            &event.encode()?,
            &mut self.frame,
        )?;
        check_wire_size(&self.frame)?;
        self.last_event_id = Some(event_id);
        self.stream.send_frame(&self.frame).await
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_CLINICAL_RESPONSE_DELAY, validate_clinical_response_delay};
    use metis_core::ErrorCode;
    use std::time::Duration;

    #[test]
    fn clinical_response_delay_is_bounded_and_zero_is_immediate() {
        assert_eq!(
            validate_clinical_response_delay(Duration::ZERO).expect("zero delay is valid"),
            None
        );
        let delay = Duration::from_secs(4);
        assert_eq!(
            validate_clinical_response_delay(delay).expect("bounded delay is valid"),
            Some(delay)
        );
        assert_eq!(
            validate_clinical_response_delay(MAX_CLINICAL_RESPONSE_DELAY)
                .expect("maximum delay is valid"),
            Some(MAX_CLINICAL_RESPONSE_DELAY)
        );
        assert_eq!(
            validate_clinical_response_delay(MAX_CLINICAL_RESPONSE_DELAY + Duration::from_nanos(1))
                .expect_err("over-bound delay must fail")
                .code,
            ErrorCode::Timeout
        );
    }
}
