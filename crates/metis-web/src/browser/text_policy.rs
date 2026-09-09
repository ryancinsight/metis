//! Rust-owned text editing policy for the browser workbench.

use std::fmt;

const MAX_TEXT_BYTES: usize = 1_048_576;
const MAX_METADATA_BYTES: usize = 128;
const MAX_LOCALE_BYTES: usize = 64;
const MAX_DISPLAY_BYTES: usize = 96;
const INITIAL_TEXT: &str = "Résumé — 東京 / 影像";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SelectionDirection {
    Forward,
    Backward,
    None,
    Other,
}

impl SelectionDirection {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Forward => "forward",
            Self::Backward => "backward",
            Self::None => "none",
            Self::Other => "other",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Selection {
    start: u32,
    end: u32,
    direction: SelectionDirection,
}

impl Selection {
    pub(crate) fn new(
        start: u32,
        end: u32,
        direction: SelectionDirection,
    ) -> Result<Self, TextError> {
        if start > end {
            return Err(TextError::SelectionReversed);
        }
        Ok(Self {
            start,
            end,
            direction,
        })
    }

    pub(crate) const fn start(self) -> u32 {
        self.start
    }

    pub(crate) const fn end(self) -> u32 {
        self.end
    }

    pub(crate) const fn direction(self) -> SelectionDirection {
        self.direction
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompositionState {
    Inactive,
    Active,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompositionPhase {
    Start,
    Update,
    Commit,
    Cancel,
}

impl CompositionPhase {
    const fn label(self) -> &'static str {
        match self {
            Self::Start => "started",
            Self::Update => "updated",
            Self::Commit => "committed",
            Self::Cancel => "cancelled",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TextEvent {
    Ready,
    Input,
    Composition(CompositionPhase),
    Selection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TextError {
    TextTooLong,
    MetadataTooLong,
    LocaleTooLong,
    SelectionReversed,
    SelectionOutOfBounds,
    SelectionSplitsScalar,
}

impl fmt::Display for TextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::TextTooLong => "text value exceeds the 1 MiB bound",
            Self::MetadataTooLong => "text event metadata exceeds the 128-byte bound",
            Self::LocaleTooLong => "composition locale exceeds the 64-byte bound",
            Self::SelectionReversed => "text selection start follows end",
            Self::SelectionOutOfBounds => "text selection exceeds the current value",
            Self::SelectionSplitsScalar => "text selection splits a UTF-16 surrogate pair",
        };
        formatter.write_str(message)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TextState {
    value: String,
    selection: Selection,
    composition: CompositionState,
    preedit: Option<String>,
    last_composition: Option<String>,
    last_input_data: Option<String>,
    locale: String,
    input_type: String,
    last_event: TextEvent,
}

impl Default for TextState {
    fn default() -> Self {
        Self::new(INITIAL_TEXT.to_owned()).expect("invariant: the seeded text specimen is bounded")
    }
}

impl TextState {
    pub(crate) fn new(value: String) -> Result<Self, TextError> {
        let value = bounded_text(value)?;
        let end = utf16_length(&value)?;
        Ok(Self {
            value,
            selection: Selection::new(end, end, SelectionDirection::None)?,
            composition: CompositionState::Inactive,
            preedit: None,
            last_composition: None,
            last_input_data: None,
            locale: String::new(),
            input_type: "insertText".to_owned(),
            last_event: TextEvent::Ready,
        })
    }

    pub(crate) fn apply_input(
        &mut self,
        value: String,
        data: Option<String>,
        input_type: String,
        composition: CompositionState,
        selection: Selection,
    ) -> Result<(), TextError> {
        let value = bounded_text(value)?;
        let data = data.map(bounded_metadata).transpose()?;
        let input_type = bounded_metadata(input_type)?;
        validate_selection(&value, selection)?;
        self.value = value;
        self.last_input_data = data;
        self.input_type = input_type;
        self.selection = selection;
        self.composition = composition;
        if matches!(composition, CompositionState::Inactive) {
            self.preedit = None;
        }
        self.last_event = TextEvent::Input;
        Ok(())
    }

    pub(crate) fn apply_selection(&mut self, selection: Selection) -> Result<(), TextError> {
        validate_selection(&self.value, selection)?;
        self.selection = selection;
        self.last_event = TextEvent::Selection;
        Ok(())
    }

    pub(crate) fn apply_composition(
        &mut self,
        phase: CompositionPhase,
        data: Option<String>,
        locale: String,
    ) -> Result<(), TextError> {
        let data = data.map(bounded_text).transpose()?;
        let locale = bounded_locale(locale)?;
        match phase {
            CompositionPhase::Start | CompositionPhase::Update => {
                self.preedit.clone_from(&data);
                self.composition = CompositionState::Active;
            }
            CompositionPhase::Commit | CompositionPhase::Cancel => {
                self.preedit = None;
                self.composition = CompositionState::Inactive;
            }
        }
        self.last_composition = data;
        self.locale = locale;
        self.last_event = TextEvent::Composition(phase);
        Ok(())
    }

    pub(crate) const fn selection(&self) -> Selection {
        self.selection
    }

    pub(crate) const fn composition(&self) -> CompositionState {
        self.composition
    }

    pub(crate) fn preedit(&self) -> Option<&str> {
        self.preedit.as_deref()
    }

    pub(crate) fn input_type(&self) -> &str {
        &self.input_type
    }

    pub(crate) fn value_display(&self) -> String {
        truncate_text(&self.value, MAX_DISPLAY_BYTES)
    }

    pub(crate) fn text_status(&self) -> String {
        match self.last_event {
            TextEvent::Ready => "Text: ready; Unicode specimen loaded".to_owned(),
            TextEvent::Input => format!(
                "Text: input {} applied; data {}",
                self.input_type,
                self.last_input_data.as_deref().map_or("none", |data| data)
            ),
            TextEvent::Composition(phase) => {
                format!("Text: composition {}", phase.label())
            }
            TextEvent::Selection => "Text: selection updated".to_owned(),
        }
    }

    pub(crate) fn composition_status(&self) -> String {
        match self.last_event {
            TextEvent::Composition(CompositionPhase::Start | CompositionPhase::Update)
                if self.preedit.is_some() =>
            {
                format!(
                    "Composition: composing {:?}; locale {}",
                    self.preedit(),
                    locale_label(&self.locale)
                )
            }
            TextEvent::Composition(phase) => format!(
                "Composition: {}; locale {}",
                phase.label(),
                locale_label(&self.locale)
            ),
            _ => format!(
                "Composition: idle; last data {}; locale {}",
                self.last_composition.as_deref().map_or("none", |data| data),
                locale_label(&self.locale)
            ),
        }
    }

    pub(crate) fn selection_status(&self) -> String {
        let selection = self.selection;
        let span = if selection.start == selection.end {
            format!("caret {}", selection.start)
        } else {
            format!("range {}–{}", selection.start, selection.end)
        };
        format!(
            "Selection: {span} UTF-16 code units; direction {}",
            selection.direction.label()
        )
    }

    pub(crate) fn state_name(&self) -> &'static str {
        match (self.composition, self.last_event) {
            (CompositionState::Active, _) => "composing",
            (_, TextEvent::Ready) => "ready",
            (_, TextEvent::Input) => "editing",
            (_, TextEvent::Selection) => "selected",
            (_, TextEvent::Composition(_)) => "composition",
        }
    }
}

fn bounded_text(value: String) -> Result<String, TextError> {
    if value.len() > MAX_TEXT_BYTES {
        return Err(TextError::TextTooLong);
    }
    Ok(value)
}

fn bounded_metadata(value: String) -> Result<String, TextError> {
    if value.len() > MAX_METADATA_BYTES {
        return Err(TextError::MetadataTooLong);
    }
    Ok(value)
}

fn bounded_locale(value: String) -> Result<String, TextError> {
    if value.len() > MAX_LOCALE_BYTES {
        return Err(TextError::LocaleTooLong);
    }
    Ok(value)
}

fn utf16_length(value: &str) -> Result<u32, TextError> {
    u32::try_from(value.encode_utf16().count()).map_err(|_| TextError::TextTooLong)
}

fn validate_selection(value: &str, selection: Selection) -> Result<(), TextError> {
    if selection.end > utf16_length(value)? {
        return Err(TextError::SelectionOutOfBounds);
    }
    for offset in [selection.start(), selection.end()] {
        if !is_utf16_boundary(value, offset)? {
            return Err(TextError::SelectionSplitsScalar);
        }
    }
    Ok(())
}

fn is_utf16_boundary(value: &str, offset: u32) -> Result<bool, TextError> {
    if offset == 0 {
        return Ok(true);
    }
    let mut position = 0_u32;
    for character in value.chars() {
        let width = u32::try_from(character.len_utf16()).map_err(|_| TextError::TextTooLong)?;
        position = position.checked_add(width).ok_or(TextError::TextTooLong)?;
        if position == offset {
            return Ok(true);
        }
        if position > offset {
            return Ok(false);
        }
    }
    Ok(false)
}

fn locale_label(locale: &str) -> &str {
    if locale.is_empty() {
        "unspecified"
    } else {
        locale
    }
}

fn truncate_text(value: &str, maximum_bytes: usize) -> String {
    if value.len() <= maximum_bytes {
        return value.to_owned();
    }
    let boundary = value
        .char_indices()
        .take_while(|(index, _)| *index < maximum_bytes)
        .map(|(index, character)| index + character.len_utf8())
        .last()
        .unwrap_or(0);
    format!("{}...", &value[..boundary])
}

#[cfg(test)]
mod tests {
    use super::{
        CompositionPhase, CompositionState, Selection, SelectionDirection, TextError, TextState,
    };

    #[test]
    fn unicode_values_preserve_utf16_selection_coordinates() {
        let mut state = TextState::new("A😀é".to_owned()).expect("Unicode value is bounded");
        let selection = Selection::new(1, 3, SelectionDirection::Backward)
            .expect("selection ordering is valid");
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

        let forward = Selection::new(0, 0, SelectionDirection::Forward)
            .expect("forward caret ordering is valid");
        state
            .apply_selection(forward)
            .expect("forward caret fits the value");
        assert_eq!(state.selection().direction(), SelectionDirection::Forward);
        assert_eq!(state.state_name(), "selected");
        let unknown = Selection::new(0, 0, SelectionDirection::Other)
            .expect("unknown caret ordering is valid");
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
}
