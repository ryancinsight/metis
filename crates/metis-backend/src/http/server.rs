//! Finite peer I/O lifecycle, with one owner of authenticated application state.

use super::{BrowserHttpService, http_io_error, peer_disconnected};
use metis_core::error::{ErrorCode, MetisError, Result};
use moirai_http::{
    AwaitingRequest, AwaitingResponse, HttpConnection, HttpRequest, HttpResponse, HttpServer,
};
use std::{
    future::{Future, poll_fn},
    io,
    pin::Pin,
    task::Poll,
    time::Duration,
};

struct ReadyRequest {
    request: HttpRequest,
    connection: HttpConnection<AwaitingResponse>,
}

enum PeerEvent {
    Read(io::Result<Box<ReadyRequest>>),
    Written(io::Result<()>),
}

enum Progress {
    Accepted(io::Result<HttpConnection<AwaitingRequest>>),
    Peer(usize, PeerEvent),
}

type PeerIo = Pin<Box<dyn Future<Output = PeerEvent> + Send>>;

fn read_peer(connection: HttpConnection<AwaitingRequest>) -> PeerIo {
    Box::pin(async move {
        PeerEvent::Read(
            connection
                .read_request()
                .await
                .map(|(request, connection)| {
                    Box::new(ReadyRequest {
                        request,
                        connection,
                    })
                }),
        )
    })
}

fn write_peer(
    connection: HttpConnection<AwaitingResponse>,
    response: HttpResponse,
    delay: Option<Duration>,
) -> PeerIo {
    Box::pin(async move {
        if let Some(delay) = delay {
            moirai_async::timer::sleep(delay).await;
        }
        PeerEvent::Written(connection.write_response(response).await)
    })
}

fn transport_error(error: &io::Error, phase: &'static str) -> MetisError {
    MetisError::transport(
        http_io_error(error).code,
        format!("HTTP {phase} transport terminated ({:?})", error.kind()),
    )
}

/// Starts after the first actual accept so tests can establish peer ordering
/// without sleeps, fake transports or changing the production read deadline.
pub(super) async fn serve_accepted(
    server: HttpServer,
    first: HttpConnection<AwaitingRequest>,
    mut application: BrowserHttpService,
    max_requests: usize,
    response_delay: Option<Duration>,
    mut on_connection_error: impl FnMut(MetisError),
) -> Result<()> {
    let mut peers = Vec::with_capacity(max_requests);
    peers.push(read_peer(first));
    let mut accepted = 1;
    let mut accept = (accepted < max_requests).then(|| Box::pin(server.accept()));
    while !peers.is_empty() || accepted < max_requests {
        let progress = poll_fn(|context| {
            for (index, peer) in peers.iter_mut().enumerate() {
                if let Poll::Ready(event) = peer.as_mut().poll(context) {
                    return Poll::Ready(Progress::Peer(index, event));
                }
            }
            if let Some(future) = accept.as_mut()
                && let Poll::Ready(result) = future.as_mut().poll(context)
            {
                return Poll::Ready(Progress::Accepted(result));
            }
            Poll::Pending
        })
        .await;
        match progress {
            Progress::Accepted(Ok(connection)) => {
                accepted += 1;
                peers.push(read_peer(connection));
                accept = (accepted < max_requests).then(|| Box::pin(server.accept()));
            }
            Progress::Accepted(Err(error))
                if error.kind() == io::ErrorKind::WouldBlock && !peers.is_empty() =>
            {
                // The provider's configured pool is full. Re-arm only after a
                // peer releases its slot; a request becoming ready retains it.
                accept = None;
            }
            Progress::Accepted(Err(error)) => return Err(transport_error(&error, "accept")),
            Progress::Peer(index, event) => {
                drop(peers.swap_remove(index));
                let released = match event {
                    PeerEvent::Read(Ok(ready)) => {
                        let ReadyRequest {
                            request,
                            connection,
                        } = *ready;
                        // The sole state owner responds synchronously. No
                        // application borrow survives a network/timer await.
                        let response = application.respond(&request).map_err(|_| {
                            MetisError::transport(
                                ErrorCode::IoError,
                                "HTTP response construction failed",
                            )
                        })?;
                        peers.push(write_peer(connection, response, response_delay));
                        false
                    }
                    PeerEvent::Read(Err(error))
                        if peer_disconnected(&error)
                            || error.kind() == io::ErrorKind::InvalidData =>
                    {
                        on_connection_error(transport_error(&error, "read"));
                        true
                    }
                    PeerEvent::Read(Err(error)) => return Err(transport_error(&error, "read")),
                    PeerEvent::Written(Ok(())) => true,
                    PeerEvent::Written(Err(error)) if peer_disconnected(&error) => {
                        on_connection_error(transport_error(&error, "write"));
                        true
                    }
                    PeerEvent::Written(Err(error)) => return Err(transport_error(&error, "write")),
                };
                if released && accept.is_none() && accepted < max_requests {
                    accept = Some(Box::pin(server.accept()));
                }
            }
        }
    }
    Ok(())
}
