use super::{NavigationError, Navigator};
use crate::reactive::{Store, derived};
use metis_core::route::{RouteError, Router};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Home,
    Study,
}

fn navigator(path: &str) -> Navigator<Screen> {
    let mut router = Router::new();
    router.add("/", Screen::Home).expect("home");
    router.add("/study/:id", Screen::Study).expect("study");
    Navigator::new(router, path).expect("navigator")
}

#[test]
fn arrivals_update_subscribers_and_derived_views() {
    let navigator = navigator("/");
    let title = derived(&navigator, |current| match current {
        Some(route) if route.route == Screen::Study => {
            format!("Study {}", route.parameter("id").unwrap_or_default())
        }
        Some(_) => "Home".to_owned(),
        None => "Not found".to_owned(),
    });
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = Rc::clone(&seen);
    let _watch = title.subscribe(move |text| log.borrow_mut().push(text.clone()));
    assert!(navigator.arrive("/study/7?tab=info").expect("arrive"));
    assert!(!navigator.arrive("/study/7?tab=info").expect("same path"));
    assert!(navigator.arrive("/missing").expect("unmatched"));
    assert_eq!(*seen.borrow(), ["Home", "Study 7", "Not found"]);
    assert_eq!(navigator.current(), None);
}

#[test]
fn checks_refuse_unknown_and_malformed_paths() {
    let navigator = navigator("/study/1");
    assert_eq!(navigator.check("/missing"), Err(NavigationError::NoRoute));
    assert_eq!(
        navigator.check("/study/.."),
        Err(NavigationError::Path(RouteError::Malformed))
    );
    let before = navigator.current();
    assert!(navigator.arrive("/study/%zz").is_err());
    assert_eq!(
        navigator.current(),
        before,
        "a malformed arrival keeps the route"
    );
    let checked = navigator.check("/study/a%20b").expect("route");
    assert_eq!(checked.parameter("id"), Some("a b"));
    assert_eq!(checked.path, "/study/a%20b");
}
