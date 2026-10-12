use crate::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Desktop,
    Device,
    Simulator,
}

impl Target {
    pub(super) fn parse(value: &str) -> Result<Self> {
        match value {
            "macos" => Ok(Self::Desktop),
            "ios" => Ok(Self::Device),
            "ios-simulator" => Ok(Self::Simulator),
            _ => Err(format!("unknown Apple target {value}").into()),
        }
    }
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Desktop => "macos",
            Self::Device => "ios",
            Self::Simulator => "ios-simulator",
        }
    }
    pub(super) const fn sdk(self) -> &'static str {
        match self {
            Self::Desktop => "macosx",
            Self::Device => "iphoneos",
            Self::Simulator => "iphonesimulator",
        }
    }
    pub(super) const fn deployment(self) -> (&'static str, &'static str) {
        // The shared frontend permits WebAssembly through wasm-unsafe-eval,
        // which requires WebKit 16. These OS releases include that engine;
        // compilation and bundle metadata use this same deployment contract.
        match self {
            Self::Desktop => ("MACOSX_DEPLOYMENT_TARGET", "13.0"),
            Self::Device | Self::Simulator => ("IPHONEOS_DEPLOYMENT_TARGET", "16.0"),
        }
    }
    pub(super) fn triple(self) -> Result<&'static str> {
        match (self, std::env::consts::ARCH) {
            (Self::Desktop, "aarch64") => Ok("aarch64-apple-darwin"),
            (Self::Desktop, "x86_64") => Ok("x86_64-apple-darwin"),
            (Self::Simulator, "aarch64") => Ok("aarch64-apple-ios-sim"),
            (Self::Simulator, "x86_64") => Ok("x86_64-apple-ios"),
            (Self::Device, _) => Ok("aarch64-apple-ios"),
            _ => Err("Apple host architecture must be aarch64 or x86_64".into()),
        }
    }
}
