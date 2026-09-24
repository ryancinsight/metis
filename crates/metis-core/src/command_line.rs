//! Command lines written for launchers that the operating system parses.
//!
//! Installers, shortcuts and login items each store a program and its
//! arguments as one string that another parser splits again. These encoders
//! are the single implementation of each platform's quoting rules, so an
//! argument reaches the program exactly as given.

/// Quotes one argument under the Microsoft C runtime rules.
///
/// The result is always quoted, so empty arguments and arguments with spaces
/// survive; backslashes are doubled only where they precede a quote.
/// See [Parsing C command-line arguments](https://learn.microsoft.com/en-us/cpp/c-language/parsing-c-command-line-arguments).
#[must_use]
pub fn windows_argument(argument: &str) -> String {
    let mut encoded = String::with_capacity(argument.len() + 2);
    encoded.push('"');
    let mut backslashes = 0;
    for character in argument.chars() {
        if character == '\\' {
            backslashes += 1;
            continue;
        }
        let escapes = if character == '"' {
            backslashes * 2 + 1
        } else {
            backslashes
        };
        encoded.extend(std::iter::repeat_n('\\', escapes));
        encoded.push(character);
        backslashes = 0;
    }
    encoded.extend(std::iter::repeat_n('\\', backslashes * 2));
    encoded.push('"');
    encoded
}

/// Joins a program and its arguments into one Windows command line.
#[must_use]
pub fn windows_command_line<I, S>(program: &str, arguments: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut line = windows_argument(program);
    for argument in arguments {
        line.push(' ');
        line.push_str(&windows_argument(argument.as_ref()));
    }
    line
}

/// Quotes one word of a freedesktop.org desktop entry `Exec` key.
///
/// Words with spaces, controls or reserved characters are quoted, and `\`,
/// `"`, `` ` `` and `$` are escaped inside the quotes; `%` doubles so the
/// launcher does not read it as a field code.
#[must_use]
pub fn desktop_exec_word(value: &str) -> String {
    let needs_quotes = value.is_empty()
        || value.bytes().any(|byte| {
            byte.is_ascii_whitespace()
                || byte.is_ascii_control()
                || matches!(byte, b'"' | b'`' | b'$' | b'\\' | b'%')
        });
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' | '"' | '`' | '$' => {
                escaped.push('\\');
                escaped.push(character);
            }
            '%' => escaped.push_str("%%"),
            _ => escaped.push(character),
        }
    }
    if needs_quotes {
        format!("\"{escaped}\"")
    } else {
        escaped
    }
}

#[cfg(test)]
mod tests {
    use super::{desktop_exec_word, windows_argument, windows_command_line};

    #[test]
    fn windows_arguments_follow_the_crt_rules() {
        assert_eq!(windows_argument(""), "\"\"");
        assert_eq!(windows_argument("a b"), "\"a b\"");
        assert_eq!(windows_argument("ab\"c"), "\"ab\\\"c\"");
        assert_eq!(windows_argument("C:\\folder\\"), "\"C:\\folder\\\\\"");
        assert_eq!(windows_argument("a\\\\b"), "\"a\\\\b\"");
        assert_eq!(
            windows_command_line("C:\\App\\app.exe", ["--minimized", ""]),
            "\"C:\\App\\app.exe\" \"--minimized\" \"\""
        );
    }

    #[test]
    fn desktop_words_escape_reserved_characters() {
        assert_eq!(desktop_exec_word("plain"), "plain");
        assert_eq!(desktop_exec_word(""), "\"\"");
        assert_eq!(desktop_exec_word("a b"), "\"a b\"");
        assert_eq!(desktop_exec_word("$HOME"), "\"\\$HOME\"");
        assert_eq!(desktop_exec_word("a%b"), "\"a%%b\"");
        assert_eq!(desktop_exec_word("a`b"), "\"a\\`b\"");
    }
}
