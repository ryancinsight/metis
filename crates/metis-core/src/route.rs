//! Typed routes for application screens.
//!
//! The counterpart of Dioxus's `Routable` router and `SvelteKit`'s route
//! folders: a [`Router`] maps path patterns such as `/study/:id` or
//! `/files/*path` to application values, and resolves a path, or the
//! segments of a [`crate::deep_link::DeepLink`], to the matching value and
//! its decoded parameters. Paths decode under the same rule as deep links.
//!
//! Matching does not depend on the order routes were added: at the first
//! position where two patterns differ, a literal beats a parameter and a
//! parameter beats a rest match, and two patterns of the same shape are
//! refused as a conflict when the second is added.

mod pattern;

pub use pattern::RoutePattern;

use crate::uri_path;
use std::{error::Error, fmt};

/// Most routes one router holds.
pub const MAX_ROUTES: usize = 64;
/// Most segments a routed path may carry.
pub const MAX_ROUTE_SEGMENTS: usize = 32;
/// Longest routed path, in bytes.
pub const MAX_ROUTE_PATH_BYTES: usize = 2048;

/// Why a pattern, route or path was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RouteError {
    /// The pattern is not `/`-separated literals, `:name` parameters and an
    /// optional final `*name` rest match with distinct names.
    InvalidPattern,
    /// A route of the same shape is already present.
    Conflict,
    /// The router holds [`MAX_ROUTES`] routes.
    Full,
    /// The path does not decode, or a parameter value is missing.
    Malformed,
    /// The path exceeds [`MAX_ROUTE_PATH_BYTES`] or [`MAX_ROUTE_SEGMENTS`].
    TooLarge,
}

impl fmt::Display for RouteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidPattern => "route pattern is invalid",
            Self::Conflict => "a route of the same shape exists",
            Self::Full => "router is full",
            Self::Malformed => "route path is malformed",
            Self::TooLarge => "route path exceeds its bounds",
        })
    }
}

impl Error for RouteError {}

/// A resolved route and its decoded parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteMatch<'a, R> {
    /// The value the matching pattern was added with.
    pub route: &'a R,
    parameters: Vec<(&'a str, String)>,
}

impl<R> RouteMatch<'_, R> {
    /// The decoded value of parameter `name`; a rest match joins its
    /// segments with `/`.
    #[must_use]
    pub fn parameter(&self, name: &str) -> Option<&str> {
        self.parameters
            .iter()
            .find_map(|(key, value)| (*key == name).then_some(value.as_str()))
    }

    /// Every parameter in pattern order.
    #[must_use]
    pub fn parameters(&self) -> &[(&str, String)] {
        &self.parameters
    }
}

/// Maps route patterns to application values.
#[derive(Debug, Clone)]
pub struct Router<R> {
    routes: Vec<(RoutePattern, R)>,
}

impl<R> Default for Router<R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R> Router<R> {
    /// An empty router.
    #[must_use]
    pub const fn new() -> Self {
        Self { routes: Vec::new() }
    }

    /// Number of routes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.routes.len()
    }

    /// Whether no route is present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    /// Adds `route` for `pattern`.
    ///
    /// # Errors
    /// Returns [`RouteError::InvalidPattern`], [`RouteError::Conflict`] for a
    /// pattern with an existing shape, or [`RouteError::Full`].
    pub fn add(&mut self, pattern: &str, route: R) -> Result<(), RouteError> {
        let pattern = RoutePattern::parse(pattern)?;
        if self
            .routes
            .iter()
            .any(|(existing, _)| existing.same_shape(&pattern))
        {
            return Err(RouteError::Conflict);
        }
        if self.routes.len() >= MAX_ROUTES {
            return Err(RouteError::Full);
        }
        self.routes.push((pattern, route));
        Ok(())
    }

    /// Resolves an absolute path such as `/study/7?series=3`; the query and
    /// fragment are ignored.
    ///
    /// # Errors
    /// Returns [`RouteError::Malformed`] or [`RouteError::TooLarge`] for a
    /// path that does not decode within its bounds.
    pub fn resolve(&self, path: &str) -> Result<Option<RouteMatch<'_, R>>, RouteError> {
        if path.len() > MAX_ROUTE_PATH_BYTES {
            return Err(RouteError::TooLarge);
        }
        let path = path.split(['?', '#']).next().unwrap_or_default();
        let segments =
            uri_path::segments(path, MAX_ROUTE_SEGMENTS).map_err(|error| match error {
                uri_path::PathError::Malformed => RouteError::Malformed,
                uri_path::PathError::TooLarge => RouteError::TooLarge,
            })?;
        Ok(self.resolve_segments(&segments))
    }

    /// Resolves already-decoded segments, such as a deep link's.
    #[must_use]
    pub fn resolve_segments<S: AsRef<str>>(&self, segments: &[S]) -> Option<RouteMatch<'_, R>> {
        self.routes
            .iter()
            .filter_map(|(pattern, route)| {
                pattern
                    .capture(segments)
                    .map(|parameters| (pattern, RouteMatch { route, parameters }))
            })
            .min_by(|(left, _), (right, _)| left.precedence(right))
            .map(|(_, found)| found)
    }
}

#[cfg(test)]
mod tests;
