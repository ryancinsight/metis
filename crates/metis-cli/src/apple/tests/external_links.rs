//! Bundle authority uses the same exact destinations as the validated manifest.
use super::*;

fn application() -> WebApplication {
    serde_json::from_str(include_str!("../../../../metis-starter/metis.json"))
        .expect("real starter manifest")
}

#[test]
fn every_apple_target_serializes_exact_declared_link_authority() {
    let _trace = capture();
    let _test = tracing::info_span!("apple_external_link_authority").entered();
    let application = tracing::info_span!("arrange").in_scope(application);
    for target in [Target::Desktop, Target::Device, Target::Simulator] {
        let document = tracing::info_span!("act").in_scope(|| bundle::info(&application, target));
        let _assert = tracing::info_span!("assert").entered();
        assert_eq!(
            document
                .matches(moirai_webview::EXTERNAL_LINKS_INFO_KEY)
                .count(),
            1
        );
        assert!(document.contains(&format!(
            "<key>{}</key><array><string>https://github.com/ryancinsight/metis</string><string>https://webassembly.org/</string></array>",
            moirai_webview::EXTERNAL_LINKS_INFO_KEY,
        )));
    }
}

#[test]
fn empty_link_authority_serializes_an_empty_array() {
    let _trace = capture();
    let _test = tracing::info_span!("apple_empty_external_link_authority").entered();
    let application = tracing::info_span!("arrange").in_scope(|| {
        let mut application = application();
        application.external_links.clear();
        application
    });
    for target in [Target::Desktop, Target::Device, Target::Simulator] {
        let document = tracing::info_span!("act").in_scope(|| bundle::info(&application, target));
        let _assert = tracing::info_span!("assert").entered();
        assert!(document.contains(&format!(
            "<key>{}</key><array></array>",
            moirai_webview::EXTERNAL_LINKS_INFO_KEY,
        )));
        assert!(!document.contains("https://github.com/ryancinsight/metis"));
        assert!(!document.contains("https://webassembly.org/"));
    }
}

#[test]
fn link_query_and_fragment_keep_their_identity_through_xml_escaping() {
    let _trace = capture();
    let _test = tracing::info_span!("apple_external_link_xml_identity").entered();
    let application = tracing::info_span!("arrange").in_scope(|| {
        let mut application = application();
        application.external_links =
            vec!["https://example.test/docs?a=1&b=%26#owner's-guide".into()];
        application
            .validate()
            .expect("valid query/fragment destination");
        application
    });
    let document =
        tracing::info_span!("act").in_scope(|| bundle::info(&application, Target::Desktop));
    let _assert = tracing::info_span!("assert").entered();
    assert!(document.contains(&format!(
        "<key>{}</key><array><string>https://example.test/docs?a=1&amp;b=%26#owner&apos;s-guide</string></array>",
        moirai_webview::EXTERNAL_LINKS_INFO_KEY,
    )));
    assert!(!document.contains("a=1&b="));
    assert_eq!(
        application.external_links,
        ["https://example.test/docs?a=1&b=%26#owner's-guide"]
    );
}
