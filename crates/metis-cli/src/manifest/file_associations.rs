//! Validation of the document types an application opens.

use crate::Result;
use serde::{Deserialize, Serialize};

/// Most document types one application may register.
pub(crate) const FILE_ASSOCIATION_LIMIT: usize = 8;
/// Most extensions one document type may claim.
pub(crate) const EXTENSION_LIMIT: usize = 8;
const EXTENSION_BYTES: usize = 16;
const MIME_TYPE_BYTES: usize = 96;
const DESCRIPTION_CHARS: usize = 64;

/// A document type the installers associate with the application, the
/// counterpart of a Tauri bundle `fileAssociations` entry.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FileAssociation {
    /// Lowercase extensions without the dot, such as `dcm`.
    pub(crate) extensions: Vec<String>,
    /// The document's MIME type, such as `application/dicom`.
    pub(crate) mime_type: String,
    /// A short human-readable name, such as `DICOM image`.
    pub(crate) description: String,
}

/// Admits associations whose extensions, MIME types and descriptions every
/// installer can write without escaping into a command or registry path.
pub(super) fn validate(associations: &[FileAssociation]) -> Result<()> {
    if associations.len() > FILE_ASSOCIATION_LIMIT {
        return Err("an application registers at most 8 file associations".into());
    }
    let mut extensions = Vec::new();
    for (index, association) in associations.iter().enumerate() {
        let field = |name: &str| format!("file_associations[{index}].{name}");
        if association.extensions.is_empty() || association.extensions.len() > EXTENSION_LIMIT {
            return Err(format!("{} needs 1 to 8 extensions", field("extensions")).into());
        }
        for extension in &association.extensions {
            let valid = !extension.is_empty()
                && extension.len() <= EXTENSION_BYTES
                && extension
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit());
            if !valid {
                return Err(format!(
                    "{} {extension:?} must be 1 to 16 lowercase letters or digits",
                    field("extensions")
                )
                .into());
            }
            if extensions.contains(&extension) {
                return Err(format!("{} repeats {extension}", field("extensions")).into());
            }
            extensions.push(extension);
        }
        if !mime_type_is_valid(&association.mime_type)
            || associations[..index]
                .iter()
                .any(|earlier| earlier.mime_type == association.mime_type)
        {
            return Err(format!("{} is not a unique type/subtype", field("mime_type")).into());
        }
        let description = &association.description;
        // The MSI Registry table reads a leading `#` as a numeric value.
        if description.trim().is_empty()
            || description.starts_with('#')
            || description.chars().count() > DESCRIPTION_CHARS
            || description
                .chars()
                .any(|c| c.is_control() || matches!(c, '[' | ']' | '{' | '}'))
        {
            return Err(format!(
                "{} must be 1 to 64 characters without control or formatted syntax or a leading #",
                field("description")
            )
            .into());
        }
    }
    Ok(())
}

/// `type/subtype` in lowercase ASCII token characters.
fn mime_type_is_valid(value: &str) -> bool {
    let token = |part: &str| {
        part.as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            && part.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'+' | b'-')
            })
    };
    value.len() <= MIME_TYPE_BYTES
        && value
            .split_once('/')
            .is_some_and(|(kind, subtype)| token(kind) && token(subtype))
}

#[cfg(test)]
mod tests {
    use super::{FileAssociation, validate};

    fn association(extensions: &[&str], mime_type: &str, description: &str) -> FileAssociation {
        FileAssociation {
            extensions: extensions.iter().map(|value| (*value).to_owned()).collect(),
            mime_type: mime_type.to_owned(),
            description: description.to_owned(),
        }
    }

    #[test]
    fn associations_are_bounded_and_unique() {
        let dicom = association(&["dcm", "dicom"], "application/dicom", "DICOM image");
        let nifti = association(&["nii"], "application/x-nifti", "NIfTI volume");
        assert!(validate(&[]).is_ok());
        assert!(validate(&[dicom.clone(), nifti.clone()]).is_ok());
        for rejected in [
            association(&[], "application/x-a", "A"),
            association(&["DCM"], "application/x-a", "A"),
            association(&[".dcm"], "application/x-a", "A"),
            association(&["a b"], "application/x-a", "A"),
            association(&[&"x".repeat(17)], "application/x-a", "A"),
            association(&["a"], "Application/X", "A"),
            association(&["a"], "application", "A"),
            association(&["a"], "application/x a", "A"),
            association(&["a"], "application/x-a", " "),
            association(&["a"], "application/x-a", "line\nbreak"),
            association(&["a"], "application/x-a", "[ProgramFilesFolder]"),
            association(&["a"], "application/x-a", "#1 format"),
            association(&["a"; 9], "application/x-a", "A"),
        ] {
            assert!(
                validate(std::slice::from_ref(&rejected)).is_err(),
                "{rejected:?}"
            );
        }
        let repeated_extension = association(&["dcm"], "application/x-other", "Other");
        assert!(validate(&[dicom.clone(), repeated_extension]).is_err());
        let repeated_type = association(&["dcx"], "application/dicom", "Other");
        assert!(validate(&[dicom.clone(), repeated_type]).is_err());
        assert!(validate(&vec![nifti; 9]).is_err());
    }
}
