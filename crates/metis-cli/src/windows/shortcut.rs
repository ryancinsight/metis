//! Shortcut arguments follow the documented Microsoft CRT quoting rules.
//!
//! [Microsoft parsing rules](https://learn.microsoft.com/en-us/cpp/c-language/parsing-c-command-line-arguments).
use std::error::Error;

pub(super) fn arguments(arguments: &[String]) -> Result<String, Box<dyn Error>> {
    let mut encoded = String::new();
    for argument in arguments {
        if argument.len() > 4096
            || argument
                .chars()
                .any(|character| character.is_control() || "[]".contains(character))
        {
            return Err("MSI shortcut arguments contain controls, formatted property brackets or exceed 4096 bytes".into());
        }
        if !encoded.is_empty() {
            encoded.push(' ');
        }
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
        // The standard Shortcut.Arguments column is CHAR(255); enforce the
        // encoded bound instead of letting native insertion truncate silently.
        if encoded.encode_utf16().count() > 255 {
            return Err(
                "MSI shortcut arguments exceed the 255-character table limit after quoting".into(),
            );
        }
    }
    Ok(encoded)
}

#[cfg(test)]
mod tests {
    use super::arguments;

    #[test]
    fn quotes_empty_space_quote_and_trailing_backslash() {
        let values = ["", "a b", "ab\"c", "C:\\folder\\"].map(str::to_owned);
        assert_eq!(
            arguments(&values).expect("quoted arguments"),
            "\"\" \"a b\" \"ab\\\"c\" \"C:\\folder\\\\\""
        );
        assert_eq!(
            arguments(&["60".into(), "2".into(), "0.2".into()]).expect("demo arguments"),
            "\"60\" \"2\" \"0.2\""
        );
    }

    #[test]
    fn rejects_installer_property_expansion_and_encoded_overflow() {
        assert_eq!(
            arguments(&["[INSTALLDIR]".into()])
                .expect_err("formatted input")
                .to_string(),
            "MSI shortcut arguments contain controls, formatted property brackets or exceed 4096 bytes"
        );
        assert_eq!(
            arguments(&["x".repeat(254)])
                .expect_err("quoted width")
                .to_string(),
            "MSI shortcut arguments exceed the 255-character table limit after quoting"
        );
        assert_eq!(
            arguments(&["x".repeat(253)])
                .expect("exact table width")
                .len(),
            255
        );
    }
}
