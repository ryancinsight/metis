use super::{
    ErrorCode, ExplorerStatus, GroupId, MAX_RESULT_ROWS, MetisError, RESULT_PAGE_SIZE,
    RecordOutcome, Result, ResultExplorer, ResultId, ResultRow, SortOrder, VecDeque, VisibleEntry,
    VisibleIndex, validate_filter,
};
use metis_core::protocol::ClinicalCalcResponsePayload;

enum VisibleAction {
    ToggleGroup(GroupId),
    SelectRow(ResultId),
}

impl ResultExplorer {
    /// Replaces the filter and rebuilds the bounded visible index.
    ///
    /// # Errors
    /// Rejects control characters or a filter above
    /// [`crate::MAX_RESULT_FILTER_BYTES`].
    pub fn set_filter(&mut self, value: &str) -> Result<()> {
        validate_filter(value)?;
        self.filter = value.into();
        self.rebuild_visible()
    }

    /// Replaces the ordering and rebuilds the bounded visible index.
    ///
    /// # Errors
    /// Returns a bounded-allocation error if the visible index cannot be
    /// rebuilt.
    pub fn set_sort(&mut self, sort: SortOrder) -> Result<()> {
        self.sort = sort;
        self.rebuild_visible()
    }

    /// Marks an asynchronous producer operation as in progress.
    pub fn begin_loading(&mut self) {
        self.status = ExplorerStatus::Loading;
    }

    /// Records a producer failure while retaining already valid rows.
    pub fn fail(&mut self, error: MetisError) {
        self.status = ExplorerStatus::Error(error);
    }

    /// Removes every retained row, group, selection, and visible index.
    pub fn clear(&mut self) {
        self.rows.clear();
        self.groups.clear();
        self.visible.clear();
        self.selected = None;
        self.window_start = 0;
        self.status = ExplorerStatus::Empty;
    }

    /// Records one real backend response in the bounded history.
    ///
    /// Duplicate audit identities update their existing row. New rows evict
    /// the oldest retained row once [`MAX_RESULT_ROWS`] is reached.
    ///
    /// # Errors
    /// Propagates row validation and visible-index allocation failures.
    pub fn record_response(
        &mut self,
        patient_id: &str,
        response: &ClinicalCalcResponsePayload,
    ) -> Result<RecordOutcome> {
        self.insert_row(ResultRow::try_new(patient_id, response)?)
    }

    /// Replaces all rows from a bounded iterator of validated rows.
    ///
    /// # Errors
    /// Rejects more than [`MAX_RESULT_ROWS`] rows or duplicate identities, and
    /// reports visible-index allocation failures.
    pub fn replace_rows<I>(&mut self, rows: I) -> Result<()>
    where
        I: IntoIterator<Item = ResultRow>,
    {
        let mut replacement: VecDeque<ResultRow> = VecDeque::new();
        for row in rows {
            if replacement.len() == MAX_RESULT_ROWS {
                return Err(MetisError::protocol(
                    ErrorCode::PayloadTooLarge,
                    "Result explorer row capacity is bounded",
                ));
            }
            if replacement.iter().any(|current| current.id() == row.id()) {
                return Err(MetisError::protocol(
                    ErrorCode::MalformedPayload,
                    "Result explorer rows require unique audit identities",
                ));
            }
            replacement.push_back(row);
        }
        self.rows = replacement;
        self.rebuild_groups()?;
        self.retain_selection();
        self.status = if self.rows.is_empty() {
            ExplorerStatus::Empty
        } else {
            ExplorerStatus::Ready
        };
        self.rebuild_visible()
    }

    /// Selects one retained row by stable identity.
    #[must_use]
    pub fn select(&mut self, id: ResultId) -> bool {
        if self.rows.iter().any(|row| row.id() == id) {
            self.selected = Some(id);
            true
        } else {
            false
        }
    }

    /// Selects a row in the current page by its zero-based slot.
    #[must_use]
    pub fn select_visible_row(&mut self, slot: usize) -> bool {
        let Some(VisibleIndex::Row(index)) = self.visible.get(self.window_start + slot) else {
            return false;
        };
        let Some(row) = self.rows.get(*index) else {
            return false;
        };
        self.selected = Some(row.id());
        true
    }

    /// Expands or collapses a patient group.
    ///
    /// # Errors
    /// Returns a bounded-index allocation error when disclosure changes the
    /// visible tree.
    pub fn toggle_group(&mut self, id: GroupId) -> Result<bool> {
        let Some(group) = self.groups.iter_mut().find(|group| group.id == id) else {
            return Ok(false);
        };
        group.expanded = !group.expanded;
        let expanded = group.expanded;
        self.rebuild_visible()?;
        Ok(expanded)
    }

    /// Activates a visible tree entry through its keyboard-accessible button.
    ///
    /// Group entries toggle disclosure and row entries update selection. The
    /// method returns `false` when the page slot is empty.
    #[must_use]
    pub fn activate_visible(&mut self, slot: usize) -> bool {
        let action = match self.visible_entry(slot) {
            Some(VisibleEntry::Group { id, .. }) => Some(VisibleAction::ToggleGroup(id)),
            Some(VisibleEntry::Row(row)) => Some(VisibleAction::SelectRow(row.id())),
            None => None,
        };
        match action {
            Some(VisibleAction::ToggleGroup(id)) => self.toggle_group(id).is_ok(),
            Some(VisibleAction::SelectRow(id)) => self.select(id),
            None => false,
        }
    }

    /// Advances the visible window by one page.
    pub fn next_page(&mut self) {
        if self.can_next() {
            self.window_start = self
                .window_start
                .saturating_add(RESULT_PAGE_SIZE)
                .min(self.visible.len().saturating_sub(RESULT_PAGE_SIZE));
        }
    }

    /// Moves the visible window back by one page.
    pub fn previous_page(&mut self) {
        self.window_start = self.window_start.saturating_sub(RESULT_PAGE_SIZE);
    }
}
