use super::{chord, letter};
use crate::input::{Accelerator, AcceleratorError, Key, MAX_ACCELERATOR_BYTES, Modifiers};

#[test]
fn parse_resolves_primary_per_supplied_platform() {
    let windows = Accelerator::parse_with_primary("CmdOrCtrl+S", Modifiers::CTRL).expect("parse");
    let apple = Accelerator::parse_with_primary("CmdOrCtrl+S", Modifiers::META).expect("parse");
    assert_eq!(windows, chord(Modifiers::CTRL, b'S'));
    assert_eq!(apple, chord(Modifiers::META, b'S'));
    assert_eq!(
        Accelerator::parse("Primary+S").expect("parse"),
        chord(Modifiers::host_primary(), b'S')
    );
}

#[test]
fn parse_is_case_insensitive_and_order_free_for_modifiers() {
    let expected = chord(
        Modifiers::CTRL
            .union(Modifiers::ALT)
            .union(Modifiers::SHIFT),
        b'K',
    );
    for text in [
        "Ctrl+Alt+Shift+K",
        "shift+alt+control+k",
        "OPTION+Shift+Ctrl+k",
    ] {
        assert_eq!(Accelerator::parse(text), Ok(expected), "{text}");
    }
}

#[test]
fn display_is_canonical_and_round_trips() {
    for text in [
        "Ctrl+Alt+Shift+Meta+F12",
        "Shift+Space",
        "Ctrl+=",
        "Alt+ArrowLeft",
        "Meta+/",
        "Escape",
        "Ctrl+9",
    ] {
        let parsed = Accelerator::parse(text).expect(text);
        assert_eq!(parsed.to_string(), text);
        assert_eq!(Accelerator::parse(&parsed.to_string()), Ok(parsed));
    }
    assert_eq!(
        Accelerator::parse("cmd+option+esc")
            .expect("aliases")
            .to_string(),
        "Alt+Meta+Escape"
    );
}

#[test]
fn aria_spelling_uses_control_and_key_values() {
    let accelerator = Accelerator::parse("Ctrl+Shift+Delete").expect("parse");
    assert_eq!(accelerator.aria_keyshortcuts(), "Control+Shift+Delete");
    assert_eq!(
        Accelerator::new(Modifiers::ALT, Key::Space).aria_keyshortcuts(),
        "Alt+Space"
    );
}

#[test]
fn malformed_text_is_rejected_with_a_typed_reason() {
    let cases: [(&str, AcceleratorError); 9] = [
        ("", AcceleratorError::Empty),
        ("Ctrl++K", AcceleratorError::EmptyToken),
        ("+K", AcceleratorError::EmptyToken),
        ("Ctrl+", AcceleratorError::EmptyToken),
        ("Ctrl+Shift", AcceleratorError::MissingKey),
        ("Ctrl+Ctrl+K", AcceleratorError::DuplicateModifier),
        ("K+L", AcceleratorError::MultipleKeys),
        ("K+Ctrl", AcceleratorError::MultipleKeys),
        (
            "Ctrl+Hyper",
            AcceleratorError::UnknownToken("Hyper".to_owned()),
        ),
    ];
    for (text, error) in cases {
        assert_eq!(Accelerator::parse(text), Err(error), "{text:?}");
    }
    let duplicate_primary = Accelerator::parse_with_primary("Ctrl+CmdOrCtrl+K", Modifiers::CTRL);
    assert_eq!(duplicate_primary, Err(AcceleratorError::DuplicateModifier));
    let oversized = "Shift+".repeat(MAX_ACCELERATOR_BYTES);
    assert_eq!(
        Accelerator::parse(&oversized),
        Err(AcceleratorError::TooLong)
    );
    assert_eq!(
        Accelerator::parse_with_primary("Primary+K", Modifiers::CTRL.union(Modifiers::META)),
        Err(AcceleratorError::InvalidPrimary)
    );
}

#[test]
fn function_keys_are_bounded() {
    assert!(Accelerator::parse("F1").is_ok());
    assert!(Accelerator::parse("f24").is_ok());
    for text in ["F0", "F25", "F01", "F"] {
        assert!(
            !matches!(Accelerator::parse(text), Ok(parsed) if matches!(parsed.key(), Key::Function(_))),
            "{text}"
        );
    }
    assert_eq!(
        Accelerator::parse("F").map(Accelerator::key),
        Ok(letter(b'F'))
    );
}
