//! Length-prefixed frame I/O with distinct clean EOF and truncated-frame errors.

use metis_core::crypto::crc32;
use metis_core::error::{ErrorCode, MetisError, Result};
use metis_core::protocol::{
    FrameHeader, HEADER_SIZE, MessageType, build_frame, reserve_frame_bytes,
};
use std::io::{ErrorKind, Read, Write};

/// Reads one frame. EOF is clean only before the first header byte.
///
/// The payload buffer is the only allocation, sized exactly once after the
/// header validates the payload length against the wire bound, so a hostile
/// length field cannot size an allocation beyond `MAX_PAYLOAD_SIZE`.
///
/// The caller must configure a deadline on blocking I/O implementations.
/// # Errors
/// Returns transport, truncation, header-validation, allocation, or checksum
/// errors.
pub fn read_frame<R: Read>(reader: &mut R) -> Result<(FrameHeader, Vec<u8>)> {
    let mut header_buf = [0; HEADER_SIZE];
    read_header(reader, &mut header_buf)?;
    let header = FrameHeader::decode(&header_buf)?;
    let length = payload_length(&header)?;
    let mut payload = Vec::new();
    reserve_frame_bytes(&mut payload, length)?;
    // `take` bounds the read to the validated length; `read_to_end` fills the
    // reserved capacity in place and never grows it, because the limit is the
    // capacity.
    let read = reader
        .by_ref()
        .take(header.payload_len.into())
        .read_to_end(&mut payload)
        .map_err(|error| io_error(&error))?;
    if read < length {
        return Err(MetisError::transport(
            ErrorCode::FrameTruncated,
            "EOF inside a frame",
        ));
    }
    verify_checksum(&header, &payload)?;
    Ok((header, payload))
}

/// Validates one complete wire frame held in `wire` and returns its payload.
///
/// The payload reuses `wire`'s allocation: the header is removed in place, so
/// a transport that already owns the received bytes decodes them without a
/// second allocation or copy of the payload. Errors match [`read_frame`] on
/// the same bytes, and bytes after the frame are `MalformedPayload`.
pub(crate) fn split_frame(mut wire: Vec<u8>) -> Result<(FrameHeader, Vec<u8>)> {
    if wire.is_empty() {
        return Err(MetisError::protocol(
            ErrorCode::FrameTruncated,
            "Empty wire frame received",
        ));
    }
    let Some(header_bytes) = wire.first_chunk::<HEADER_SIZE>() else {
        return Err(MetisError::transport(
            ErrorCode::FrameTruncated,
            "EOF inside a frame",
        ));
    };
    let header = FrameHeader::decode(header_bytes)?;
    let end = HEADER_SIZE + payload_length(&header)?;
    let Some(payload) = wire.get(HEADER_SIZE..end) else {
        return Err(MetisError::transport(
            ErrorCode::FrameTruncated,
            "EOF inside a frame",
        ));
    };
    verify_checksum(&header, payload)?;
    if wire.len() > end {
        return Err(MetisError::protocol(
            ErrorCode::MalformedPayload,
            "Trailing bytes after Metis frame",
        ));
    }
    wire.drain(..HEADER_SIZE);
    Ok((header, wire))
}

fn payload_length(header: &FrameHeader) -> Result<usize> {
    usize::try_from(header.payload_len).map_err(|_| {
        MetisError::protocol(
            ErrorCode::PayloadTooLarge,
            "Payload length cannot fit the address space",
        )
    })
}

fn verify_checksum(header: &FrameHeader, payload: &[u8]) -> Result<()> {
    if crc32(payload) != header.payload_crc32 {
        return Err(MetisError::protocol(
            ErrorCode::ChecksumMismatch,
            "Payload checksum does not match the frame header",
        ));
    }
    Ok(())
}

/// Fills `header`, reporting EOF before its first byte as a clean close.
fn read_header<R: Read>(reader: &mut R, mut header: &mut [u8]) -> Result<()> {
    let mut started = false;
    while !header.is_empty() {
        match reader.read(header) {
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
                header = &mut header[count..];
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

/// Encodes one bounded frame into `frame`, writes it, then flushes the stream.
///
/// `frame` is the caller's reusable encode buffer; see
/// [`build_frame`] for its capacity contract.
/// # Errors
/// Returns an oversized-payload error or the underlying transport failure.
pub fn write_frame<W: Write>(
    writer: &mut W,
    msg_type: MessageType,
    sequence_id: u64,
    payload: &[u8],
    frame: &mut Vec<u8>,
) -> Result<()> {
    build_frame(msg_type, sequence_id, payload, frame)?;
    write_wire(writer, frame)
}

pub(crate) fn write_wire<W: Write>(writer: &mut W, frame: &[u8]) -> Result<()> {
    writer.write_all(frame).map_err(|error| io_error(&error))?;
    writer.flush().map_err(|error| io_error(&error))
}
