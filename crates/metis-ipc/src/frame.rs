//! Length-prefixed frame I/O with distinct clean EOF and truncated-frame errors.

use metis_core::crypto::crc32;
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::protocol::{FrameHeader, HEADER_SIZE, MessageType, build_frame};
use std::io::{ErrorKind, Read, Write};

/// Reads one frame. EOF is clean only before the first header byte.
///
/// The caller must configure a deadline on blocking I/O implementations.
/// # Errors
/// Returns transport, truncation, header-validation, or checksum errors.
pub fn read_frame<R: Read>(reader: &mut R) -> Result<(FrameHeader, Vec<u8>)> {
    let mut header_buf = [0; HEADER_SIZE];
    read_section(reader, &mut header_buf, true)?;
    let header = FrameHeader::decode(&header_buf)?;
    let mut payload = vec![0; header.payload_len as usize];
    read_section(reader, &mut payload, false)?;
    if crc32(&payload) != header.payload_crc32 {
        return Err(MetisError::protocol(
            ErrorCode::ChecksumMismatch,
            "Payload checksum does not match the frame header",
        ));
    }
    Ok((header, payload))
}

fn read_section<R: Read>(reader: &mut R, mut section: &mut [u8], header: bool) -> Result<()> {
    let mut started = !header;
    while !section.is_empty() {
        match reader.read(section) {
            Ok(0) => {
                return Err(MetisError::transport(
                    if started {
                        ErrorCode::FrameTruncated
                    } else {
                        ErrorCode::ConnectionClosed
                    },
                    if started {
                        "EOF inside a frame"
                    } else {
                        "Peer closed between frames"
                    },
                ));
            }
            Ok(count) => {
                started = true;
                section = &mut section[count..];
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) => return Err(io_error(&error)),
        }
    }
    Ok(())
}

pub(crate) fn io_error(error: &std::io::Error) -> MetisError {
    let code = match error.kind() {
        ErrorKind::TimedOut | ErrorKind::WouldBlock => ErrorCode::Timeout,
        _ => ErrorCode::TransportBroken,
    };
    MetisError::transport(code, error.to_string())
}

/// Encodes and writes one bounded frame, then flushes the stream.
/// # Errors
/// Returns an oversized-payload error or the underlying transport failure.
pub fn write_frame<W: Write>(
    writer: &mut W,
    msg_type: MessageType,
    sequence_id: u64,
    payload: &[u8],
) -> Result<()> {
    let frame = build_frame(msg_type, sequence_id, payload)?;
    write_wire(writer, &frame)
}

pub(crate) fn write_wire<W: Write>(writer: &mut W, frame: &[u8]) -> Result<()> {
    writer.write_all(frame).map_err(|error| io_error(&error))?;
    writer.flush().map_err(|error| io_error(&error))
}
