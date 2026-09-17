use super::{
    CompositionPhase, CompositionState, NavigationKey, Selection, SelectionDirection, TextError,
    TextState,
};

#[test]
fn browser_navigation_keys_are_closed_and_format_neutral() {
    assert_eq!(
        NavigationKey::from_browser_key("ArrowLeft"),
        Some(NavigationKey::ArrowLeft)
    );
    assert_eq!(
        NavigationKey::from_browser_key("PageDown"),
        Some(NavigationKey::PageDown)
    );
    assert_eq!(NavigationKey::from_browser_key("a"), None);
}

#[test]
fn keyboard_navigation_records_the_browser_selection_after_default_action() {
    let mut state = TextState::new("e\u{301}😀x".to_owned()).expect("Unicode value is bounded");
    let selection =
        Selection::new(2, 2, SelectionDirection::None).expect("cluster boundary is ordered");
    state
        .apply_navigation(NavigationKey::ArrowRight, selection)
        .expect("browser selection remains on a grapheme boundary");
    assert_eq!(state.selection(), selection);
    assert_eq!(state.state_name(), "navigated");
    assert_eq!(
        state.text_status(),
        "Text: keyboard ArrowRight moved selection"
    );
}

#[test]
fn keyboard_navigation_rejects_a_split_grapheme_without_mutation() {
    let mut state = TextState::new("e\u{301}😀x".to_owned()).expect("Unicode value is bounded");
    let before = state.clone();
    let split =
        Selection::new(1, 1, SelectionDirection::None).expect("selection ordering is valid");
    assert_eq!(
        state
            .apply_navigation(NavigationKey::ArrowLeft, split)
            .expect_err("navigation must preserve extended grapheme boundaries"),
        TextError::SelectionSplitsGrapheme
    );
    assert_eq!(state, before);
}

#[test]
fn unicode_values_preserve_utf16_selection_coordinates() {
    let mut state = TextState::new("A😀é".to_owned()).expect("Unicode value is bounded");
    let selection =
        Selection::new(1, 3, SelectionDirection::Backward).expect("selection ordering is valid");
    state
        .apply_input(
            "A😀é!".to_owned(),
            Some("!".to_owned()),
            "insertText".to_owned(),
            CompositionState::Inactive,
            selection,
        )
        .expect("selection fits the UTF-16 value");
    assert_eq!(state.value, "A😀é!");
    assert_eq!(state.last_input_data.as_deref(), Some("!"));
    assert_eq!(state.input_type(), "insertText");
    assert!(state.text_status().contains("insertText"));
    assert_eq!(state.selection(), selection);
    assert_eq!(state.selection().start(), 1);
    assert_eq!(state.selection().end(), 3);
    assert_eq!(state.selection().direction(), SelectionDirection::Backward);
    assert!(state.selection_status().contains("range 1–3"));
    assert_eq!(state.state_name(), "editing");

    let forward =
        Selection::new(0, 0, SelectionDirection::Forward).expect("forward caret ordering is valid");
    state
        .apply_selection(forward)
        .expect("forward caret fits the value");
    assert_eq!(state.selection().direction(), SelectionDirection::Forward);
    assert_eq!(state.state_name(), "selected");
    let unknown =
        Selection::new(0, 0, SelectionDirection::Other).expect("unknown caret ordering is valid");
    state
        .apply_selection(unknown)
        .expect("unknown caret fits the value");
    assert!(state.selection_status().contains("direction other"));
}

#[test]
fn invalid_selection_and_oversize_input_leave_state_unchanged() {
    let mut state = TextState::default();
    let before = state.clone();
    let reversed = Selection::new(4, 2, SelectionDirection::None)
        .expect_err("reversed selection must be rejected");
    assert_eq!(reversed, TextError::SelectionReversed);
    let selection =
        Selection::new(0, 0, SelectionDirection::None).expect("caret ordering is valid");
    assert_eq!(
        state.apply_input(
            "x".repeat(1_048_577),
            None,
            "insertText".to_owned(),
            CompositionState::Inactive,
            selection,
        ),
        Err(TextError::TextTooLong)
    );
    assert_eq!(state, before);
}

#[test]
fn selection_rejects_offsets_inside_a_surrogate_pair() {
    let mut state = TextState::new("A😀é".to_owned()).expect("Unicode value is bounded");
    let before = state.clone();
    let split =
        Selection::new(2, 2, SelectionDirection::None).expect("selection ordering is valid");
    assert_eq!(
        state
            .apply_selection(split)
            .expect_err("selection must stay on a scalar boundary"),
        TextError::SelectionSplitsScalar
    );
    assert_eq!(state, before);
}

#[test]
fn selection_rejects_offsets_inside_extended_grapheme_clusters() {
    let value = "e\u{301}👩‍💻🇺🇸".to_owned();
    let mut state = TextState::new(value.clone()).expect("Unicode value is bounded");
    let valid =
        Selection::new(2, 7, SelectionDirection::Forward).expect("cluster boundaries are ordered");
    state
        .apply_selection(valid)
        .expect("selection at extended grapheme boundaries is valid");
    assert_eq!(state.selection(), valid);
    let before = state.clone();
    let combining_mark = u32::try_from("e".encode_utf16().count())
        .expect("the combining-mark boundary fits the selection coordinate");
    assert_eq!(
        state
            .apply_selection(
                Selection::new(combining_mark, combining_mark, SelectionDirection::None)
                    .expect("selection ordering is valid"),
            )
            .expect_err("a combining mark must remain in its grapheme cluster"),
        TextError::SelectionSplitsGrapheme
    );
    assert_eq!(state, before);

    let zwj_sequence = u32::try_from("e\u{301}👩".encode_utf16().count())
        .expect("the ZWJ boundary fits the selection coordinate");
    assert_eq!(
        state
            .apply_selection(
                Selection::new(zwj_sequence, zwj_sequence, SelectionDirection::None)
                    .expect("selection ordering is valid"),
            )
            .expect_err("a ZWJ sequence must remain in its grapheme cluster"),
        TextError::SelectionSplitsGrapheme
    );
    assert_eq!(state, before);

    assert_eq!(
        state.apply_input(
            value,
            None,
            "insertText".to_owned(),
            CompositionState::Inactive,
            Selection::new(4, 4, SelectionDirection::None).expect("selection ordering is valid"),
        ),
        Err(TextError::SelectionSplitsGrapheme)
    );
    assert_eq!(state, before);
}

#[test]
fn composition_lifecycle_preserves_preedit_and_terminal_data() {
    let mut state = TextState::default();
    state
        .apply_composition(
            CompositionPhase::Start,
            Some("に".to_owned()),
            "ja-JP".to_owned(),
        )
        .expect("composition metadata is bounded");
    assert_eq!(state.composition(), CompositionState::Active);
    assert_eq!(state.preedit(), Some("に"));
    assert_eq!(state.locale, "ja-JP");
    assert!(state.composition_status().contains("composing"));
    state
        .apply_composition(
            CompositionPhase::Update,
            Some("日本".to_owned()),
            "ja-JP".to_owned(),
        )
        .expect("composition update is bounded");
    assert_eq!(state.preedit(), Some("日本"));
    state
        .apply_composition(
            CompositionPhase::Commit,
            Some("日本".to_owned()),
            "ja-JP".to_owned(),
        )
        .expect("composition commit is bounded");
    assert_eq!(state.composition(), CompositionState::Inactive);
    assert_eq!(state.preedit(), None);
    assert!(state.composition_status().contains("committed"));
    state
        .apply_composition(CompositionPhase::Cancel, None, "ja-JP".to_owned())
        .expect("composition cancellation is bounded");
    assert!(state.composition_status().contains("cancelled"));
}

#[test]
fn display_and_metadata_limits_are_utf8_safe() {
    let mut state = TextState::new("é".repeat(80)).expect("Unicode value is bounded");
    let selection =
        Selection::new(0, 0, SelectionDirection::None).expect("caret ordering is valid");
    assert_eq!(
        state.apply_input(
            "ok".to_owned(),
            None,
            "x".repeat(129),
            CompositionState::Inactive,
            selection,
        ),
        Err(TextError::MetadataTooLong)
    );
    assert_eq!(
        state.apply_composition(CompositionPhase::Start, None, "x".repeat(65),),
        Err(TextError::LocaleTooLong)
    );
    let display = state.value_display();
    assert!(display.is_char_boundary(display.len() - 3));
    assert!(display.ends_with("..."));
}
