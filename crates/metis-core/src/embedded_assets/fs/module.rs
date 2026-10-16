//! Admitted WASM retains its actual required function export.
use super::super::inventory::invalid;
use std::io;
pub(super) fn function_export(bytes: &[u8], required_export: &str) -> io::Result<u32> {
    if !bytes.starts_with(b"\0asm\x01\0\0\0") {
        return Err(invalid("prepared WASM magic/version is invalid"));
    }
    let mut module = Reader {
        bytes: &bytes[8..],
        offset: 0,
    };
    let mut found = None;
    let mut export_section = false;
    while module.offset < module.bytes.len() {
        let tag = module.byte()?;
        if tag > 13 {
            return Err(invalid("unknown WASM section tag"));
        }
        let length = usize::try_from(module.number()?)
            .map_err(|_| invalid("WASM section width overflow"))?;
        let section = module.take(length)?;
        if tag == 7 {
            if export_section {
                return Err(invalid("duplicate WASM export section"));
            }
            export_section = true;
            let mut exports = Reader {
                bytes: section,
                offset: 0,
            };
            let count = exports.number()?;
            if usize::try_from(count).map_err(|_| invalid("WASM export count overflow"))?
                > section.len()
            {
                return Err(invalid("WASM export count exceeds remaining bytes"));
            }
            for _ in 0..count {
                let length = usize::try_from(exports.number()?)
                    .map_err(|_| invalid("WASM export name width overflow"))?;
                let name = std::str::from_utf8(exports.take(length)?)
                    .map_err(|error| invalid(format!("WASM export UTF-8: {error}")))?;
                let kind = exports.byte()?;
                let index = exports.number()?;
                if name == required_export {
                    if kind != 0 || found.is_some() {
                        return Err(invalid(
                            "required WASM export is duplicate or not a function",
                        ));
                    }
                    found = Some(index);
                }
            }
            if exports.offset != section.len() {
                return Err(invalid("trailing WASM export bytes"));
            }
        }
    }
    found.ok_or_else(|| invalid("prepared WASM omits required embedded Rust function export"))
}
struct Reader<'bytes> {
    bytes: &'bytes [u8],
    offset: usize,
}
impl<'bytes> Reader<'bytes> {
    fn take(&mut self, count: usize) -> io::Result<&'bytes [u8]> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or_else(|| invalid("WASM offset overflow"))?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| invalid("truncated WASM field"))?;
        self.offset = end;
        Ok(bytes)
    }
    fn byte(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn number(&mut self) -> io::Result<u32> {
        let mut value = 0_u32;
        for offset in [0, 7, 14, 21, 28] {
            let byte = self.byte()?;
            if offset == 28 && byte & 0xf0 != 0 {
                return Err(invalid("WASM LEB integer overflow"));
            }
            value |= u32::from(byte & 0x7f) << offset;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(invalid("WASM LEB integer overflow"))
    }
}

#[cfg(test)]
mod tests {
    use super::function_export;
    use crate::embedded_assets::tests::traced;

    #[test]
    fn actual_export_section_preserves_function_index_and_rejects_wrong_role() {
        traced(|| {
            let bytes = {
                let _phase = tracing::info_span!("arrange").entered();
                // WebAssembly core binary: () -> i32, returning the analytical
                // value 3. This tests binary admission, not page execution.
                let mut bytes = b"\0asm\x01\0\0\0".to_vec();
                bytes.extend_from_slice(&[1, 5, 1, 0x60, 0, 1, 0x7f, 3, 2, 1, 0, 7, 24, 1, 20]);
                bytes.extend_from_slice(b"metis_embedded_start");
                bytes.extend_from_slice(&[0, 0, 10, 6, 1, 4, 0, 0x41, 3, 0x0b]);
                bytes
            };
            let index = {
                let _phase = tracing::info_span!("act").entered();
                function_export(&bytes, "metis_embedded_start").expect("actual function export")
            };
            let _phase = tracing::info_span!("assert").entered();
            assert_eq!(index, 0);
            assert_eq!(
                function_export(&bytes, "another_entry")
                    .expect_err("missing function")
                    .to_string(),
                "prepared WASM omits required embedded Rust function export"
            );
            let mut wrong_role = bytes;
            wrong_role[43] = 1;
            assert_eq!(
                function_export(&wrong_role, "metis_embedded_start")
                    .expect_err("table is not a function")
                    .to_string(),
                "required WASM export is duplicate or not a function"
            );
        });
    }
}
