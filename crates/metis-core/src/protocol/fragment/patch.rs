//! Encoding and validation for typed browser DOM patches.

use super::{
    MAX_FRAGMENT_ATTRIBUTE_BYTES, MAX_FRAGMENT_BODY_BYTES, MAX_FRAGMENT_PATCHES,
    MAX_FRAGMENT_TARGET_BYTES, MAX_FRAGMENT_VALUE_BYTES, PATCH_SET_PREFIX_BYTES,
    check_fragment_length, encode_string_u16, encode_string_u32, finish, malformed, take,
    take_string_u16, take_string_u32, too_large, validate_attribute_name, validate_identifier,
    validate_text,
};
use crate::error::Result;

/// One safe, typed mutation of an allowlisted browser target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FragmentPatch {
    /// Replaces a target's text content without parsing markup.
    SetText {
        /// Allowlisted DOM target identifier.
        target: String,
        /// Replacement text content.
        value: String,
    },
    /// Sets one bounded attribute; the browser policy still decides whether
    /// the specific name is permitted on its target.
    SetAttribute {
        /// Allowlisted DOM target identifier.
        target: String,
        /// Attribute name subject to the browser policy.
        name: String,
        /// Replacement attribute value.
        value: String,
    },
    /// Replaces children with text content, never an HTML string.
    ReplaceChildren {
        /// Allowlisted DOM target identifier.
        target: String,
        /// Text inserted as the only child content.
        text: String,
    },
}

impl FragmentPatch {
    /// Builds a text-content mutation.
    ///
    /// # Errors
    /// Returns a typed validation or resource-bound error for invalid target or
    /// text values.
    pub fn set_text(target: impl AsRef<str>, value: impl AsRef<str>) -> Result<Self> {
        let target = target.as_ref();
        let value = value.as_ref();
        validate_identifier(target, MAX_FRAGMENT_TARGET_BYTES, "target")?;
        validate_text(value, MAX_FRAGMENT_VALUE_BYTES, "text value")?;
        Ok(Self::SetText {
            target: target.to_owned(),
            value: value.to_owned(),
        })
    }

    /// Builds an attribute mutation with a syntactically valid name.
    ///
    /// The browser target policy applies the security allowlist for attribute
    /// authority; this constructor only validates the wire representation.
    ///
    /// # Errors
    /// Returns a typed validation or resource-bound error for invalid fields.
    pub fn set_attribute(
        target: impl AsRef<str>,
        name: impl AsRef<str>,
        value: impl AsRef<str>,
    ) -> Result<Self> {
        let target = target.as_ref();
        let name = name.as_ref();
        let value = value.as_ref();
        validate_identifier(target, MAX_FRAGMENT_TARGET_BYTES, "target")?;
        validate_attribute_name(name)?;
        validate_text(value, MAX_FRAGMENT_VALUE_BYTES, "attribute value")?;
        Ok(Self::SetAttribute {
            target: target.to_owned(),
            name: name.to_owned(),
            value: value.to_owned(),
        })
    }

    /// Builds a text-only child replacement.
    ///
    /// # Errors
    /// Returns a typed validation or resource-bound error for invalid target or
    /// text values.
    pub fn replace_children(target: impl AsRef<str>, text: impl AsRef<str>) -> Result<Self> {
        let target = target.as_ref();
        let text = text.as_ref();
        validate_identifier(target, MAX_FRAGMENT_TARGET_BYTES, "target")?;
        validate_text(text, MAX_FRAGMENT_VALUE_BYTES, "child text")?;
        Ok(Self::ReplaceChildren {
            target: target.to_owned(),
            text: text.to_owned(),
        })
    }

    /// Returns the target DOM identifier.
    #[must_use]
    pub fn target(&self) -> &str {
        match self {
            Self::SetText { target, .. }
            | Self::SetAttribute { target, .. }
            | Self::ReplaceChildren { target, .. } => target,
        }
    }

    /// Returns text content for a text or child replacement.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::SetText { value, .. } | Self::ReplaceChildren { text: value, .. } => Some(value),
            Self::SetAttribute { .. } => None,
        }
    }

    /// Returns an attribute name and value for an attribute mutation.
    #[must_use]
    pub fn attribute(&self) -> Option<(&str, &str)> {
        match self {
            Self::SetAttribute { name, value, .. } => Some((name, value)),
            Self::SetText { .. } | Self::ReplaceChildren { .. } => None,
        }
    }

    fn encoded_length(&self) -> Result<usize> {
        self.validate_fields()?;
        let length = match self {
            Self::SetText { target, value }
            | Self::ReplaceChildren {
                target,
                text: value,
            } => 1usize
                .checked_add(2)
                .and_then(|size| size.checked_add(4))
                .and_then(|size| size.checked_add(target.len()))
                .and_then(|size| size.checked_add(value.len())),
            Self::SetAttribute {
                target,
                name,
                value,
            } => 1usize
                .checked_add(2)
                .and_then(|size| size.checked_add(2))
                .and_then(|size| size.checked_add(4))
                .and_then(|size| size.checked_add(target.len()))
                .and_then(|size| size.checked_add(name.len()))
                .and_then(|size| size.checked_add(value.len())),
        }
        .ok_or_else(|| too_large("Fragment patch size overflows its resource bound"))?;
        if length > MAX_FRAGMENT_BODY_BYTES {
            return Err(too_large("Fragment patch exceeds its resource bound"));
        }
        Ok(length)
    }

    fn validate_fields(&self) -> Result<()> {
        match self {
            Self::SetText { target, value } => {
                validate_identifier(target, MAX_FRAGMENT_TARGET_BYTES, "target")?;
                validate_text(value, MAX_FRAGMENT_VALUE_BYTES, "text value")?;
            }
            Self::SetAttribute {
                target,
                name,
                value,
            } => {
                validate_identifier(target, MAX_FRAGMENT_TARGET_BYTES, "target")?;
                validate_attribute_name(name)?;
                validate_text(value, MAX_FRAGMENT_VALUE_BYTES, "attribute value")?;
            }
            Self::ReplaceChildren { target, text } => {
                validate_identifier(target, MAX_FRAGMENT_TARGET_BYTES, "target")?;
                validate_text(text, MAX_FRAGMENT_VALUE_BYTES, "child text")?;
            }
        }
        Ok(())
    }

    fn encode_into(&self, encoded: &mut Vec<u8>) -> Result<()> {
        match self {
            Self::SetText { target, value } => {
                encoded.push(1);
                encode_string_u16(encoded, target, "Fragment target identifier")?;
                encode_string_u32(encoded, value, "Fragment text value")?;
            }
            Self::SetAttribute {
                target,
                name,
                value,
            } => {
                encoded.push(2);
                encode_string_u16(encoded, target, "Fragment target identifier")?;
                encode_string_u16(encoded, name, "Fragment attribute name")?;
                encode_string_u32(encoded, value, "Fragment attribute value")?;
            }
            Self::ReplaceChildren { target, text } => {
                encoded.push(3);
                encode_string_u16(encoded, target, "Fragment target identifier")?;
                encode_string_u32(encoded, text, "Fragment child text")?;
            }
        }
        Ok(())
    }
}

/// A generation-bound, bounded sequence of safe DOM mutations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentPatchSet {
    generation: u64,
    patches: Vec<FragmentPatch>,
}

impl FragmentPatchSet {
    /// Builds a patch set after validating its generation, count, and body size.
    ///
    /// # Errors
    /// Returns a typed validation, allocation, or resource-bound error.
    pub fn new(generation: u64, patches: Vec<FragmentPatch>) -> Result<Self> {
        validate_patch_set(generation, &patches)?;
        Ok(Self {
            generation,
            patches,
        })
    }

    /// Returns the lifecycle generation that produced this response.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns patches in their atomic application order.
    #[must_use]
    pub fn patches(&self) -> &[FragmentPatch] {
        &self.patches
    }

    /// Encodes the patch set with exact field lengths.
    ///
    /// # Errors
    /// Returns a typed resource-bound error when the response cannot fit the
    /// control-plane body limit.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let total = validate_patch_set(self.generation, &self.patches)?;
        let mut encoded = Vec::with_capacity(total);
        encoded.extend_from_slice(&self.generation.to_be_bytes());
        encoded.extend_from_slice(
            &u16::try_from(self.patches.len())
                .map_err(|_| too_large("Fragment patch count exceeds its wire length"))?
                .to_be_bytes(),
        );
        encoded.extend_from_slice(&[0, 0]);
        for patch in &self.patches {
            patch.encode_into(&mut encoded)?;
        }
        Ok(encoded)
    }

    /// Decodes a patch set from untrusted bytes.
    ///
    /// # Errors
    /// Rejects unknown patch kinds, malformed fields, invalid UTF-8, trailing
    /// bytes, and responses over the control-plane bound.
    pub fn decode(mut payload: &[u8]) -> Result<Self> {
        check_fragment_length(payload.len())?;
        let generation = u64::from_be_bytes(take(&mut payload)?);
        let count = usize::from(u16::from_be_bytes(take(&mut payload)?));
        let reserved = take::<2>(&mut payload)?;
        if reserved != [0, 0] {
            return Err(malformed("Fragment patch reserved bytes are non-zero"));
        }
        if count > MAX_FRAGMENT_PATCHES {
            return Err(too_large("Fragment patch count exceeds its resource bound"));
        }
        let mut patches = Vec::new();
        patches
            .try_reserve_exact(count)
            .map_err(|_| too_large("Fragment patch allocation exceeds its resource bound"))?;
        for _ in 0..count {
            let kind = take::<1>(&mut payload)?[0];
            let patch = match kind {
                1 => FragmentPatch::set_text(
                    take_string_u16(
                        &mut payload,
                        MAX_FRAGMENT_TARGET_BYTES,
                        "Fragment target identifier",
                    )?,
                    take_string_u32(
                        &mut payload,
                        MAX_FRAGMENT_VALUE_BYTES,
                        "Fragment text value",
                    )?,
                )?,
                2 => FragmentPatch::set_attribute(
                    take_string_u16(
                        &mut payload,
                        MAX_FRAGMENT_TARGET_BYTES,
                        "Fragment target identifier",
                    )?,
                    take_string_u16(
                        &mut payload,
                        MAX_FRAGMENT_ATTRIBUTE_BYTES,
                        "Fragment attribute name",
                    )?,
                    take_string_u32(
                        &mut payload,
                        MAX_FRAGMENT_VALUE_BYTES,
                        "Fragment attribute value",
                    )?,
                )?,
                3 => FragmentPatch::replace_children(
                    take_string_u16(
                        &mut payload,
                        MAX_FRAGMENT_TARGET_BYTES,
                        "Fragment target identifier",
                    )?,
                    take_string_u32(
                        &mut payload,
                        MAX_FRAGMENT_VALUE_BYTES,
                        "Fragment child text",
                    )?,
                )?,
                _ => return Err(malformed("Fragment patch kind is unknown")),
            };
            patches.push(patch);
        }
        finish(payload)?;
        Self::new(generation, patches)
    }
}

fn validate_patch_set(generation: u64, patches: &[FragmentPatch]) -> Result<usize> {
    if generation == 0 {
        return Err(malformed("Fragment patch generation must be non-zero"));
    }
    if patches.len() > MAX_FRAGMENT_PATCHES {
        return Err(too_large("Fragment patch count exceeds its resource bound"));
    }
    let mut total = PATCH_SET_PREFIX_BYTES;
    for patch in patches {
        total = total
            .checked_add(patch.encoded_length()?)
            .ok_or_else(|| too_large("Fragment patch set size overflows its resource bound"))?;
    }
    check_fragment_length(total)?;
    Ok(total)
}
