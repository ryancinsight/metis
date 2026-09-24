//! One running instance per application, with later launches forwarded.
//!
//! This is the counterpart of Tauri's single-instance plugin. The first
//! launch that calls [`claim_or_forward`] becomes the primary; a later launch
//! sends its arguments to it and should exit. The primary polls
//! [`PrimaryInstance::try_receive`] from its event loop and handles each
//! forwarded argument list, for example by passing it to
//! `metis_core::deep_link::DeepLink::from_arguments` and focusing its window.
//! Moirai owns the operating-system claim; this module owns the argument
//! encoding and its bounds.

use moirai_pal::instance::{self, InstanceName, InstanceRole, MAX_INSTANCE_MESSAGE_BYTES};
use std::io;

/// Maximum arguments forwarded from one launch.
pub const MAX_FORWARDED_ARGUMENTS: usize = 64;

/// The outcome of [`claim_or_forward`].
#[derive(Debug)]
pub enum Launch {
    /// This process is the running instance.
    Primary(PrimaryInstance),
    /// The arguments went to the running instance; this process should exit.
    Forwarded,
}

/// The running instance, receiving later launches' arguments.
#[derive(Debug)]
pub struct PrimaryInstance {
    inner: instance::PrimaryInstance,
}

impl PrimaryInstance {
    /// The next forwarded argument list, without blocking when none waits.
    ///
    /// # Errors
    /// Returns the transport error, or `InvalidData` for a message that is
    /// not an argument list this module encoded.
    pub fn try_receive(&mut self) -> io::Result<Option<Vec<String>>> {
        self.inner
            .try_receive()?
            .map(|message| decode_arguments(&message))
            .transpose()
    }
}

/// Claims `name` for this process or forwards `arguments` to its holder.
///
/// `name` follows Moirai's instance-name rule: 1 to 64 lowercase letters,
/// digits, `.` and `-`, such as a reverse-DNS application identifier.
///
/// # Errors
/// Returns `InvalidInput` for an invalid name, too many arguments or an
/// encoding over the message bound, or the transport error.
pub fn claim_or_forward<I, S>(name: &str, arguments: I) -> io::Result<Launch>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let message = encode_arguments(arguments)?;
    match instance::claim(&InstanceName::new(name)?)? {
        InstanceRole::Primary(inner) => Ok(Launch::Primary(PrimaryInstance { inner })),
        InstanceRole::Secondary(secondary) => {
            secondary.send(&message)?;
            Ok(Launch::Forwarded)
        }
    }
}

/// Encodes arguments as NUL-terminated UTF-8 strings.
fn encode_arguments<I, S>(arguments: I) -> io::Result<Vec<u8>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut message = Vec::new();
    for (index, argument) in arguments.into_iter().enumerate() {
        let argument = argument.as_ref();
        if index >= MAX_FORWARDED_ARGUMENTS
            || argument.contains('\0')
            || message.len() + argument.len() + 1 > MAX_INSTANCE_MESSAGE_BYTES
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "forwarded arguments exceed their bounds or contain NUL",
            ));
        }
        message.extend_from_slice(argument.as_bytes());
        message.push(0);
    }
    Ok(message)
}

fn decode_arguments(message: &[u8]) -> io::Result<Vec<String>> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidData, "malformed forwarded arguments");
    let body = match message {
        [] => return Ok(Vec::new()),
        [body @ .., 0] => body,
        _ => return Err(invalid()),
    };
    let arguments = body
        .split(|byte| *byte == 0)
        .map(|argument| String::from_utf8(argument.to_vec()).map_err(|_| invalid()))
        .collect::<io::Result<Vec<_>>>()?;
    if arguments.len() > MAX_FORWARDED_ARGUMENTS {
        return Err(invalid());
    }
    Ok(arguments)
}

#[cfg(test)]
mod tests;
