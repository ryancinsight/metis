use super::{
    ExplorerStatus, GroupId, MAX_RESULT_FILTER_BYTES, MAX_RESULT_GROUPS, MAX_RESULT_ROWS,
    RESULT_PAGE_SIZE, RecordOutcome, ResultId, ResultRow, SortDirection, SortKey, SortOrder,
    VisibleEntry,
};
use metis_core::error::{ErrorCode, MetisError, Result};
use std::collections::{HashMap, VecDeque};

mod mutations;
mod visibility;

struct ResultGroup {
    id: GroupId,
    label: Box<str>,
    expanded: bool,
    /// Retained rows for this patient, independent of the filter.
    row_count: usize,
}

/// An empty vector with room for `capacity` values, or a bounded
/// allocation error naming `what`.
fn reserved<T>(capacity: usize, what: &'static str) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve(capacity)
        .map_err(|_| MetisError::protocol(ErrorCode::PayloadTooLarge, what))?;
    Ok(values)
}

/// Maps each distinct label to its position in `labels`, in one pass.
fn label_index<'a>(
    labels: impl ExactSizeIterator<Item = &'a str>,
) -> Result<HashMap<&'a str, usize>> {
    let mut index = HashMap::new();
    index.try_reserve(labels.len()).map_err(|_| {
        MetisError::protocol(
            ErrorCode::PayloadTooLarge,
            "Result explorer label index allocation failed",
        )
    })?;
    for (position, label) in labels.enumerate() {
        index.entry(label).or_insert(position);
    }
    Ok(index)
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

    /// Regroups rows by patient in first-seen order, keeping each surviving
    /// group's identity and disclosure. One hash lookup per row replaces a
    /// scan of every group, and a label is copied only for a new group.
    fn rebuild_groups(&mut self) -> Result<()> {
        let previous = std::mem::take(&mut self.groups);
        let previous_index = label_index(previous.iter().map(|group| group.label.as_ref()))?;
        let mut groups: Vec<ResultGroup> = reserved(
            self.rows.len(),
            "Result explorer group index allocation failed",
        )?;
        let mut current: HashMap<&str, usize> = HashMap::new();
        current.try_reserve(self.rows.len()).map_err(|_| {
            MetisError::protocol(
                ErrorCode::PayloadTooLarge,
                "Result explorer label index allocation failed",
            )
        })?;
        let mut next_group_id = self.next_group_id;
        for row in &self.rows {
            let label = row.patient_id();
            if let Some(position) = current.get(label) {
                if let Some(group) = groups.get_mut(*position) {
                    group.row_count += 1;
                }
                continue;
            }
            if groups.len() == MAX_RESULT_GROUPS {
                continue;
            }
            let (id, expanded) = match previous_index
                .get(label)
                .and_then(|position| previous.get(*position))
            {
                Some(group) => (group.id, group.expanded),
                None => (allocate_group_id(&mut next_group_id)?, true),
            };
            current.insert(label, groups.len());
            groups.push(ResultGroup {
                id,
                label: label.into(),
                expanded,
                row_count: 1,
            });
        }
        self.next_group_id = next_group_id;
        self.groups = groups;
        Ok(())
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

/// Issues the next group identity from `next`, failing once exhausted.
fn allocate_group_id(next: &mut u64) -> Result<GroupId> {
    let exhausted = || {
        MetisError::protocol(
            ErrorCode::MalformedPayload,
            "Result explorer group identity exhausted",
        )
    };
    let id = GroupId::try_from(*next).map_err(|_| exhausted())?;
    *next = next.checked_add(1).ok_or_else(exhausted)?;
    Ok(id)
}
