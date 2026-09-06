//! Launches one application executable with separate process roles.
use std::process::Command;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let executable = std::env::current_exe()?;
    let target = executable
        .parent()
        .and_then(std::path::Path::parent)
        .ok_or("Example executable has no target directory")?;
    let backend = target.join(format!("metis-app{}", std::env::consts::EXE_SUFFIX));
    let status = Command::new(backend).args(["60", "2", "0.2"]).status()?;
    if !status.success() {
        return Err(format!("Backend session failed: {status}").into());
    }
    Ok(())
}
