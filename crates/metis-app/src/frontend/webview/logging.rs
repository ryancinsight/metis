//! Preserve ordinary tracing while selecting private native startup observations.
//!
//! Normal records retain their INFO filter and full span context. Only the
//! dedicated fixture target gains DEBUG admission. Its separate JSON formatter
//! omits time and span context, giving the provider a fixed envelope for its
//! aggregate escaped-output bound. Neither layer changes page admission.

use tracing::{Level, Subscriber};
use tracing_subscriber::{
    Layer,
    filter::filter_fn,
    fmt::{MakeWriter, format::FmtSpan},
    layer::SubscriberExt,
};

const TARGET: &str = "moirai_pal::windows::webview::startup";

#[cfg(windows)]
pub(super) fn subscriber() -> impl Subscriber + Send + Sync {
    let selected = std::env::var_os("MOIRAI_WEBVIEW_STARTUP_OBSERVATION").is_some();
    with_writer(std::io::stderr, selected)
}

fn with_writer<W>(writer: W, selected: bool) -> impl Subscriber + Send + Sync
where
    W: for<'writer> MakeWriter<'writer> + Clone + Send + Sync + 'static,
{
    let ordinary = tracing_subscriber::fmt::layer()
        .json()
        .without_time()
        .with_ansi(false)
        .with_writer(writer.clone())
        .with_span_events(FmtSpan::FULL)
        .with_filter(filter_fn(|metadata| {
            metadata.level() <= &Level::INFO && metadata.target() != TARGET
        }));
    let diagnostic = tracing_subscriber::fmt::layer()
        .json()
        .without_time()
        .with_ansi(false)
        .with_writer(writer)
        .with_current_span(false)
        .with_span_list(false)
        .with_span_events(FmtSpan::NONE)
        .with_filter(filter_fn(move |metadata| {
            selected && metadata.level() <= &Level::DEBUG && metadata.target() == TARGET
        }));
    tracing_subscriber::registry()
        .with(ordinary)
        .with(diagnostic)
}

#[cfg(test)]
#[path = "logging/tests.rs"]
mod tests;
