//! Native message transport seam for one canonical asynchronous IPC server.

mod error;
use error::websocket_error;
use metis_core::Result;
use moirai_async::io::{AsyncRead, AsyncWrite};
use moirai_http::{WebSocketPeerClosed, WebSocketStream};
use std::future::Future;

/// An ordered native session transport with asynchronous, bounded I/O.
///
/// This service-side seam differs from the browser client trait: native writes
/// await stream backpressure, while browser host sends admit a bounded queue
/// immediately. Transport implementations own I/O deadlines and cancellation;
/// the server owns frame validation, replay, dispatch and response construction.
pub trait NativeSessionTransport {
    /// Send one complete Metis wire frame under the transport's finite deadline.
    ///
    /// # Errors
    /// Returns transport, size, deadline or cancellation failures.
    fn send_frame(&mut self, frame: &[u8]) -> impl Future<Output = Result<()>> + Send;

    /// Receive one wire frame; `None` denotes clean peer closure.
    ///
    /// # Errors
    /// Returns transport, size, deadline or truncation failures.
    fn recv_frame(&mut self) -> impl Future<Output = Result<Option<Vec<u8>>>> + Send;

    /// Finish the sending half after a clean peer closure.
    ///
    /// # Errors
    /// Returns transport, deadline or closed-stream failures.
    fn finish(&mut self) -> impl Future<Output = Result<()>> + Send;
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send> NativeSessionTransport for WebSocketStream<S> {
    async fn send_frame(&mut self, frame: &[u8]) -> Result<()> {
        self.send_binary(frame)
            .await
            .map_err(|error| websocket_error(&error))
    }

    async fn recv_frame(&mut self) -> Result<Option<Vec<u8>>> {
        match self.recv_message().await {
            Ok(frame) => Ok(Some(frame)),
            // Condition 1: std::io::Error erases the concrete I/O cause.
            Err(error)
                if error.kind() == std::io::ErrorKind::UnexpectedEof
                    && error.get_ref().is_some_and(
                        <dyn std::error::Error + Send + Sync + 'static>::is::<WebSocketPeerClosed>,
                    ) =>
            {
                Ok(None)
            }
            Err(error) => Err(websocket_error(&error)),
        }
    }

    async fn finish(&mut self) -> Result<()> {
        self.close(1000, b"")
            .await
            .map_err(|error| websocket_error(&error))
    }
}

#[cfg(feature = "quic")]
impl NativeSessionTransport for moirai_quic::QuicStream {
    async fn send_frame(&mut self, frame: &[u8]) -> Result<()> {
        crate::quic::validate_stream(self)?;
        moirai_quic::QuicStream::send_message(self, frame)
            .await
            .map_err(|error| crate::quic::quic_error(&error))
    }

    async fn recv_frame(&mut self) -> Result<Option<Vec<u8>>> {
        crate::quic::validate_stream(self)?;
        self.recv_message()
            .await
            .map_err(|error| crate::quic::quic_error(&error))
    }

    async fn finish(&mut self) -> Result<()> {
        self.finish_and_wait()
            .await
            .map_err(|error| crate::quic::quic_error(&error))
    }
}
