//! Browser history for the application's routes.

use metis_core::route::Router;
use metis_frontend::navigation::{CurrentRoute, NavigationError, Navigator};
use metis_frontend::reactive::{Store, Subscription};
use moirai_pal::wasm::{HistoryListener, WebHistory};
use std::io;

/// A [`Navigator`] kept in step with the page's same-origin history.
///
/// The current route starts from the page's path, follows back and forward,
/// and [`BrowserNavigator::navigate`] pushes a history entry only for a path
/// some route matches. Dropping the navigator removes its history listener.
pub struct BrowserNavigator<R> {
    navigator: Navigator<R>,
    history: WebHistory,
    _listener: HistoryListener,
}

impl<R: Clone + PartialEq + 'static> BrowserNavigator<R> {
    /// Starts from the page's current path.
    ///
    /// # Errors
    /// Returns the browser error, or `InvalidData` when the page's path does
    /// not decode.
    pub fn new(router: Router<R>) -> io::Result<Self> {
        let history = WebHistory::new()?;
        let navigator = Navigator::new(router, &history.path()?).map_err(invalid_data)?;
        let follower = navigator.clone();
        let listener = history.on_navigate(move |path| {
            // A path that does not decode keeps the current route.
            let _ = follower.arrive(&path);
        })?;
        Ok(Self {
            navigator,
            history,
            _listener: listener,
        })
    }

    /// Moves to `path`, adding a history entry.
    ///
    /// # Errors
    /// Returns `NotFound` when no route matches, `InvalidInput` for a path
    /// that does not decode or is not same-origin, or the browser error.
    pub fn navigate(&self, path: &str) -> io::Result<CurrentRoute<R>> {
        self.commit(path, WebHistory::push)
    }

    /// Moves to `path`, replacing the current history entry.
    ///
    /// # Errors
    /// As [`Self::navigate`].
    pub fn redirect(&self, path: &str) -> io::Result<CurrentRoute<R>> {
        self.commit(path, WebHistory::replace)
    }

    /// The navigator, for deriving views from the current route.
    #[must_use]
    pub fn navigator(&self) -> &Navigator<R> {
        &self.navigator
    }

    fn commit(
        &self,
        path: &str,
        write: impl FnOnce(&WebHistory, &str) -> io::Result<()>,
    ) -> io::Result<CurrentRoute<R>> {
        let route = self.navigator.check(path).map_err(|error| match error {
            NavigationError::NoRoute => io::Error::new(io::ErrorKind::NotFound, error),
            _ => io::Error::new(io::ErrorKind::InvalidInput, error),
        })?;
        write(&self.history, path)?;
        self.navigator.arrive(path).map_err(invalid_data)?;
        Ok(route)
    }
}

impl<R: Clone + PartialEq + 'static> Store<Option<CurrentRoute<R>>> for BrowserNavigator<R> {
    fn with<T>(&self, read: impl FnOnce(&Option<CurrentRoute<R>>) -> T) -> T {
        self.navigator.with(read)
    }

    fn subscribe(&self, listener: impl FnMut(&Option<CurrentRoute<R>>) + 'static) -> Subscription {
        self.navigator.subscribe(listener)
    }
}

fn invalid_data(error: NavigationError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}
