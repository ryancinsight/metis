use super::letter;
use crate::input::{Digit, FunctionKey, Key, Letter};

#[test]
fn bounded_constructors_reject_out_of_range_values() {
    assert_eq!(Letter::new(b'q').map(Letter::byte), Some(b'Q'));
    assert_eq!(Letter::new(b'1'), None);
    assert_eq!(Digit::new(9).map(Digit::value), Some(9));
    assert_eq!(Digit::new(10), None);
    assert_eq!(FunctionKey::new(0), None);
    assert_eq!(FunctionKey::new(24).map(FunctionKey::number), Some(24));
    assert_eq!(FunctionKey::new(25), None);
}

#[test]
fn browser_codes_name_physical_positions() {
    assert_eq!(Key::from_browser_code("KeyK"), Some(letter(b'K')));
    assert_eq!(
        Key::from_browser_code("Digit7"),
        Digit::new(7).map(Key::Digit)
    );
    assert_eq!(
        Key::from_browser_code("F11"),
        FunctionKey::new(11).map(Key::Function)
    );
    assert_eq!(Key::from_browser_code("NumpadEnter"), Some(Key::Enter));
    assert_eq!(Key::from_browser_code("Equal"), Some(Key::Equal));
    for rejected in [
        "Keyk",
        "KeyAB",
        "Digit",
        "F0",
        "F25",
        "ShiftLeft",
        "IntlRo",
        "",
    ] {
        assert_eq!(Key::from_browser_code(rejected), None, "{rejected}");
    }
}

#[test]
fn browser_key_values_accept_canonical_names_only() {
    assert_eq!(Key::from_browser_key("k"), Some(letter(b'K')));
    assert_eq!(Key::from_browser_key("K"), Some(letter(b'K')));
    assert_eq!(Key::from_browser_key(" "), Some(Key::Space));
    assert_eq!(Key::from_browser_key("Esc"), Some(Key::Escape));
    assert_eq!(Key::from_browser_key("ArrowUp"), Some(Key::ArrowUp));
    assert_eq!(Key::from_browser_key("/"), Some(Key::Slash));
    for rejected in ["arrowup", "Up", "Return", "Shift", "Dead", "+", "é"] {
        assert_eq!(Key::from_browser_key(rejected), None, "{rejected}");
    }
}

#[test]
fn windows_virtual_keys_cover_the_named_set() {
    assert_eq!(Key::from_windows_virtual_key(0x4B), Some(letter(b'K')));
    assert_eq!(
        Key::from_windows_virtual_key(0x31),
        Digit::new(1).map(Key::Digit)
    );
    assert_eq!(
        Key::from_windows_virtual_key(0x87),
        FunctionKey::new(24).map(Key::Function)
    );
    assert_eq!(Key::from_windows_virtual_key(0x1B), Some(Key::Escape));
    assert_eq!(Key::from_windows_virtual_key(0xBB), Some(Key::Equal));
    for rejected in [0x10, 0x11, 0x5B, 0x88, 0x1_0041] {
        assert_eq!(
            Key::from_windows_virtual_key(rejected),
            None,
            "{rejected:#x}"
        );
    }
}

#[test]
fn every_key_name_parses_back_to_its_key() {
    let mut keys: Vec<Key> = (b'A'..=b'Z').map(letter).collect();
    keys.extend((0..=9).filter_map(Digit::new).map(Key::Digit));
    keys.extend(
        (1..=FunctionKey::MAX)
            .filter_map(FunctionKey::new)
            .map(Key::Function),
    );
    keys.extend([
        Key::Enter,
        Key::Escape,
        Key::Tab,
        Key::Space,
        Key::Backspace,
        Key::Delete,
        Key::Insert,
        Key::Home,
        Key::End,
        Key::PageUp,
        Key::PageDown,
        Key::ArrowUp,
        Key::ArrowDown,
        Key::ArrowLeft,
        Key::ArrowRight,
        Key::Minus,
        Key::Equal,
        Key::Comma,
        Key::Period,
        Key::Slash,
    ]);
    for key in keys {
        assert_eq!(Key::from_token(key.name()), Some(key), "{}", key.name());
    }
}

#[test]
fn every_windows_virtual_key_maps_back_to_its_code() {
    let mut mapped = 0;
    for code in 0..=0xFF {
        if let Some(key) = Key::from_windows_virtual_key(code) {
            assert_eq!(key.windows_virtual_key(), Some(code), "{key:?}");
            mapped += 1;
        }
    }
    assert_eq!(mapped, 26 + 10 + 24 + 20);
}
