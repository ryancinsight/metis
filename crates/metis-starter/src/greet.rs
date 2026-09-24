//! The starter's one command.

/// The greeting the page shows for `name`.
///
/// The wording is create-tauri-app's `greet` command, so the starter behaves
/// as that template does: the name is used as typed, with no trimming.
///
/// # Examples
///
/// ```
/// assert_eq!(
///     metis_starter::greet("World"),
///     "Hello, World! You've been greeted from Rust!"
/// );
/// ```
#[must_use]
pub fn greet(name: &str) -> String {
    format!("Hello, {name}! You've been greeted from Rust!")
}

#[cfg(test)]
mod tests {
    use super::greet;

    #[test]
    fn the_name_is_greeted_exactly_as_typed() {
        assert_eq!(greet(""), "Hello, ! You've been greeted from Rust!");
        assert_eq!(
            greet("  Ada Lovelace "),
            "Hello,   Ada Lovelace ! You've been greeted from Rust!"
        );
        assert_eq!(greet("東京"), "Hello, 東京! You've been greeted from Rust!");
    }
}
