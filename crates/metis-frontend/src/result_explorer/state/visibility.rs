use super::{
    ErrorCode, MetisError, RESULT_PAGE_SIZE, Result, ResultExplorer, ResultRow, SortDirection,
    SortKey, VisibleEntry, VisibleIndex,
};

impl ResultExplorer {
    /// Returns one visible tree entry in the current page.
    #[must_use]
    pub fn visible_entry(&self, slot: usize) -> Option<VisibleEntry<'_>> {
        let index = *self.visible.get(self.window_start + slot)?;
        match index {
            VisibleIndex::Group(index) => {
                let group = self.groups.get(index)?;
                let row_count = self
                    .rows
                    .iter()
                    .filter(|row| row.patient_id() == group.label.as_ref())
                    .count();
                Some(VisibleEntry::Group {
                    id: group.id,
                    label: &group.label,
                    expanded: group.expanded,
                    row_count,
                })
            }
            VisibleIndex::Row(index) => self.rows.get(index).map(VisibleEntry::Row),
        }
    }

    /// Returns visible entries in the current page without allocating.
    pub fn visible_entries(&self) -> impl Iterator<Item = VisibleEntry<'_>> {
        (0..RESULT_PAGE_SIZE).filter_map(|slot| self.visible_entry(slot))
    }

    /// Returns visible rows in the current page without allocating.
    pub fn visible_rows(&self) -> impl Iterator<Item = &ResultRow> {
        self.visible_entries().filter_map(|entry| match entry {
            VisibleEntry::Row(row) => Some(row),
            VisibleEntry::Group { .. } => None,
        })
    }

    pub(crate) fn rebuild_visible(&mut self) -> Result<()> {
        let mut row_indices = Vec::new();
        row_indices.try_reserve(self.rows.len()).map_err(|_| {
            MetisError::protocol(
                ErrorCode::PayloadTooLarge,
                "Result explorer row index allocation failed",
            )
        })?;
        for (index, row) in self.rows.iter().enumerate() {
            if matches_filter(row, &self.filter) {
                row_indices.push(index);
            }
        }
        row_indices.sort_by(|left, right| self.compare_rows(*left, *right));
        let mut visible = Vec::new();
        visible
            .try_reserve(row_indices.len().saturating_add(self.groups.len()))
            .map_err(|_| {
                MetisError::protocol(
                    ErrorCode::PayloadTooLarge,
                    "Result explorer visible index allocation failed",
                )
            })?;
        let mut group_indices = Vec::new();
        group_indices.try_reserve(self.groups.len()).map_err(|_| {
            MetisError::protocol(
                ErrorCode::PayloadTooLarge,
                "Result explorer group index allocation failed",
            )
        })?;
        for (group_index, group) in self.groups.iter().enumerate() {
            let group_rows = row_indices.iter().copied().filter(|index| {
                self.rows
                    .get(*index)
                    .is_some_and(|row| row.patient_id() == group.label.as_ref())
            });
            if group_rows.clone().next().is_some() {
                group_indices.push(group_index);
            }
        }
        group_indices.sort_by(|left, right| self.compare_groups(*left, *right, &row_indices));
        for group_index in group_indices {
            let Some(group) = self.groups.get(group_index) else {
                continue;
            };
            let group_rows = row_indices.iter().copied().filter(|index| {
                self.rows
                    .get(*index)
                    .is_some_and(|row| row.patient_id() == group.label.as_ref())
            });
            let group_rows = group_rows.peekable();
            visible.push(VisibleIndex::Group(group_index));
            if group.expanded {
                visible.extend(group_rows.map(VisibleIndex::Row));
            }
        }
        self.visible = visible;
        self.window_start = self
            .window_start
            .min(self.visible.len().saturating_sub(RESULT_PAGE_SIZE));
        Ok(())
    }

    fn compare_groups(&self, left: usize, right: usize, rows: &[usize]) -> std::cmp::Ordering {
        let Some(left_group) = self.groups.get(left) else {
            return std::cmp::Ordering::Equal;
        };
        let Some(right_group) = self.groups.get(right) else {
            return std::cmp::Ordering::Equal;
        };
        if self.sort.key() == SortKey::Patient {
            return match self.sort.direction() {
                SortDirection::Ascending => left_group.label.cmp(&right_group.label),
                SortDirection::Descending => right_group.label.cmp(&left_group.label),
            };
        }
        let left_row = rows.iter().copied().find(|index| {
            self.rows
                .get(*index)
                .is_some_and(|row| row.patient_id() == left_group.label.as_ref())
        });
        let right_row = rows.iter().copied().find(|index| {
            self.rows
                .get(*index)
                .is_some_and(|row| row.patient_id() == right_group.label.as_ref())
        });
        match (left_row, right_row) {
            (Some(left), Some(right)) => self.compare_rows(left, right),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left_group.id.cmp(&right_group.id),
        }
    }

    fn compare_rows(&self, left: usize, right: usize) -> std::cmp::Ordering {
        let Some(left) = self.rows.get(left) else {
            return std::cmp::Ordering::Equal;
        };
        let Some(right) = self.rows.get(right) else {
            return std::cmp::Ordering::Equal;
        };
        let ordering = match self.sort.key() {
            SortKey::Sequence => left.id().cmp(&right.id()),
            SortKey::Patient => left.patient_id().cmp(right.patient_id()),
            SortKey::VolumeRate => left.rate_ml_hr().total_cmp(&right.rate_ml_hr()),
            SortKey::DrugRate => left.drug_rate_mg_hr().total_cmp(&right.drug_rate_mg_hr()),
        };
        let ordering = match self.sort.direction() {
            SortDirection::Ascending => ordering,
            SortDirection::Descending => ordering.reverse(),
        };
        ordering.then_with(|| left.id().cmp(&right.id()))
    }
}

fn matches_filter(row: &ResultRow, filter: &str) -> bool {
    filter.is_empty() || row.patient_id().contains(filter)
}
