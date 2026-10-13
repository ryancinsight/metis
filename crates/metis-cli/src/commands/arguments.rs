//! Optional manifest/output arity belongs to one command argument parser.
use crate::{Result, manifest::DEFAULT_MANIFEST, usage_error};
use std::{ffi::OsString, path::Path};

pub(crate) fn manifest_output<'args>(
    args: &'args [OsString],
    command: &str,
) -> Result<(&'args Path, Option<&'args Path>)> {
    match args {
        [] => Ok((Path::new(DEFAULT_MANIFEST), None)),
        [manifest] => Ok((Path::new(manifest), None)),
        [manifest, output] => Ok((Path::new(manifest), Some(Path::new(output)))),
        _ => Err(usage_error(&format!(
            "{command} takes at most MANIFEST and OUTPUT"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifest_output_preserves_exact_arity_and_path_values() {
        crate::test_support::traced(|| {
            let cases = {
                let _phase = tracing::info_span!("arrange").entered();
                [
                    vec![],
                    vec![OsString::from("workspace/metis.json")],
                    vec![
                        OsString::from("workspace/metis.json"),
                        OsString::from("prepared/output"),
                    ],
                ]
            };
            let values = {
                let _phase = tracing::info_span!("act").entered();
                cases
                    .iter()
                    .map(|args| manifest_output(args, "prepare").expect("arity zero through two"))
                    .collect::<Vec<_>>()
            };
            {
                let _phase = tracing::info_span!("assert").entered();
                assert_eq!(
                    values,
                    [
                        (Path::new(DEFAULT_MANIFEST), None),
                        (Path::new("workspace/metis.json"), None),
                        (
                            Path::new("workspace/metis.json"),
                            Some(Path::new("prepared/output"))
                        )
                    ]
                );
                let too_many = ["manifest", "output", "extra"].map(OsString::from);
                for command in ["prepare", "build"] {
                    assert_eq!(
                        manifest_output(&too_many, command)
                            .expect_err("arity three rejected")
                            .to_string(),
                        format!(
                            "{command} takes at most MANIFEST and OUTPUT\n\n{}",
                            crate::commands::help()
                        )
                    );
                }
            }
        });
    }
}
