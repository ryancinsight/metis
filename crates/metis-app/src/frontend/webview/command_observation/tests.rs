use super::*;
use tracing_subscriber::fmt::format::FmtSpan;

fn trace() -> tracing::subscriber::DefaultGuard {
    tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_test_writer()
            .with_span_events(FmtSpan::FULL)
            .finish(),
    )
}

fn actual_form_markup() -> &'static str {
    // Read the actual authored Rust raw literal without requiring a Windows
    // asset build in portable policy tests. Native extraction remains a gate.
    include_str!("../assets.rs")
        .split_once("const INDEX_HTML: &str = r#\"")
        .expect("actual packaged form constant")
        .1
        .split_once("\"#;")
        .expect("actual raw literal boundary")
        .0
}

#[test]
fn only_the_exact_selected_form_configuration_enables_document_observation() {
    let _trace = trace();
    let _test = tracing::info_span!("command_observation_selection").entered();
    let invalid = tracing::info_span!("arrange").in_scope(|| {
        [
            String::new(),
            "{}".into(),
            format!(" {PROVIDER_SELECTION}"),
            PROVIDER_SELECTION.replace("68", "83"),
            PROVIDER_SELECTION.replace("#result", "#theme-state"),
            PROVIDER_SELECTION.replace("metis.invalid", "outside.invalid"),
        ]
    });
    let selected = tracing::info_span!("act")
        .in_scope(|| selected_configuration(PageRole::Form, Some(OsStr::new(PROVIDER_SELECTION))));
    tracing::info_span!("assert").in_scope(|| {
        assert!(selected);
        assert_eq!(TARGET, "metis_app::webview::command_observation");
        assert_eq!(EVENT, "page_command_observation");
        for role in [PageRole::Form, PageRole::PermissionProbe] {
            assert!(!selected_configuration(role, None));
            for encoded in &invalid {
                assert!(!selected_configuration(role, Some(OsStr::new(encoded))));
            }
        }
        assert!(!selected_configuration(
            PageRole::PermissionProbe,
            Some(OsStr::new(PROVIDER_SELECTION))
        ));
        let actual: serde_json::Value =
            serde_json::from_str(PROVIDER_SELECTION).expect("canonical fixture selection");
        assert_eq!(actual["start_uri"], "https://metis.invalid/index.html");
        assert_eq!(
            actual["selectors"],
            serde_json::json!(["#host-status", "#result"])
        );
        assert_eq!(actual["late_input_after_key"], 68);
        assert_eq!(actual.as_object().expect("selection").len(), 3);
    });
}

#[test]
fn the_marker_changes_only_the_selected_actual_packaged_body() {
    let _trace = trace();
    let _test = tracing::info_span!("command_observation_body_marker").entered();
    let original = tracing::info_span!("arrange").in_scope(actual_form_markup);
    let (ordinary, selected) = tracing::info_span!("act").in_scope(|| {
        (
            mark_document(original.to_owned(), false).expect("ordinary markup"),
            mark_document(original.to_owned(), true).expect("selected markup"),
        )
    });
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(ordinary.as_bytes(), original.as_bytes());
        let attribute = format!("{MARKER_NAME}=\"{MARKER_VALUE}\" ");
        assert_eq!(selected.matches(&attribute).count(), 1);
        assert_eq!(
            selected.replace(&attribute, "").as_bytes(),
            original.as_bytes()
        );
        assert_eq!(selected.matches("<script ").count(), 1);
        for invalid in [
            String::new(),
            original.replace("<body ", "<section "),
            selected,
        ] {
            assert_eq!(
                mark_document(invalid, true)
                    .expect_err("unambiguous body required")
                    .kind(),
                io::ErrorKind::InvalidData
            );
        }
    });
}

#[path = "tests/receiver.rs"]
mod receiver;
