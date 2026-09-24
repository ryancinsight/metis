use super::*;
use std::io::{Read, Write};

struct Built(std::path::PathBuf);

impl Built {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "metis-serve-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("test clock")
                .as_nanos()
        ));
        fs::create_dir_all(root.join("assets")).expect("site root");
        fs::write(root.join("index.html"), b"<!doctype html><title>t</title>").expect("page");
        fs::write(root.join("module_bg.wasm"), b"\0asm\x01\0\0\0").expect("module");
        fs::write(root.join("assets/logo.svg"), b"<svg/>").expect("image");
        fs::write(root.join("a b.txt"), b"spaced").expect("spaced name");
        Self(root)
    }
}

impl Drop for Built {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("test cleanup");
    }
}

/// Sends `request` over loopback and returns the status, headers and body.
fn exchange(site: &Site, request: &str) -> (u16, Vec<(String, String)>, Vec<u8>) {
    let executor = ExecutorBuilder::new()
        .worker_threads(1)
        .async_threads(1)
        .build()
        .expect("executor");
    let server = executor
        .block_on(HttpServer::bind("127.0.0.1:0", ServerConfig::default()))
        .expect("bind");
    let address = server.local_addr().expect("address");
    let request = request.to_owned();
    let client = std::thread::spawn(move || {
        let mut stream = std::net::TcpStream::connect(address).expect("connect");
        stream.write_all(request.as_bytes()).expect("request");
        let mut reply = Vec::new();
        stream.read_to_end(&mut reply).expect("reply");
        reply
    });
    executor
        .block_on(async {
            let connection = server.accept().await?;
            let (request, connection) = connection.read_request().await?;
            connection.write_response(site.respond(&request)?).await
        })
        .expect("exchange");
    let reply = client.join().expect("client thread");
    let split = reply
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("header terminator");
    let head = std::str::from_utf8(&reply[..split]).expect("ASCII head");
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse().ok())
        .expect("status line");
    let headers = lines
        .filter_map(|line| line.split_once(": "))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.to_owned()))
        .collect();
    (status, headers, reply[split + 4..].to_vec())
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(candidate, _)| candidate == name)
        .map(|(_, value)| value.as_str())
}

#[test]
fn served_files_carry_their_bytes_and_type() {
    let built = Built::new();
    let site = Site::load(&built.0).expect("site");
    for (request, body, kind) in [
        (
            "GET / HTTP/1.1\r\nHost: t\r\n\r\n",
            &b"<!doctype html><title>t</title>"[..],
            "text/html; charset=utf-8",
        ),
        (
            "GET /module_bg.wasm HTTP/1.1\r\nHost: t\r\n\r\n",
            b"\0asm\x01\0\0\0",
            "application/wasm",
        ),
        (
            "GET /assets/logo.svg?v=2 HTTP/1.1\r\nHost: t\r\n\r\n",
            b"<svg/>",
            "image/svg+xml",
        ),
        (
            "GET /a%20b.txt HTTP/1.1\r\nHost: t\r\n\r\n",
            b"spaced",
            "text/plain; charset=utf-8",
        ),
    ] {
        let (status, headers, received) = exchange(&site, request);
        assert_eq!(status, 200, "{request}");
        assert_eq!(received, body, "{request}");
        assert_eq!(header(&headers, "content-type"), Some(kind), "{request}");
        assert_eq!(header(&headers, "cache-control"), Some("no-cache"));
        assert_eq!(header(&headers, "x-content-type-options"), Some("nosniff"));
    }
}

#[test]
fn requests_outside_the_build_are_refused_by_status() {
    let built = Built::new();
    fs::write(built.0.with_extension("secret"), b"outside").expect("sibling file");
    let site = Site::load(&built.0).expect("site");
    let secret = format!(
        "GET /../{} HTTP/1.1\r\nHost: t\r\n\r\n",
        built
            .0
            .with_extension("secret")
            .file_name()
            .and_then(|name| name.to_str())
            .expect("name")
    );
    for (request, expected) in [
        ("GET /missing.js HTTP/1.1\r\nHost: t\r\n\r\n", 404),
        (secret.as_str(), 404),
        ("GET /%2e%2e/secret HTTP/1.1\r\nHost: t\r\n\r\n", 404),
        ("GET /%zz HTTP/1.1\r\nHost: t\r\n\r\n", 400),
        ("GET /%ff HTTP/1.1\r\nHost: t\r\n\r\n", 400),
    ] {
        let (status, _, body) = exchange(&site, request);
        assert_eq!(status, expected, "{request}");
        assert!(!body.windows(7).any(|window| window == b"outside"));
    }
    let (status, headers, _) = exchange(
        &site,
        "POST / HTTP/1.1\r\nHost: t\r\nContent-Length: 0\r\n\r\n",
    );
    assert_eq!(status, 405);
    assert_eq!(header(&headers, "allow"), Some("GET"));
    fs::remove_file(built.0.with_extension("secret")).expect("test cleanup");
}

#[test]
fn targets_resolve_directories_to_their_index() {
    assert_eq!(resolve("/").as_deref(), Some("index.html"));
    assert_eq!(
        resolve("/docs/?q=1#top").as_deref(),
        Some("docs/index.html")
    );
    assert_eq!(resolve("/a%2Fb").as_deref(), Some("a/b"));
    assert_eq!(resolve("relative"), None);
    assert_eq!(resolve("/%4"), None);
}

#[test]
fn a_build_without_a_page_is_not_served() {
    let built = Built::new();
    fs::remove_file(built.0.join("index.html")).expect("remove page");
    let error = Site::load(&built.0).err().expect("missing index.html");
    assert!(error.to_string().contains("no index.html"), "{error}");
}
