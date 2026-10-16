//! Per-test scoped tracing captures full arrange/act/assert trees.
pub(crate) fn traced(test: impl FnOnce()) {
    let subscriber = tracing_subscriber::fmt()
        .with_test_writer()
        .without_time()
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        let _test = tracing::info_span!("cli_asset_contract").entered();
        test();
    });
}
