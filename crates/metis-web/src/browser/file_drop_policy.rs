//! Rust-owned policy for the browser file-drop workflow.

use std::fmt;

// The bound admits the largest committed Atlas DICOM study while the byte
// budget below remains the primary consumer memory limit.
const MAX_FILES: usize = 512;
const MAX_FILE_NAME_BYTES: usize = 4_096;
const MAX_MEDIA_TYPE_BYTES: usize = 256;
const MAX_DISPLAY_NAME_BYTES: usize = 96;
pub(crate) const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
pub(crate) const MAX_BATCH_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FileDropEntry {
    name: String,
    media_type: String,
    size_bytes: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum DropReadState {
    #[default]
    Idle,
    Reading,
    Complete {
        bytes_read: usize,
        files_read: usize,
    },
    Failed,
}

impl DropReadState {
    pub(crate) const fn state_name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Reading => "reading",
            Self::Complete { .. } => "complete",
            Self::Failed => "failed",
        }
    }

    pub(crate) fn status_message(self) -> String {
        match self {
            Self::Idle => "Byte access: waiting for a selected file".to_owned(),
            Self::Reading => "Byte access: reading a bounded file batch".to_owned(),
            Self::Complete {
                bytes_read,
                files_read,
            } => format!("Byte access: read {bytes_read} bytes from {files_read} file(s)"),
            Self::Failed => "Byte access: host rejected the selected file".to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PayloadSizeError {
    FileTooLarge,
    BatchTooLarge,
    SizeNotRepresentable,
}

pub(crate) fn check_payload_size(
    size_bytes: u64,
    total_bytes: u64,
) -> Result<usize, PayloadSizeError> {
    if size_bytes > MAX_FILE_BYTES {
        return Err(PayloadSizeError::FileTooLarge);
    }
    let next_total = total_bytes
        .checked_add(size_bytes)
        .ok_or(PayloadSizeError::BatchTooLarge)?;
    if next_total > MAX_BATCH_BYTES {
        return Err(PayloadSizeError::BatchTooLarge);
    }
    usize::try_from(size_bytes).map_err(|_| PayloadSizeError::SizeNotRepresentable)
}

impl fmt::Display for PayloadSizeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::FileTooLarge => "a browser file exceeds the 64 MiB payload bound",
            Self::BatchTooLarge => "the browser file batch exceeds the 256 MiB payload bound",
            Self::SizeNotRepresentable => {
                "a browser file size cannot be represented by this target"
            }
        };
        formatter.write_str(message)
    }
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

    fn display_name(&self) -> String {
        truncate_text(&self.name, MAX_DISPLAY_NAME_BYTES)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FileDropError {
    EmptyDrop,
    TooManyFiles,
    FileTooLarge,
    BatchTooLarge,
    SizeNotRepresentable,
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
            Self::TooManyFiles => "the drop exceeds the 512-file bound",
            Self::FileTooLarge => "a dropped file exceeds the 64 MiB payload bound",
            Self::BatchTooLarge => "the dropped batch exceeds the 256 MiB payload bound",
            Self::SizeNotRepresentable => {
                "a dropped file size cannot be represented by this target"
            }
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
        let mut total_bytes = 0_u64;
        for entry in entries {
            if accepted.len() == MAX_FILES {
                return Err(FileDropError::TooManyFiles);
            }
            check_payload_size(entry.size_bytes(), total_bytes).map_err(map_payload_error)?;
            total_bytes = total_bytes
                .checked_add(entry.size_bytes())
                .expect("invariant: accepted payload total passed the batch bound");
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

fn map_payload_error(error: PayloadSizeError) -> FileDropError {
    match error {
        PayloadSizeError::FileTooLarge => FileDropError::FileTooLarge,
        PayloadSizeError::BatchTooLarge => FileDropError::BatchTooLarge,
        PayloadSizeError::SizeNotRepresentable => FileDropError::SizeNotRepresentable,
    }
}

fn accepted_status(files: &[FileDropEntry]) -> String {
    let details = files
        .iter()
        .take(3)
        .map(|file| format!("{} ({} bytes)", file.display_name(), file.size_bytes()))
        .collect::<Vec<_>>()
        .join(", ");
    let suffix = if files.len() > 3 { "; ..." } else { "" };
    format!(
        "Drop status: accepted {} file(s); {}{}",
        files.len(),
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
    use super::{
        DropReadState, DropState, FileDropEntry, FileDropError, MAX_BATCH_BYTES, MAX_FILE_BYTES,
        MAX_FILES, PayloadSizeError, check_payload_size,
    };

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
            FileDropEntry::new("scan.bin".to_owned(), "x".repeat(257), 0)
                .expect_err("oversized media types must be rejected"),
            FileDropError::MediaTypeTooLong
        );
    }

    #[test]
    fn accepted_state_is_bounded_and_reports_metadata() {
        let state = DropState::accept([
            entry("scan.bin", "application/octet-stream"),
            entry("notes.txt", "text/plain"),
        ])
        .expect("bounded metadata is accepted");
        assert_eq!(state.file_count(), 2);
        assert_eq!(state.state_name(), "accepted");
        assert!(state.status_message().contains("accepted 2 file(s)"));
        assert!(state.status_message().contains("scan.bin (1024 bytes)"));
    }

    #[test]
    fn empty_and_oversized_drops_have_typed_rejections() {
        assert_eq!(MAX_FILES, 512);
        assert_eq!(
            DropState::accept([]).expect_err("empty drops must be rejected"),
            FileDropError::EmptyDrop
        );
        let admitted = std::iter::repeat_with(|| entry("scan.bin", "application/octet-stream"))
            .take(MAX_FILES);
        assert_eq!(
            DropState::accept(admitted)
                .expect("the bounded file-count edge must be accepted")
                .file_count(),
            MAX_FILES
        );
        let entries = std::iter::repeat_with(|| entry("scan.bin", "application/octet-stream"))
            .take(MAX_FILES + 1);
        assert_eq!(
            DropState::accept(entries).expect_err("oversized drops must be rejected"),
            FileDropError::TooManyFiles
        );
        assert_eq!(
            DropState::accept([FileDropEntry::new(
                "large.bin".to_owned(),
                "application/octet-stream".to_owned(),
                MAX_FILE_BYTES + 1,
            )
            .expect("metadata is valid")])
            .expect_err("oversized files must be rejected before reading"),
            FileDropError::FileTooLarge
        );
        assert_eq!(
            check_payload_size(1, u64::MAX),
            Err(PayloadSizeError::BatchTooLarge)
        );
    }

    #[test]
    fn long_names_are_truncated_without_splitting_utf8() {
        let state =
            DropState::accept([entry(&"é".repeat(100), "")]).expect("bounded metadata is accepted");
        let status = state.status_message();
        assert!(status.contains("... (1024 bytes)"));
    }

    #[test]
    fn byte_read_status_exposes_bounded_progress() {
        assert_eq!(DropReadState::default().state_name(), "idle");
        assert!(
            DropReadState::Reading
                .status_message()
                .contains("bounded file batch")
        );
        assert!(
            DropReadState::Complete {
                bytes_read: 132,
                files_read: 1,
            }
            .status_message()
            .contains("read 132 bytes from 1 file(s)")
        );
        assert_eq!(DropReadState::Failed.state_name(), "failed");
    }

    #[test]
    fn payload_budget_accepts_file_and_batch_edges() {
        assert_eq!(
            check_payload_size(MAX_FILE_BYTES, 0),
            Ok(usize::try_from(MAX_FILE_BYTES).expect("test bound fits target"))
        );
        assert_eq!(check_payload_size(1, MAX_BATCH_BYTES - 1), Ok(1));
        assert_eq!(
            check_payload_size(MAX_FILE_BYTES + 1, 0),
            Err(PayloadSizeError::FileTooLarge)
        );
        assert_eq!(
            check_payload_size(1, MAX_BATCH_BYTES),
            Err(PayloadSizeError::BatchTooLarge)
        );
    }
}
