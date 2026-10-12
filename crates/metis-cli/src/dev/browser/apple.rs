//! macOS native development window owns one supervised host process.
//! A successful rebuild replaces the window; a failed build keeps the prior
//! running application. Host exit terminates the developer session.

use super::super::{DevMode, snapshot, watcher};
use crate::{
    Result,
    apple::{self, Target},
    build,
    manifest::WebApplication,
    process::{self, Containment},
};
use std::{path::Path, time::Duration};

const OBSERVATION: Duration = Duration::from_millis(100);

pub(super) fn run(application: &WebApplication, root: &Path, mode: DevMode) -> Result<()> {
    let served = build::web::application(application, root, None)?;
    let executable = apple::executable(Target::Desktop)?;
    let output = root.join("output/apple/dev-macos");
    let bundle = apple::package(
        application,
        root,
        &served,
        &executable,
        &output,
        Target::Desktop,
    )?;
    let program = apple::program(&bundle, Target::Desktop);
    let watcher = matches!(mode, DevMode::Watch)
        .then(|| watcher::Watcher::new(root))
        .transpose()?;
    let mut baseline = snapshot(root)?;
    let mut child = process::spawn(&program, &[], Containment::Required)?;
    eprintln!("metis dev: {} in a macOS WebKit window", application.name);
    loop {
        if let Some(status) = child.wait_timeout(OBSERVATION)? {
            if status.exit_status().success() {
                return Ok(());
            }
            return Err(format!("Apple application exited: {}", status.exit_status()).into());
        }
        let Some(watcher) = &watcher else { continue };
        if !watcher.changed()? {
            continue;
        }
        let current = snapshot(root)?;
        if current == baseline {
            continue;
        }
        baseline = current;
        let rebuild = build::web::application(application, root, None).and_then(|served| {
            apple::package(
                application,
                root,
                &served,
                &executable,
                &output,
                Target::Desktop,
            )
        });
        match rebuild {
            Ok(bundle) => {
                child.terminate_timeout(Duration::from_secs(1))?;
                child = process::spawn(
                    &apple::program(&bundle, Target::Desktop),
                    &[],
                    Containment::Required,
                )?;
                eprintln!("metis dev: rebuilt; reopened the window");
            }
            Err(error) => eprintln!("metis dev: rebuild failed; keeping the page: {error}"),
        }
    }
}
