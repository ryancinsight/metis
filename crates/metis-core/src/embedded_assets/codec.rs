//! METISAS1 big-endian fixed-schema codec.
mod decode;
mod encode;
pub(super) use decode::decode;
pub(super) use encode::encode;
pub(crate) const MAGIC: &[u8; 8] = b"METISAS1";
pub(crate) const SCHEMA: u16 = 1;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use decode::Reader;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use encode::Writer;
