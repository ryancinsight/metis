//! Native file-selection adapter over Moirai's platform provider.

pub use moirai_pal::windows::dialog::{
    DialogSelection, FileFilter, MAX_DIALOG_FILTERS, MAX_DIALOG_PATH_UNITS, SaveRequest,
};

/// Opens the Windows common dialog and returns the selected path.
///
/// [`DialogSelection::Save`] carries a [`SaveRequest`] and returns the
/// destination the user confirmed; the application writes the file.
///
/// The platform layer returns only the user selection. Applications remain
/// responsible for authorization, root confinement, file-type validation and
/// bounded reads.
pub use moirai_pal::windows::dialog::pick;

#[cfg(test)]
mod tests {
    use super::{FileFilter, MAX_DIALOG_FILTERS, SaveRequest};

    #[test]
    fn a_save_request_built_through_metis_carries_its_name_and_filters() -> std::io::Result<()> {
        let png = FileFilter::new("PNG image", "*.png")?;
        let request = SaveRequest::new("axial.png", [png.clone()])?;
        assert_eq!(request.file_name(), "axial.png");
        assert_eq!(request.filters(), [png]);
        Ok(())
    }

    #[test]
    fn a_save_request_at_the_filter_bound_keeps_every_filter() -> std::io::Result<()> {
        let filter = FileFilter::new("PNG image", "*.png")?;
        let request = SaveRequest::new("axial.png", vec![filter; MAX_DIALOG_FILTERS])?;
        assert_eq!(request.filters().len(), MAX_DIALOG_FILTERS);
        Ok(())
    }

    #[test]
    fn a_save_request_past_the_filter_bound_is_rejected() -> std::io::Result<()> {
        let filter = FileFilter::new("PNG image", "*.png")?;
        let error = SaveRequest::new("axial.png", vec![filter; MAX_DIALOG_FILTERS + 1])
            .expect_err("invariant: one filter past the bound is rejected");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        Ok(())
    }
}
