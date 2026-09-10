//! Encoding and validation for generation-bound browser actions.

use super::{
    MAX_FRAGMENT_ACTION_BYTES, MAX_FRAGMENT_TARGET_BYTES, MAX_FRAGMENT_VALUE_BYTES, action_length,
    check_fragment_length, finish, malformed, take, take_string, too_large, validate_identifier,
    validate_text,
};
use crate::error::Result;

/// One authenticated, generation-bound action from a browser mount.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentAction {
    generation: u64,
    action: String,
    target: String,
    input: String,
}

impl FragmentAction {
    /// Builds an action with bounded identifiers and input.
    ///
    /// `generation` must be non-zero and is compared with the browser mount
    /// generation before any returned patch is applied.
    ///
    /// # Errors
    /// Returns a typed malformed-payload or resource-bound error when a field
    /// violates its identifier or byte-length contract.
    pub fn new(
        generation: u64,
        action: impl AsRef<str>,
        target: impl AsRef<str>,
        input: impl AsRef<str>,
    ) -> Result<Self> {
        if generation == 0 {
            return Err(malformed("Fragment action generation must be non-zero"));
        }
        let action = action.as_ref();
        let target = target.as_ref();
        let input = input.as_ref();
        validate_identifier(action, MAX_FRAGMENT_ACTION_BYTES, "action")?;
        validate_identifier(target, MAX_FRAGMENT_TARGET_BYTES, "target")?;
        validate_text(input, MAX_FRAGMENT_VALUE_BYTES, "input")?;
        action_length(action, target, input.len())?;
        Ok(Self {
            generation,
            action: action.to_owned(),
            target: target.to_owned(),
            input: input.to_owned(),
        })
    }

    /// Returns the lifecycle generation that issued this action.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns the typed action identifier.
    #[must_use]
    pub fn action(&self) -> &str {
        &self.action
    }

    /// Returns the target DOM identifier.
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Returns the bounded user input carried by this action.
    #[must_use]
    pub fn input(&self) -> &str {
        &self.input
    }

    /// Encodes the action using exact field lengths.
    ///
    /// # Errors
    /// Returns a typed resource-bound error if the validated action cannot fit
    /// the fragment body limit.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let total = action_length(&self.action, &self.target, self.input.len())?;
        let mut encoded = Vec::with_capacity(total);
        encoded.extend_from_slice(&self.generation.to_be_bytes());
        encoded.extend_from_slice(
            &u16::try_from(self.action.len())
                .map_err(|_| too_large("Fragment action identifier exceeds its wire length"))?
                .to_be_bytes(),
        );
        encoded.extend_from_slice(
            &u16::try_from(self.target.len())
                .map_err(|_| too_large("Fragment target identifier exceeds its wire length"))?
                .to_be_bytes(),
        );
        encoded.extend_from_slice(
            &u32::try_from(self.input.len())
                .map_err(|_| too_large("Fragment action input exceeds its wire length"))?
                .to_be_bytes(),
        );
        encoded.extend_from_slice(self.action.as_bytes());
        encoded.extend_from_slice(self.target.as_bytes());
        encoded.extend_from_slice(self.input.as_bytes());
        Ok(encoded)
    }

    /// Decodes an action from untrusted bytes.
    ///
    /// # Errors
    /// Rejects zero generations, invalid UTF-8, invalid identifiers, trailing
    /// bytes, and bodies over the control-plane bound.
    pub fn decode(mut payload: &[u8]) -> Result<Self> {
        check_fragment_length(payload.len())?;
        let generation = u64::from_be_bytes(take(&mut payload)?);
        let action_len = usize::from(u16::from_be_bytes(take(&mut payload)?));
        let target_len = usize::from(u16::from_be_bytes(take(&mut payload)?));
        let input_len = usize::try_from(u32::from_be_bytes(take(&mut payload)?))
            .map_err(|_| too_large("Fragment action input length cannot fit this target"))?;
        let action = take_string(
            &mut payload,
            action_len,
            MAX_FRAGMENT_ACTION_BYTES,
            "Fragment action identifier",
        )?;
        let target = take_string(
            &mut payload,
            target_len,
            MAX_FRAGMENT_TARGET_BYTES,
            "Fragment target identifier",
        )?;
        let input = take_string(
            &mut payload,
            input_len,
            MAX_FRAGMENT_VALUE_BYTES,
            "Fragment action input",
        )?;
        finish(payload)?;
        Self::new(generation, action, target, input)
    }
}
