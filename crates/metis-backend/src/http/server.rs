//! Finite peer I/O lifecycle, with one owner of authenticated application state.

use super::{BrowserHttpService, http_io_error, peer_disconnected};
use metis_core::error::{ErrorCode, MetisError, Result};
use moirai_http::{AwaitingRequest, HttpConnection, HttpServer};
use std::{io, time::Duration};

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
    let mut first = Some(first);
    for _ in 0..max_requests {
        let connection = match first.take() {
            Some(connection) => connection,
            None => server
                .accept()
                .await
                .map_err(|error| http_io_error(&error))?,
        };
        let (request, connection) = match connection.read_request().await {
            Ok(request) => request,
            Err(error)
                if peer_disconnected(&error) || error.kind() == io::ErrorKind::InvalidData =>
            {
                on_connection_error(http_io_error(&error));
                continue;
            }
            Err(error) => return Err(http_io_error(&error)),
        };
        let response = application.respond(&request).map_err(|_| {
            MetisError::transport(ErrorCode::IoError, "HTTP response construction failed")
        })?;
        if let Some(delay) = response_delay {
            moirai_async::timer::sleep(delay).await;
        }
        if let Err(error) = connection.write_response(response).await {
            if !peer_disconnected(&error) {
                return Err(http_io_error(&error));
            }
            on_connection_error(http_io_error(&error));
        }
    }
    Ok(())
}
