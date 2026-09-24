//! Loopback server for a built browser application.
//!
//! The built tree is read into memory once, so a request is a map lookup: no
//! request path reaches the file system and no handler blocks the executor on
//! file I/O.
use crate::{Result, tree};
use moirai_core::{
    executor::{ExecutorControl, TaskSpawner},
    task::TaskHandle,
};
use moirai_executor::ExecutorBuilder;
use moirai_http::{
    AwaitingRequest, HttpConnection, HttpRequest, HttpResponse, HttpServer, ServerConfig,
};
use std::{
    collections::{BTreeMap, VecDeque},
    fs, io,
    path::Path,
    sync::Arc,
    time::Duration,
};

/// Tauri's development-server port, so a migrated project keeps its address.
pub(crate) const DEFAULT_PORT: u16 = 1420;
/// Concurrent connections: a browser opens up to six per origin, plus
/// speculative connections that may idle until the request deadline.
const CONNECTION_LIMIT: usize = 16;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const FILE_LIMIT: usize = 32 * 1024 * 1024;
const SITE_LIMIT: usize = 256 * 1024 * 1024;
/// Response budget beyond the body, for the status line and headers.
const HEADER_ALLOWANCE: usize = 4 * 1024;

/// A built page, frozen: served name to content type and bytes.
pub(crate) struct Site(BTreeMap<String, (&'static str, Arc<[u8]>)>);

impl Site {
    /// Reads every regular file below `root`; links and oversize files are refused.
    pub(crate) fn load(root: &Path) -> Result<Self> {
        let mut files = BTreeMap::new();
        let mut total = 0_usize;
        for path in tree::regular_files(root, |_| false)? {
            let name = tree::relative_name(root, &path)?;
            let bytes = fs::read(&path)?;
            if bytes.len() > FILE_LIMIT {
                return Err(format!("{name} exceeds the 32 MiB served-file budget").into());
            }
            total += bytes.len();
            if total > SITE_LIMIT {
                return Err("the built application exceeds the 256 MiB served budget".into());
            }
            let kind = content_type(&name);
            files.insert(name, (kind, bytes.into()));
        }
        if !files.contains_key("index.html") {
            return Err("the built application has no index.html".into());
        }
        Ok(Self(files))
    }

    /// The file for a `GET` of a served name; otherwise the status naming why not.
    pub(crate) fn respond(&self, request: &HttpRequest) -> io::Result<HttpResponse> {
        if request.method() != "GET" {
            let mut response = plain(405, "only GET is served\n")?;
            response.set_header("Allow", "GET")?;
            return Ok(response);
        }
        let Some(name) = resolve(request.target()) else {
            return plain(400, "malformed request target\n");
        };
        let Some((kind, bytes)) = self.0.get(&name) else {
            return plain(404, format!("{name} is not part of the build\n"));
        };
        let mut response = HttpResponse::new(200, bytes.to_vec())?;
        response.set_header("Content-Type", *kind)?;
        // Revalidate on every load, so a rebuilt page is never served stale.
        response.set_header("Cache-Control", "no-cache")?;
        response.set_header("X-Content-Type-Options", "nosniff")?;
        Ok(response)
    }
}

fn plain(status: u16, body: impl Into<Vec<u8>>) -> io::Result<HttpResponse> {
    let mut response = HttpResponse::new(status, body)?;
    response.set_header("Content-Type", "text/plain; charset=utf-8")?;
    Ok(response)
}

/// Maps an origin-form target to a served name: the query and fragment are
/// dropped, percent-escapes decode, and a directory resolves to its
/// `index.html`.
fn resolve(target: &str) -> Option<String> {
    let path = target.split(['?', '#']).next()?.strip_prefix('/')?;
    let mut bytes = Vec::with_capacity(path.len());
    let mut rest = path.bytes();
    while let Some(byte) = rest.next() {
        if byte == b'%' {
            let high = char::from(rest.next()?).to_digit(16)?;
            let low = char::from(rest.next()?).to_digit(16)?;
            bytes.push(u8::try_from(high * 16 + low).ok()?);
        } else {
            bytes.push(byte);
        }
    }
    let mut name = String::from_utf8(bytes).ok()?;
    if name.is_empty() || name.ends_with('/') {
        name.push_str("index.html");
    }
    Some(name)
}

fn content_type(name: &str) -> &'static str {
    let extension = name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase());
    match extension.as_deref() {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("txt" | "md") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Serves `site` on loopback `port` until the process is interrupted.
///
/// Each connection is its own task, at most [`CONNECTION_LIMIT`] at once; at
/// the limit the accept loop joins the oldest before accepting another.
pub(crate) fn serve(site: Site, name: &str, port: u16) -> Result<()> {
    let site = Arc::new(site);
    let executor = ExecutorBuilder::new()
        .worker_threads(2)
        .async_threads(2)
        .build()?;
    let config = ServerConfig {
        max_connections: CONNECTION_LIMIT,
        max_response_bytes: FILE_LIMIT + HEADER_ALLOWANCE,
        request_timeout: REQUEST_TIMEOUT,
        ..ServerConfig::default()
    };
    let server = executor.block_on(HttpServer::bind(&format!("127.0.0.1:{port}"), config))?;
    println!("{name}: http://{}/", server.local_addr()?);
    let mut in_flight: VecDeque<TaskHandle<()>> = VecDeque::with_capacity(CONNECTION_LIMIT);
    loop {
        let mut index = 0;
        while let Some(task) = in_flight.get(index) {
            if task.is_finished() {
                if let Some(task) = in_flight.remove(index) {
                    collect(task);
                }
            } else {
                index += 1;
            }
        }
        if in_flight.len() == CONNECTION_LIMIT
            && let Some(task) = in_flight.pop_front()
        {
            collect(task);
        }
        let connection = executor.block_on(server.accept())?;
        in_flight.push_back(executor.spawn_async(answer(connection, Arc::clone(&site)))?);
    }
}

/// Joins a connection task; a panic or lost result is reported, never dropped.
fn collect(task: TaskHandle<()>) {
    if task.join().is_none_or(|result| result.is_err()) {
        eprintln!("metis serve: a connection task ended without completing");
    }
}

async fn answer(connection: HttpConnection<AwaitingRequest>, site: Arc<Site>) {
    let result = async {
        let (request, connection) = connection.read_request().await?;
        connection.write_response(site.respond(&request)?).await
    }
    .await;
    // A browser closes idle or superseded connections; only other failures
    // say something about this server.
    if let Err(error) = result
        && !matches!(
            error.kind(),
            io::ErrorKind::UnexpectedEof
                | io::ErrorKind::BrokenPipe
                | io::ErrorKind::ConnectionReset
                | io::ErrorKind::ConnectionAborted
                | io::ErrorKind::TimedOut
        )
    {
        eprintln!("metis serve: {error}");
    }
}

#[cfg(test)]
mod tests;
