use super::{MAX_ROUTE_SEGMENTS, MAX_ROUTES, RouteError, RoutePattern, Router};
use crate::deep_link::{DeepLink, DeepLinkScheme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Home,
    Studies,
    Study,
    Series,
    NewStudy,
    Files,
}

fn router() -> Router<Screen> {
    let mut router = Router::new();
    for (pattern, screen) in [
        ("/", Screen::Home),
        ("/study", Screen::Studies),
        ("/study/:id", Screen::Study),
        ("/study/:id/series/:series", Screen::Series),
        ("/study/new", Screen::NewStudy),
        ("/files/*path", Screen::Files),
    ] {
        router.add(pattern, screen).expect("route");
    }
    router
}

#[test]
fn paths_resolve_to_the_most_specific_route() {
    let router = router();
    let found = |path: &str| router.resolve(path).expect("path").map(|m| *m.route);
    assert_eq!(found("/"), Some(Screen::Home));
    assert_eq!(found("/study/"), Some(Screen::Studies));
    assert_eq!(
        found("/study/new"),
        Some(Screen::NewStudy),
        "a literal beats a parameter"
    );
    assert_eq!(found("/study/7?tab=info#top"), Some(Screen::Study));
    assert_eq!(found("/study/7/series"), None);
    assert_eq!(found("/files"), None, "a rest match needs a segment");
    assert_eq!(found("/unknown"), None);

    let series = router
        .resolve("/study/a%20b/series/3")
        .expect("path")
        .expect("match");
    assert_eq!(*series.route, Screen::Series);
    assert_eq!(series.parameter("id"), Some("a b"));
    assert_eq!(series.parameter("series"), Some("3"));
    let files = router
        .resolve("/files/2026/scan.dcm")
        .expect("path")
        .expect("match");
    assert_eq!(files.parameter("path"), Some("2026/scan.dcm"));
}

#[test]
fn matching_does_not_depend_on_insertion_order() {
    let mut reversed = Router::new();
    reversed.add("/files/*path", Screen::Files).expect("rest");
    reversed
        .add("/files/:name", Screen::Study)
        .expect("parameter");
    reversed
        .add("/files/readme", Screen::Home)
        .expect("literal");
    let found = |path: &str| *reversed.resolve(path).expect("path").expect("match").route;
    assert_eq!(found("/files/readme"), Screen::Home);
    assert_eq!(found("/files/a"), Screen::Study);
    assert_eq!(found("/files/a/b"), Screen::Files);
}

#[test]
fn patterns_conflicts_and_bounds_are_checked() {
    let mut router = router();
    assert_eq!(
        router.add("/study/:other", Screen::Home),
        Err(RouteError::Conflict)
    );
    for invalid in [
        "study",
        "/:",
        "/:Id",
        "/*rest/tail",
        "/:a/:a",
        "/a/%zz",
        "/a/..",
        "/:1x",
    ] {
        assert_eq!(
            RoutePattern::parse(invalid),
            Err(RouteError::InvalidPattern),
            "{invalid}"
        );
    }
    for rejected in ["/study/%2F", "/study/..", "/study/%zz", "/study/%00"] {
        assert_eq!(
            router.resolve(rejected).map(|m| m.is_some()),
            Err(RouteError::Malformed),
            "{rejected}"
        );
    }
    let deep = "/x".repeat(MAX_ROUTE_SEGMENTS + 1);
    assert_eq!(router.resolve(&deep).map(|_| ()), Err(RouteError::TooLarge));
    let mut full = Router::new();
    for index in 0..MAX_ROUTES {
        full.add(&format!("/r{index}"), index).expect("route");
    }
    assert_eq!(full.add("/one-more", 0), Err(RouteError::Full));
}

#[test]
fn hrefs_route_back_to_their_parameters() {
    let pattern = RoutePattern::parse("/study/:id/series/:series").expect("pattern");
    let href = pattern
        .href(&[("id", "a b&c"), ("series", "3")])
        .expect("href");
    assert_eq!(href, "/study/a%20b%26c/series/3");
    let router = router();
    let found = router.resolve(&href).expect("path").expect("match");
    assert_eq!(found.parameter("id"), Some("a b&c"));
    assert_eq!(
        pattern.href(&[("id", "a/b"), ("series", "3")]),
        Err(RouteError::Malformed),
        "a separator cannot hide inside one segment"
    );
    assert_eq!(pattern.href(&[("id", "7")]), Err(RouteError::Malformed));
    assert_eq!(
        pattern.href(&[("id", ".."), ("series", "1")]),
        Err(RouteError::Malformed)
    );
    let rest = RoutePattern::parse("/files/*path").expect("pattern");
    assert_eq!(
        rest.href(&[("path", "2026/scan.dcm")]).expect("href"),
        "/files/2026/scan.dcm"
    );
    assert_eq!(
        RoutePattern::parse("/")
            .expect("root")
            .href(&[])
            .expect("href"),
        "/"
    );
}

#[test]
fn deep_links_route_through_their_segments() {
    let schemes = [DeepLinkScheme::new("org.metis.viewer").expect("scheme")];
    let link = DeepLink::parse("org.metis.viewer://study/7/series/2", &schemes).expect("link");
    let router = router();
    let found = router.resolve_segments(link.segments()).expect("match");
    assert_eq!(*found.route, Screen::Series);
    assert_eq!(
        found.parameters(),
        [("id", "7".to_owned()), ("series", "2".to_owned())]
    );
}
