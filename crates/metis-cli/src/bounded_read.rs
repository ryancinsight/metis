//! Reads capped at a byte budget, so oversized input is refused before it is held.

use crate::Result;
use std::{fs::File, io::Read, path::Path};

/// Reads `reader` to its end, failing with `exceeded` when it yields more
/// than `limit` bytes.
///
/// `expected` is the length the source declares, or zero when unknown. It is
/// reserved fallibly up front, so a declared size the host cannot hold is an
/// error instead of an abort.
pub(crate) fn read_bounded(
    reader: impl Read,
    limit: u64,
    expected: u64,
    exceeded: &str,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(usize::try_from(expected.min(limit))?)?;
    reader
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len())? > limit {
        return Err(exceeded.into());
    }
    Ok(bytes)
}

/// Opens `path` and reads it under `limit`, taking the declared length from
/// the open handle so the check and the read name the same file.
pub(crate) fn read_file(path: &Path, limit: u64, exceeded: &str) -> Result<Vec<u8>> {
    let file = File::open(path)?;
    let declared = file.metadata()?.len();
    if declared > limit {
        return Err(exceeded.into());
    }
    read_bounded(file, limit, declared, exceeded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Cursor};

    /// Yields `remaining` bytes and then fails, proving a read stops at the bound.
    struct FailsAfter {
        remaining: usize,
    }

    impl Read for FailsAfter {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if self.remaining == 0 {
                return Err(io::Error::other("read past the bound"));
            }
            let count = self.remaining.min(buffer.len());
            buffer.iter_mut().take(count).for_each(|byte| *byte = 7);
            self.remaining -= count;
            Ok(count)
        }
    }

    #[test]
    fn input_within_the_bound_is_returned_exactly() {
        let bytes = read_bounded(Cursor::new(b"abcd"), 4, 4, "too large").expect("bounded read");
        assert_eq!(bytes, b"abcd");
    }

    #[test]
    fn input_one_byte_over_the_bound_is_rejected() {
        let error = read_bounded(Cursor::new(b"abcde"), 4, 0, "too large").expect_err("bound");
        assert_eq!(error.to_string(), "too large");
    }

    #[test]
    fn an_over_bound_reader_is_not_read_to_its_end() {
        let error =
            read_bounded(FailsAfter { remaining: 5 }, 4, 0, "too large").expect_err("bound");
        assert_eq!(error.to_string(), "too large");
    }

    #[test]
    fn a_declared_length_over_the_bound_is_rejected() {
        let directory = std::env::temp_dir().join(format!("metis-bounded-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("directory");
        let path = directory.join("input");
        std::fs::write(&path, b"abcde").expect("input");
        let error = read_file(&path, 4, "too large").expect_err("declared length");
        assert_eq!(error.to_string(), "too large");
        assert_eq!(
            read_file(&path, 5, "too large").expect("at the bound"),
            b"abcde"
        );
        std::fs::remove_dir_all(directory).expect("cleanup");
    }
}
