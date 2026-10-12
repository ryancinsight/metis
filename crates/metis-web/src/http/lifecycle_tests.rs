use super::lifecycle::Lifecycle;

#[test]
fn replacement_and_reset_reject_every_prior_completion() {
    let _trace = super::test_support::trace();
    let _test = tracing::info_span!("request_replacement").entered();
    let arrange = tracing::info_span!("arrange").entered();
    let mut lifecycle = Lifecycle::new();
    drop(arrange);
    let act = tracing::info_span!("act").entered();
    let first = lifecycle.begin().expect("first request");
    drop(act);
    let _assert = tracing::info_span!("assert").entered();
    assert_eq!(first.mount(), 1);
    assert!(lifecycle.accepts(first));
    let second = lifecycle.begin().expect("replacement");
    assert!(!lifecycle.accepts(first));
    assert!(lifecycle.accepts(second));
    assert_eq!(lifecycle.reset().expect("reset"), 2);
    assert!(!lifecycle.accepts(second));
    let third = lifecycle.begin().expect("new mount");
    assert_eq!(third.mount(), 2);
    lifecycle.finish(second);
    assert!(lifecycle.accepts(third));
    lifecycle.finish(third);
    assert!(!lifecycle.accepts(third));
}

#[test]
fn request_and_mount_counters_never_wrap_or_accept_exhausted_leases() {
    let _trace = super::test_support::trace();
    let _test = tracing::info_span!("counter_exhaustion").entered();
    let arrange = tracing::info_span!("arrange").entered();
    let mut lifecycle = Lifecycle::at(u64::MAX, u64::MAX - 1);
    drop(arrange);
    let act = tracing::info_span!("act").entered();
    let last = lifecycle.begin().expect("last request");
    drop(act);
    let _assert = tracing::info_span!("assert").entered();
    assert_eq!(last.mount(), u64::MAX);
    assert!(lifecycle.accepts(last));
    assert_eq!(
        lifecycle.begin().expect_err("request exhaustion").kind(),
        std::io::ErrorKind::Other
    );
    assert!(!lifecycle.accepts(last));
    assert_eq!(
        lifecycle.reset().expect_err("mount exhaustion").kind(),
        std::io::ErrorKind::Other
    );
    assert_eq!(lifecycle.mount(), u64::MAX);
}
