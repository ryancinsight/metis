//! Rust-owned policy for the browser file-drop workflow.

use std::fmt;

const MAX_FILES: usize = 64;
const MAX_FILE_NAME_BYTES: usize = 4_096;
const MAX_MEDIA_TYPE_BYTES: usize = 256;
const MAX_DISPLAY_NAME_BYTES: usize = 96;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FileDropEntry {
    name: String,
    media_type: String,
    size_bytes: u64,
}

impl FileDropEntry {
    pub(crate) fn new(
        name: String,
        media_type: String,
        size_bytes: u64,
    ) -> Result<Self, FileDropError> {
        if name.is_empty() {
            return Err(FileDropError::EmptyName);
        }
        if name.len() > MAX_FILE_NAME_BYTES {
            return Err(FileDropError::NameTooLong);
        }
        if name.contains('\0') {
            return Err(FileDropError::EmbeddedNul);
        }
        if media_type.len() > MAX_MEDIA_TYPE_BYTES {
            return Err(FileDropError::MediaTypeTooLong);
        }
        Ok(Self {
            name,
            media_type,
            size_bytes,
        })
    }

    pub(crate) const fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    fn is_dicom_candidate(&self) -> bool {
        self.media_type.eq_ignore_ascii_case("application/dicom")
            || self.name.rsplit_once('.').is_some_and(|(_, extension)| {
                extension.eq_ignore_ascii_case("dcm") || extension.eq_ignore_ascii_case("dicom")
            })
    }

    fn display_name(&self) -> String {
        truncate_text(&self.name, MAX_DISPLAY_NAME_BYTES)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FileDropError {
    EmptyDrop,
    TooManyFiles,
    EmptyName,
    NameTooLong,
    EmbeddedNul,
    MediaTypeTooLong,
    #[cfg(target_arch = "wasm32")]
    ProviderMetadata,
}

impl fmt::Display for FileDropError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::EmptyDrop => "the drop did not contain files",
            Self::TooManyFiles => "the drop exceeds the 64-file bound",
            Self::EmptyName => "a dropped file has an empty name",
            Self::NameTooLong => "a dropped file name exceeds the 4096-byte bound",
            Self::EmbeddedNul => "a dropped file name contains NUL",
            Self::MediaTypeTooLong => "a dropped file media type exceeds the 256-byte bound",
            #[cfg(target_arch = "wasm32")]
            Self::ProviderMetadata => "the browser supplied invalid file metadata",
        };
        formatter.write_str(message)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum DropState {
    #[default]
    Idle,
    #[cfg(target_arch = "wasm32")]
    Hovering,
    Accepted(Box<[FileDropEntry]>),
    #[cfg(target_arch = "wasm32")]
    Rejected(FileDropError),
}

impl DropState {
    #[cfg(target_arch = "wasm32")]
    pub(crate) const fn hovering() -> Self {
        Self::Hovering
    }

    pub(crate) fn accept<I>(entries: I) -> Result<Self, FileDropError>
    where
        I: IntoIterator<Item = FileDropEntry>,
    {
        let mut accepted = Vec::with_capacity(MAX_FILES);
        for entry in entries {
            if accepted.len() == MAX_FILES {
                return Err(FileDropError::TooManyFiles);
            }
            accepted.push(entry);
        }
        if accepted.is_empty() {
            return Err(FileDropError::EmptyDrop);
        }
        Ok(Self::Accepted(accepted.into_boxed_slice()))
    }

    pub(crate) const fn state_name(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            #[cfg(target_arch = "wasm32")]
            Self::Hovering => "hovering",
            Self::Accepted(_) => "accepted",
            #[cfg(target_arch = "wasm32")]
            Self::Rejected(_) => "rejected",
        }
    }

    pub(crate) const fn file_count(&self) -> usize {
        match self {
            Self::Accepted(files) => files.len(),
            Self::Idle => 0,
            #[cfg(target_arch = "wasm32")]
            Self::Hovering | Self::Rejected(_) => 0,
        }
    }

    pub(crate) fn status_message(&self) -> String {
        match self {
            Self::Idle => "Drop status: ready; no files captured".to_owned(),
            #[cfg(target_arch = "wasm32")]
            Self::Hovering => "Drop status: release to inspect bounded file metadata".to_owned(),
            #[cfg(target_arch = "wasm32")]
            Self::Rejected(error) => format!("Drop status: rejected ({error})"),
            Self::Accepted(files) => accepted_status(files),
        }
    }
}

fn accepted_status(files: &[FileDropEntry]) -> String {
    let dicom_count = files
        .iter()
        .filter(|file| file.is_dicom_candidate())
        .count();
    let details = files
        .iter()
        .take(3)
        .map(|file| format!("{} ({} bytes)", file.display_name(), file.size_bytes()))
        .collect::<Vec<_>>()
        .join(", ");
    let suffix = if files.len() > 3 { "; ..." } else { "" };
    format!(
        "Drop status: accepted {} file(s); DICOM candidates {}; {}{}",
        files.len(),
        dicom_count,
        details,
        suffix
    )
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
    use super::{DropState, FileDropEntry, FileDropError, MAX_FILES};

    fn entry(name: &str, media_type: &str) -> FileDropEntry {
        FileDropEntry::new(name.to_owned(), media_type.to_owned(), 1024)
            .expect("test metadata is valid")
    }

    #[test]
    fn metadata_validation_rejects_unbounded_or_ambiguous_values() {
        assert_eq!(
            FileDropEntry::new(String::new(), String::new(), 0)
                .expect_err("empty names must be rejected"),
            FileDropError::EmptyName
        );
        assert_eq!(
            FileDropEntry::new("bad\0name".to_owned(), String::new(), 0)
                .expect_err("NUL names must be rejected"),
            FileDropError::EmbeddedNul
        );
        assert_eq!(
            FileDropEntry::new("x".repeat(4_097), String::new(), 0)
                .expect_err("oversized names must be rejected"),
            FileDropError::NameTooLong
        );
        assert_eq!(
            FileDropEntry::new("scan.dcm".to_owned(), "x".repeat(257), 0)
                .expect_err("oversized media types must be rejected"),
            FileDropError::MediaTypeTooLong
        );
    }

    #[test]
    fn accepted_state_is_bounded_and_reports_dicom_candidates() {
        let state = DropState::accept([
            entry("scan.dcm", "application/dicom"),
            entry("notes.txt", "text/plain"),
        ])
        .expect("bounded metadata is accepted");
        assert_eq!(state.file_count(), 2);
        assert_eq!(state.state_name(), "accepted");
        assert!(state.status_message().contains("DICOM candidates 1"));
        assert!(state.status_message().contains("scan.dcm (1024 bytes)"));
    }

    #[test]
    fn empty_and_oversized_drops_have_typed_rejections() {
        assert_eq!(
            DropState::accept([]).expect_err("empty drops must be rejected"),
            FileDropError::EmptyDrop
        );
        let entries =
            std::iter::repeat_with(|| entry("scan.dcm", "application/dicom")).take(MAX_FILES + 1);
        assert_eq!(
            DropState::accept(entries).expect_err("oversized drops must be rejected"),
            FileDropError::TooManyFiles
        );
    }

    #[test]
    fn long_names_are_truncated_without_splitting_utf8() {
        let state =
            DropState::accept([entry(&"é".repeat(100), "")]).expect("bounded metadata is accepted");
        let status = state.status_message();
        assert!(status.contains("... (1024 bytes)"));
    }
}
