//! Browser input operation classification for the text boundary.

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
            "insertFromPaste" => Self::Paste,
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
            | "deleteWordForward" => Self::Delete,
            "insertText" | "insertReplacementText" | "insertFromDrop" | "insertFromYank" => {
                Self::Edit
            }
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
