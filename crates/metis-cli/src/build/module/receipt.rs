//! Typed compiler output ownership for browser and native preparation.
//!
//! Browser compilation consumes Cargo's ordinary artifacts and preserves its
//! fingerprint cache. Native preparation additionally requires the selected
//! root unit's actual raw dep-info: Cargo's exported lists omit env-dep keys.
//! The destination belongs to the existing fresh Context for its whole lifetime.
//! Successful compilation does not establish admission; the input owner still
//! requires this actual bounded, unlinked regular file and exact root event.
//!
//! Only the native receipt may implement compiled input collection. This keeps
//! an ordinary browser result from being mistaken for complete input evidence.
//! The explicit emission preserves Cargo's link emission but prevents root
//! fingerprint reuse. That measured cost is deliberate; no cached or check-phase
//! environment values substitute for code generation's actual dependencies.
use super::Context;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

mod sealed {
    pub trait Sealed {}
}

pub(in crate::build) trait Receipt: sealed::Sealed + Sized {
    const EXPLICIT: bool;
    fn new(context: &Context) -> Self;
    fn dep_info(&self) -> Option<&Path>;

    fn command() -> &'static str {
        if Self::EXPLICIT { "rustc" } else { "build" }
    }

    fn arguments(&self) -> Vec<OsString> {
        self.dep_info()
            .into_iter()
            .flat_map(|path| {
                let mut emission = OsString::from("--emit=dep-info=");
                emission.push(path.as_os_str());
                [OsString::from("--"), emission]
            })
            .collect()
    }

    fn semantic_arguments() -> Vec<String> {
        if Self::EXPLICIT {
            ["--", "--emit=dep-info"].map(String::from).to_vec()
        } else {
            Vec::new()
        }
    }
}

/// No inventory input evidence is requested by ordinary browser compilation.
#[derive(Debug)]
pub(in crate::build) struct BrowserInputs;
impl sealed::Sealed for BrowserInputs {}
impl Receipt for BrowserInputs {
    const EXPLICIT: bool = false;
    fn new(_context: &Context) -> Self {
        Self
    }
    fn dep_info(&self) -> Option<&Path> {
        None
    }
}

/// Actual root dep-info stays owned by the compilation's fresh Context.
#[derive(Debug)]
pub(in crate::build) struct RootInputs {
    path: PathBuf,
}
impl sealed::Sealed for RootInputs {}
impl Receipt for RootInputs {
    const EXPLICIT: bool = true;
    fn new(context: &Context) -> Self {
        Self {
            path: context.directory.join("root-inputs.d"),
        }
    }
    fn dep_info(&self) -> Option<&Path> {
        Some(&self.path)
    }
}
