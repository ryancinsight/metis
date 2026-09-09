//! Completed browser file batches handed to a trusted application consumer.

/// One browser-selected file copied into a bounded application-owned payload.
///
/// The payload contains the provider-validated display metadata and the exact
/// bytes read from the browser file handle. It does not contain a filesystem
/// path or a browser binding. Consumers should drop it after handing the bytes
/// to their decoder; the browser host retains at most one completed batch until
/// the WASM-only `take_file_drop` handoff consumes it.
#[derive(Eq, PartialEq)]
pub struct FileDropPayload {
    name: String,
    media_type: String,
    bytes: Box<[u8]>,
}

impl FileDropPayload {
    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn from_parts(name: String, media_type: String, bytes: Box<[u8]>) -> Self {
        Self {
            name,
            media_type,
            bytes,
        }
    }

    /// Returns the browser-provided name as display metadata.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the browser-provided media type, which may be empty.
    #[must_use]
    pub fn media_type(&self) -> &str {
        &self.media_type
    }

    /// Returns the bytes copied from the browser file handle.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the payload length in bytes.
    #[must_use]
    pub fn size_bytes(&self) -> usize {
        self.bytes.len()
    }

    /// Transfers the display metadata and byte buffer to the consumer.
    ///
    /// Moving this tuple does not copy the byte allocation. A decoder can use
    /// the returned name and media type while borrowing the returned bytes,
    /// or retain the buffer for an owned decode job.
    #[must_use]
    pub fn into_parts(self) -> (String, String, Box<[u8]>) {
        let Self {
            name,
            media_type,
            bytes,
        } = self;
        (name, media_type, bytes)
    }
}

/// A bounded, named byte batch ready for a trusted application decoder.
///
/// The browser host constructs this value only after validating every file
/// size and reading each file to its provider-reported end. A new drop replaces
/// an unconsumed batch, and stopping the host drops the batch with the rest of
/// the mounted state. Callers retrieve it through the WASM-only
/// `take_file_drop` handoff function.
#[derive(Eq, PartialEq)]
pub struct FileDropBatch {
    files: Box<[FileDropPayload]>,
    total_bytes: usize,
}

impl FileDropBatch {
    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn from_payloads(payloads: Vec<FileDropPayload>) -> Self {
        let total_bytes = payloads.iter().try_fold(0_usize, |total, payload| {
            total.checked_add(payload.size_bytes())
        });
        let total_bytes =
            total_bytes.expect("invariant: file payload sizes passed the bounded batch check");
        Self {
            files: payloads.into_boxed_slice(),
            total_bytes,
        }
    }

    /// Returns the files in their original browser drop order.
    #[must_use]
    pub fn files(&self) -> &[FileDropPayload] {
        &self.files
    }

    /// Returns the total number of bytes held by this batch.
    #[must_use]
    pub const fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Returns the number of files held by this batch.
    #[must_use]
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Transfers the complete payload collection to the consumer.
    ///
    /// The collection and each byte buffer move without copying. This is the
    /// ownership boundary for a decoder such as RITK: consume the batch, then
    /// borrow each payload's bytes for the synchronous load or move the
    /// buffers into a longer-lived decode task.
    #[must_use]
    pub fn into_files(self) -> Box<[FileDropPayload]> {
        self.files
    }
}

#[cfg(test)]
mod tests {
    use super::{FileDropBatch, FileDropPayload};

    #[test]
    fn batch_preserves_names_media_types_and_bytes() {
        let batch = FileDropBatch::from_payloads(vec![
            FileDropPayload::from_parts(
                "first.dcm".to_owned(),
                "application/dicom".to_owned(),
                Box::from([1, 2, 3]),
            ),
            FileDropPayload::from_parts("second.dcm".to_owned(), String::new(), Box::from([4, 5])),
        ]);
        assert_eq!(batch.file_count(), 2);
        assert_eq!(batch.total_bytes(), 5);
        assert_eq!(batch.files()[0].name(), "first.dcm");
        assert_eq!(batch.files()[0].media_type(), "application/dicom");
        assert_eq!(batch.files()[0].bytes(), [1, 2, 3]);
        assert_eq!(batch.files()[1].bytes(), [4, 5]);
    }

    #[test]
    fn consuming_batch_preserves_allocations_and_parts() {
        let batch = FileDropBatch::from_payloads(vec![FileDropPayload::from_parts(
            "study.dcm".to_owned(),
            "application/dicom".to_owned(),
            Box::from([7, 8, 9]),
        )]);
        let files_address = batch.files().as_ptr();
        let payload_address = batch.files()[0].bytes().as_ptr();

        let files = batch.into_files();
        assert_eq!(files.as_ptr(), files_address);
        let payload = files
            .into_vec()
            .pop()
            .expect("invariant: test batch contains one payload");
        let (name, media_type, bytes) = payload.into_parts();

        assert_eq!(name, "study.dcm");
        assert_eq!(media_type, "application/dicom");
        assert_eq!(bytes.as_ptr(), payload_address);
        assert_eq!(&*bytes, [7, 8, 9]);
    }
}
