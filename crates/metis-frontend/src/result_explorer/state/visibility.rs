use super::{
    RESULT_PAGE_SIZE, Result, ResultExplorer, ResultRow, SortDirection, SortKey, VisibleEntry,
    VisibleIndex, label_index, reserved,
};

impl ResultExplorer {
    /// Returns one visible tree entry in the current page.
    #[must_use]
    pub fn visible_entry(&self, slot: usize) -> Option<VisibleEntry<'_>> {
        let index = *self.visible.get(self.window_start + slot)?;
        match index {
            VisibleIndex::Group(index) => {
                let group = self.groups.get(index)?;
                Some(VisibleEntry::Group {
                    id: group.id,
                    label: &group.label,
                    expanded: group.expanded,
                    row_count: group.row_count,
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

    /// Rebuilds the flattened tree: filtered rows in sort order, bucketed
    /// under their patient group, groups ordered by the sort key.
    ///
    /// Rows are assigned to groups through one label lookup each and then
    /// placed with a stable counting sort, so the rebuild costs one sort of
    /// the filtered rows plus linear passes, not a scan of every row per group.
    pub(crate) fn rebuild_visible(&mut self) -> Result<()> {
        const ROWS: &str = "Result explorer row index allocation failed";
        let mut row_indices: Vec<usize> = reserved(self.rows.len(), ROWS)?;
        row_indices.extend(
            self.rows
                .iter()
                .enumerate()
                .filter(|(_, row)| matches_filter(row, &self.filter))
                .map(|(index, _)| index),
        );
        row_indices.sort_by(|left, right| self.compare_rows(*left, *right));

        let groups = label_index(self.groups.iter().map(|group| group.label.as_ref()))?;
        let mut row_group: Vec<usize> = reserved(row_indices.len(), ROWS)?;
        let mut bucket_start: Vec<usize> = reserved(self.groups.len() + 1, ROWS)?;
        bucket_start.resize(self.groups.len() + 1, 0);
        for index in &row_indices {
            // Every retained row has a group; a missing one is skipped as
            // the pre-bucketing implementation skipped it.
            let group = self
                .rows
                .get(*index)
                .and_then(|row| groups.get(row.patient_id()).copied())
                .unwrap_or(self.groups.len());
            row_group.push(group);
            if let Some(count) = bucket_start.get_mut(group + 1) {
                *count += 1;
            }
        }
        for position in 1..bucket_start.len() {
            bucket_start[position] += bucket_start[position - 1];
        }
        let mut next_slot = bucket_start.clone();
        let mut bucketed: Vec<usize> = reserved(row_indices.len(), ROWS)?;
        bucketed.resize(row_indices.len(), usize::MAX);
        for (index, group) in row_indices.iter().zip(&row_group) {
            if let Some(slot) = next_slot.get_mut(*group) {
                bucketed[*slot] = *index;
                *slot += 1;
            }
        }

        let bucket = |group: usize| &bucketed[bucket_start[group]..bucket_start[group + 1]];
        let mut group_order: Vec<usize> = reserved(
            self.groups.len(),
            "Result explorer group index allocation failed",
        )?;
        group_order.extend((0..self.groups.len()).filter(|group| !bucket(*group).is_empty()));
        group_order.sort_by(|left, right| {
            self.compare_groups(*left, *right, bucket(*left)[0], bucket(*right)[0])
        });

        let mut visible = reserved(
            row_indices.len().saturating_add(group_order.len()),
            "Result explorer visible index allocation failed",
        )?;
        for group in group_order {
            visible.push(VisibleIndex::Group(group));
            if self.groups[group].expanded {
                visible.extend(bucket(group).iter().copied().map(VisibleIndex::Row));
            }
        }
        self.visible = visible;
        self.window_start = self
            .window_start
            .min(self.visible.len().saturating_sub(RESULT_PAGE_SIZE));
        Ok(())
    }

    /// Orders two non-empty groups: by label under the patient key, otherwise
    /// by each group's first row in the current row order.
    fn compare_groups(
        &self,
        left: usize,
        right: usize,
        left_first: usize,
        right_first: usize,
    ) -> std::cmp::Ordering {
        if self.sort.key() == SortKey::Patient {
            let (left, right) = (&self.groups[left].label, &self.groups[right].label);
            return match self.sort.direction() {
                SortDirection::Ascending => left.cmp(right),
                SortDirection::Descending => right.cmp(left),
            };
        }
        self.compare_rows(left_first, right_first)
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
