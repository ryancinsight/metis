//! The current route as a reactive store.
//!
//! A [`Navigator`] pairs a [`Router`] with the route the application is on,
//! so views subscribe to navigation the way they subscribe to any other
//! store. Hosts feed it paths: the browser host from its history, a native
//! host from deep links or its own navigation. Navigating to a path no route
//! matches is refused, so the application never enters an unknown screen by
//! its own hand; a path arriving from outside (a typed URL, back/forward)
//! that matches nothing sets the current route to `None`.

use crate::reactive::{CascadeLimit, Store, Subscription, Writable};
use metis_core::route::{RouteError, Router};
use std::rc::Rc;
use std::{error::Error, fmt};

/// The route the application is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentRoute<R> {
    /// The matched route value.
    pub route: R,
    /// The decoded parameters in pattern order.
    pub parameters: Vec<(String, String)>,
    /// The path that was matched, query and fragment included.
    pub path: String,
}

impl<R> CurrentRoute<R> {
    /// The decoded value of parameter `name`.
    #[must_use]
    pub fn parameter(&self, name: &str) -> Option<&str> {
        self.parameters
            .iter()
            .find_map(|(key, value)| (key == name).then_some(value.as_str()))
    }
}

/// Why a navigation was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum NavigationError {
    /// The path does not decode.
    Path(RouteError),
    /// No route matches the path.
    NoRoute,
    /// Listeners kept changing stores past the cascade limit.
    Cascade,
}

impl fmt::Display for NavigationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path(error) => write!(formatter, "navigation path rejected: {error}"),
            Self::NoRoute => formatter.write_str("no route matches the navigation path"),
            Self::Cascade => CascadeLimit.fmt(formatter),
        }
    }
}

impl Error for NavigationError {}

/// A router and the reactive current route.
pub struct Navigator<R> {
    router: Rc<Router<R>>,
    current: Writable<Option<CurrentRoute<R>>>,
}

impl<R> Clone for Navigator<R> {
    fn clone(&self) -> Self {
        Self {
            router: Rc::clone(&self.router),
            current: self.current.clone(),
        }
    }
}

impl<R: Clone + PartialEq + 'static> Navigator<R> {
    /// A navigator starting at `path`.
    ///
    /// # Errors
    /// Returns [`NavigationError::Path`] when `path` does not decode.
    pub fn new(router: Router<R>, path: &str) -> Result<Self, NavigationError> {
        let current = resolve(&router, path)?;
        Ok(Self {
            router: Rc::new(router),
            current: Writable::new(current),
        })
    }

    /// The current route, if any matches.
    #[must_use]
    pub fn current(&self) -> Option<CurrentRoute<R>> {
        self.current.get()
    }

    /// Checks that `path` names a route; the host then commits it to its
    /// history and calls [`Self::arrive`].
    ///
    /// # Errors
    /// Returns [`NavigationError::Path`] or [`NavigationError::NoRoute`].
    pub fn check(&self, path: &str) -> Result<CurrentRoute<R>, NavigationError> {
        resolve(&self.router, path)?.ok_or(NavigationError::NoRoute)
    }

    /// Records that the host is now at `path`, from its own navigation or
    /// from outside; an unmatched path clears the current route. Returns
    /// whether the current route changed.
    ///
    /// # Errors
    /// Returns [`NavigationError::Path`] for a path that does not decode
    /// (the current route is kept), or [`NavigationError::Cascade`].
    pub fn arrive(&self, path: &str) -> Result<bool, NavigationError> {
        let next = resolve(&self.router, path)?;
        self.current.set(next).map_err(|_| NavigationError::Cascade)
    }
}

impl<R: Clone + PartialEq + 'static> Store<Option<CurrentRoute<R>>> for Navigator<R> {
    fn with<T>(&self, read: impl FnOnce(&Option<CurrentRoute<R>>) -> T) -> T {
        self.current.with(read)
    }

    fn subscribe(&self, listener: impl FnMut(&Option<CurrentRoute<R>>) + 'static) -> Subscription {
        self.current.subscribe(listener)
    }
}

fn resolve<R: Clone>(
    router: &Router<R>,
    path: &str,
) -> Result<Option<CurrentRoute<R>>, NavigationError> {
    let found = router.resolve(path).map_err(NavigationError::Path)?;
    Ok(found.map(|found| CurrentRoute {
        route: found.route.clone(),
        parameters: found
            .parameters()
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect(),
        path: path.to_owned(),
    }))
}

#[cfg(test)]
mod tests;
