use super::{
    ExplorerStatus, GroupId, MAX_RESULT_FILTER_BYTES, MAX_RESULT_GROUPS, MAX_RESULT_ROWS,
    RESULT_PAGE_SIZE, RecordOutcome, ResultId, ResultRow, SortDirection, SortKey, SortOrder,
    VisibleEntry,
};
use metis_core::error::{ErrorCode, MetisError, Result};
use std::collections::VecDeque;

mod mutations;
mod visibility;

struct ResultGroup {
    id: GroupId,
    label: Box<str>,
    expanded: bool,
}

fn validate_filter(value: &str) -> Result<()> {
    if value.len() > MAX_RESULT_FILTER_BYTES {
        return Err(MetisError::protocol(
            ErrorCode::PayloadTooLarge,
            "Result filter exceeds the bounded display limit",
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(MetisError::protocol(
            ErrorCode::MalformedPayload,
            "Result filter contains a control character",
        ));
    }
    Ok(())
}
#[derive(Clone, Copy)]
enum VisibleIndex {
    Group(usize),
    Row(usize),
}

/// Owns bounded result rows and host-independent view state.
pub struct ResultExplorer {
    rows: VecDeque<ResultRow>,
    groups: Vec<ResultGroup>,
    visible: Vec<VisibleIndex>,
    selected: Option<ResultId>,
    filter: Box<str>,
    sort: SortOrder,
    window_start: usize,
    next_group_id: u64,
    status: ExplorerStatus,
}

impl Default for ResultExplorer {
    fn default() -> Self {
        Self::new()
    }
}

impl ResultExplorer {
    /// Creates an empty explorer with no allocated row history.
    #[must_use]
    pub fn new() -> Self {
        Self {
            rows: VecDeque::new(),
            groups: Vec::new(),
            visible: Vec::new(),
            selected: None,
            filter: "".into(),
            sort: SortOrder::default(),
            window_start: 0,
            next_group_id: 1,
            status: ExplorerStatus::Empty,
        }
    }

    /// Returns the current lifecycle state.
    #[must_use]
    pub const fn status(&self) -> &ExplorerStatus {
        &self.status
    }

    /// Returns the retained row count.
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Returns the retained patient-group count.
    #[must_use]
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Returns the current filter without copying it.
    #[must_use]
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Returns the current ordering.
    #[must_use]
    pub const fn sort(&self) -> SortOrder {
        self.sort
    }

    /// Returns the selected row identity, if one remains retained.
    #[must_use]
    pub const fn selected_id(&self) -> Option<ResultId> {
        self.selected
    }

    /// Returns the selected row, even when the filter currently hides it.
    #[must_use]
    pub fn selected(&self) -> Option<&ResultRow> {
        self.selected
            .and_then(|id| self.rows.iter().find(|row| row.id() == id))
    }

    /// Returns the number of entries after filtering and tree disclosure.
    #[must_use]
    pub fn visible_count(&self) -> usize {
        self.visible.len()
    }

    /// Returns the current first visible entry offset.
    #[must_use]
    pub const fn window_start(&self) -> usize {
        self.window_start
    }

    /// Reports whether a previous page exists.
    #[must_use]
    pub const fn can_previous(&self) -> bool {
        self.window_start != 0
    }

    /// Reports whether a next page exists.
    #[must_use]
    pub fn can_next(&self) -> bool {
        self.window_start.saturating_add(RESULT_PAGE_SIZE) < self.visible.len()
    }

    fn insert_row(&mut self, row: ResultRow) -> Result<RecordOutcome> {
        let mut outcome = RecordOutcome::Inserted;
        if let Some(index) = self
            .rows
            .iter()
            .position(|current| current.id() == row.id())
        {
            self.rows[index] = row;
            outcome = RecordOutcome::Updated;
        } else {
            if self.rows.len() == MAX_RESULT_ROWS {
                let evicted = self.rows.pop_front().ok_or_else(|| {
                    MetisError::protocol(
                        ErrorCode::MalformedPayload,
                        "Result explorer capacity state is inconsistent",
                    )
                })?;
                outcome = RecordOutcome::Evicted(evicted.id());
            }
            self.rows.push_back(row);
        }
        self.rebuild_groups()?;
        self.retain_selection();
        self.status = ExplorerStatus::Ready;
        self.rebuild_visible()?;
        Ok(outcome)
    }

    fn rebuild_groups(&mut self) -> Result<()> {
        let previous = std::mem::take(&mut self.groups);
        let mut groups = Vec::new();
        groups.try_reserve(self.rows.len()).map_err(|_| {
            MetisError::protocol(
                ErrorCode::PayloadTooLarge,
                "Result explorer group index allocation failed",
            )
        })?;
        for index in 0..self.rows.len() {
            let label = self
                .rows
                .get(index)
                .map(ResultRow::patient_id)
                .ok_or_else(|| {
                    MetisError::protocol(
                        ErrorCode::MalformedPayload,
                        "Result explorer row index is inconsistent",
                    )
                })?
                .to_owned();
            if groups
                .iter()
                .any(|group: &ResultGroup| group.label.as_ref() == label.as_str())
            {
                continue;
            }
            let (id, expanded) = if let Some(group) = previous
                .iter()
                .find(|group| group.label.as_ref() == label.as_str())
            {
                (group.id, group.expanded)
            } else {
                (self.allocate_group_id()?, true)
            };
            groups.push(ResultGroup {
                id,
                label: label.into(),
                expanded,
            });
            if groups.len() == MAX_RESULT_GROUPS {
                break;
            }
        }
        self.groups = groups;
        Ok(())
    }

    fn allocate_group_id(&mut self) -> Result<GroupId> {
        let id = GroupId::try_from(self.next_group_id).map_err(|_| {
            MetisError::protocol(
                ErrorCode::MalformedPayload,
                "Result explorer group identity exhausted",
            )
        })?;
        self.next_group_id = self.next_group_id.checked_add(1).ok_or_else(|| {
            MetisError::protocol(
                ErrorCode::MalformedPayload,
                "Result explorer group identity exhausted",
            )
        })?;
        Ok(id)
    }

    fn retain_selection(&mut self) {
        if self
            .selected
            .is_some_and(|id| !self.rows.iter().any(|row| row.id() == id))
        {
            self.selected = None;
        }
    }
}
