//! Browser input operation classification for the text boundary.
//!
//! The operation names follow the W3C [Input Events] vocabulary.
//!
//! [Input Events]: https://w3c.github.io/input-events/

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InputOperation {
    Edit,
    Delete,
    Paste,
    Cut,
    Undo,
    Redo,
    Composition,
    Other,
}

impl InputOperation {
    pub(crate) fn from_browser_input_type(input_type: &str) -> Self {
        match input_type {
            "insertFromPaste" | "insertFromPasteAsQuotation" => Self::Paste,
            "deleteByCut" => Self::Cut,
            "historyUndo" => Self::Undo,
            "historyRedo" => Self::Redo,
            "insertCompositionText" | "insertFromComposition" | "deleteByComposition" => {
                Self::Composition
            }
            "deleteContent"
            | "deleteContentBackward"
            | "deleteContentForward"
            | "deleteWordBackward"
            | "deleteWordForward"
            | "deleteSoftLineBackward"
            | "deleteSoftLineForward"
            | "deleteEntireSoftLine"
            | "deleteHardLineBackward"
            | "deleteHardLineForward"
            | "deleteByDrag" => Self::Delete,
            "insertText"
            | "insertReplacementText"
            | "insertFromDrop"
            | "insertFromYank"
            | "insertLineBreak"
            | "insertParagraph"
            | "insertTranspose" => Self::Edit,
            _ => Self::Other,
        }
    }

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Edit => "edit",
            Self::Delete => "delete",
            Self::Paste => "paste",
            Self::Cut => "cut",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::Composition => "composition",
            Self::Other => "other",
        }
    }
}
