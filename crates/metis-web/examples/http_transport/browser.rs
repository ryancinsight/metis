//! Reads actual HTTP streams; the independent fixture generates analytical bytes.
use moirai_pal::wasm::{
    LocalTaskHandle, WebDocument, WebHttp, WebHttpMethod, spawn_local_with_handle,
};
use std::{cell::RefCell, io, time::Duration};

const BOUND: usize = metis_core::MAX_FRAGMENT_BODY_BYTES;
const SERVICE: &str = "http://127.0.0.1:8767";
thread_local! { static TASK: RefCell<Option<LocalTaskHandle>> = const { RefCell::new(None) }; }

/// Runs real streamed-byte, error, deadline and abort probes on the test origin.
#[expect(unsafe_code, reason = "raw WASM example entry ABI")]
#[unsafe(no_mangle)]
pub extern "C" fn metis_http_transport_start() {
    let Ok(document) = WebDocument::current() else {
        return;
    };
    let result_document = document.clone();
    let task = spawn_local_with_handle(async move {
        let result = probe().await;
        if let Some(status) = result_document.get_element_by_id("transport-status") {
            match result {
                Ok(()) => {
                    status.set_text("Rust Fetch verified: exact bytes · empty body · bounds · redirect refusal · deadline · cancellation");
                    let _ = status.set_attribute("data-result", "pass");
                }
                Err(error) => {
                    status.set_text(&format!("Rust Fetch rejected: {error}"));
                    let _ = status.set_attribute("data-result", "fail");
                }
            }
        }
        TASK.with_borrow_mut(Option::take);
    });
    TASK.with_borrow_mut(|slot| *slot = Some(task));
}

async fn get(path: &str, deadline: Duration) -> io::Result<moirai_pal::wasm::WebHttpResponse> {
    WebHttp::request(
        WebHttpMethod::Get,
        &format!("{SERVICE}{path}"),
        None,
        &[],
        BOUND,
        deadline,
    )
    .await
}

async fn probe() -> io::Result<()> {
    for status in [204, 205, 304] {
        let response = get(&format!("/bodyless/{status}"), Duration::from_secs(5)).await?;
        require(
            response.status == status && response.body.is_empty(),
            "Bodyless status changed or acquired response bytes",
        )?;
    }
    for size in [0, 1, BOUND] {
        let response = get(&format!("/bytes/{size}"), Duration::from_secs(5)).await?;
        require(
            response.status == 200 && response.body.len() == size,
            "byte count/status mismatch",
        )?;
        for (index, byte) in response.body.iter().enumerate() {
            require(
                *byte == u8::try_from(index % 251).map_err(io::Error::other)?,
                "streamed byte mismatch",
            )?;
        }
    }
    let body = "Δ Rust 🦀".as_bytes();
    let echoed = WebHttp::request(
        WebHttpMethod::Post,
        &format!("{SERVICE}/echo"),
        Some("application/octet-stream"),
        body,
        BOUND,
        Duration::from_secs(5),
    )
    .await?;
    require(
        echoed.status == 200 && echoed.body == body,
        "binary POST changed Unicode bytes",
    )?;
    for path in [
        format!("/bytes/{}", BOUND + 1),
        "/declared-excess".to_owned(),
    ] {
        let error = get(&path, Duration::from_secs(5))
            .await
            .err()
            .ok_or_else(|| io::Error::other("Oversized response was accepted"))?;
        require(
            error.kind() == io::ErrorKind::InvalidData,
            "oversized response lost its error kind",
        )?;
    }
    let redirected = get("/redirect", Duration::from_secs(5)).await;
    let error = redirected
        .err()
        .ok_or_else(|| io::Error::other("Fetch followed an unadmitted redirect"))?;
    require(
        error.kind() == io::ErrorKind::Other,
        "Redirect refusal lost its transport context",
    )?;
    let ledger = get("/redirect-result", Duration::from_secs(5)).await?;
    require(
        ledger.status == 200 && ledger.body == b"redirected=1;followed=0\n",
        "Peer observed redirect following",
    )?;
    // The fixture delays headers/body by 500 ms; this request's declared
    // 100 ms deadline covers the corresponding wait without a latency epsilon.
    for path in ["/slow-headers", "/slow-body"] {
        let error = get(path, Duration::from_millis(100))
            .await
            .err()
            .ok_or_else(|| io::Error::other("Deadline did not expire"))?;
        require(
            error.kind() == io::ErrorKind::TimedOut,
            "deadline lost its error kind",
        )?;
    }
    // Start an actual Fetch and drop its task while the peer holds the body.
    // The peer records connection closure; /abort-result returns that observed
    // fact rather than a test-side constant or simulated cancellation event.
    let handle = spawn_local_with_handle(async {
        let _ = get("/abort", Duration::from_secs(5)).await;
    });
    let ready = get("/abort-ready", Duration::from_secs(5)).await?;
    require(
        ready.status == 200 && ready.body == b"ready\n",
        "Peer did not enter the body wait",
    )?;
    drop(handle);
    let result = get("/abort-result", Duration::from_secs(5)).await?;
    require(
        result.status == 200 && result.body == b"closed\n",
        "peer did not observe Fetch cancellation",
    )?;
    Ok(())
}

fn require(value: bool, diagnostic: &'static str) -> io::Result<()> {
    if value {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::InvalidData, diagnostic))
    }
}
