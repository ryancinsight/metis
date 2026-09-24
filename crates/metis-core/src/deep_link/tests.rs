use super::{DeepLink, DeepLinkError, DeepLinkScheme, MAX_DEEP_LINK_BYTES, MAX_DEEP_LINK_SEGMENTS};

fn schemes() -> Vec<DeepLinkScheme> {
    vec![DeepLinkScheme::new("org.atlas.viewer").expect("scheme")]
}

#[test]
fn schemes_are_lowercase_unreserved_and_bounded() {
    for valid in ["myapp", "org.atlas.viewer", "a1+b-c"] {
        assert!(DeepLinkScheme::new(valid).is_ok(), "{valid}");
    }
    for invalid in [
        "",
        "MyApp",
        "1app",
        "my_app",
        "my app",
        "https",
        "file",
        "javascript",
        &"a".repeat(33),
    ] {
        assert_eq!(
            DeepLinkScheme::new(invalid),
            Err(DeepLinkError::InvalidScheme),
            "{invalid:?}"
        );
    }
}

#[test]
fn links_decode_routes_and_query_pairs() {
    let link = DeepLink::parse(
        "org.atlas.viewer://open/study%20A?series=3&label=CT%2FMR&flag#ignored",
        &schemes(),
    )
    .expect("link");
    assert_eq!(link.scheme().as_str(), "org.atlas.viewer");
    assert_eq!(link.segments(), ["open", "study A"]);
    assert_eq!(link.query_value("series"), Some("3"));
    assert_eq!(link.query_value("label"), Some("CT/MR"));
    assert_eq!(link.query_value("flag"), Some(""));
    let opaque = DeepLink::parse("ORG.ATLAS.VIEWER:open//study%20A", &schemes()).expect("link");
    assert_eq!(opaque.segments(), link.segments());
}

#[test]
fn hostile_links_are_rejected() {
    let cases = [
        ("other://open", DeepLinkError::UnknownScheme),
        ("org.atlas.viewer", DeepLinkError::Malformed),
        ("org.atlas.viewer://open study", DeepLinkError::Malformed),
        (
            "org.atlas.viewer://open/%2e%2e/secrets",
            DeepLinkError::Malformed,
        ),
        (
            "org.atlas.viewer://open/../secrets",
            DeepLinkError::Malformed,
        ),
        ("org.atlas.viewer://open/a%2Fb", DeepLinkError::Malformed),
        ("org.atlas.viewer://open/%0a", DeepLinkError::Malformed),
        ("org.atlas.viewer://open/%ff", DeepLinkError::Malformed),
        ("org.atlas.viewer://open/%4", DeepLinkError::Malformed),
        ("org.atlas.viewer://open?=value", DeepLinkError::Malformed),
        ("org.atlas.viewer://ópen", DeepLinkError::Malformed),
    ];
    for (text, error) in cases {
        assert_eq!(DeepLink::parse(text, &schemes()), Err(error), "{text}");
    }
    let long = format!("org.atlas.viewer://{}", "a".repeat(MAX_DEEP_LINK_BYTES));
    assert_eq!(
        DeepLink::parse(&long, &schemes()),
        Err(DeepLinkError::TooLarge)
    );
    let deep = format!(
        "org.atlas.viewer://{}",
        "a/".repeat(MAX_DEEP_LINK_SEGMENTS + 1)
    );
    assert_eq!(
        DeepLink::parse(&deep, &schemes()),
        Err(DeepLinkError::TooLarge)
    );
}

#[test]
fn the_first_link_argument_is_found() {
    let arguments = ["--flag", "https://example.org", "org.atlas.viewer://open/x"];
    let link = DeepLink::from_arguments(arguments, &schemes()).expect("link argument");
    assert_eq!(link.segments(), ["open", "x"]);
    assert_eq!(DeepLink::from_arguments(["--flag"], &schemes()), None);
}
