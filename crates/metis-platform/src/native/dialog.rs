//! Native file-selection adapter over Moirai's platform provider.

pub use moirai_pal::windows::dialog::{DialogSelection, MAX_DIALOG_PATH_UNITS};

/// Opens the Windows common dialog and returns the selected path.
///
/// The platform layer returns only the user selection. Applications remain
/// responsible for authorization, root confinement, file-type validation and
/// bounded reads.
pub use moirai_pal::windows::dialog::pick;
