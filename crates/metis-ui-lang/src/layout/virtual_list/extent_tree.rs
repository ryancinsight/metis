//! A Fenwick (binary indexed) tree over item extents.

use crate::parser::limit_error;
use metis_core::error::Result;

/// Prefix sums over item extents with logarithmic update and search.
///
/// `sums[i - 1]` holds the extent sum of items `(i - lowbit(i), i]` in the
/// one-based Fenwick convention; `extents` keeps each item's own extent so an
/// update can apply the difference.
#[derive(Debug, Clone)]
pub(super) struct ExtentTree {
    sums: Vec<u64>,
    extents: Vec<u32>,
}

impl ExtentTree {
    /// Builds the tree in linear time.
    pub(super) fn new(extents: &[u32]) -> Result<Self> {
        let mut own = reserve(extents.len())?;
        own.extend_from_slice(extents);
        let mut sums: Vec<u64> = reserve(extents.len())?;
        sums.extend(extents.iter().map(|extent| u64::from(*extent)));
        for index in 1..=sums.len() {
            let parent = index + lowbit(index);
            if parent <= sums.len() {
                sums[parent - 1] += sums[index - 1];
            }
        }
        Ok(Self { sums, extents: own })
    }

    pub(super) fn len(&self) -> usize {
        self.extents.len()
    }

    /// Replaces the extent of item `index`, which must be in range.
    pub(super) fn set(&mut self, index: usize, extent: u32) {
        let previous = std::mem::replace(&mut self.extents[index], extent);
        let mut position = index + 1;
        while position <= self.sums.len() {
            let sum = &mut self.sums[position - 1];
            *sum = *sum - u64::from(previous) + u64::from(extent);
            position += lowbit(position);
        }
    }

    /// Sum of the extents of items `0..count`.
    pub(super) fn prefix(&self, count: usize) -> u64 {
        let mut position = count.min(self.sums.len());
        let mut total = 0;
        while position > 0 {
            total += self.sums[position - 1];
            position -= lowbit(position);
        }
        total
    }

    /// Index of the first item whose span contains `offset`, skipping
    /// zero-extent items, or `len` when `offset` is at or past the end.
    pub(super) fn index_at(&self, offset: u64) -> usize {
        // Binary lifting: find the largest prefix whose sum is <= offset.
        let mut position = 0;
        let mut remaining = offset;
        let mut step = self.sums.len().checked_next_power_of_two().unwrap_or(0);
        if step > self.sums.len() {
            step /= 2;
        }
        while step > 0 {
            let next = position + step;
            if next <= self.sums.len() && self.sums[next - 1] <= remaining {
                position = next;
                remaining -= self.sums[next - 1];
            }
            step /= 2;
        }
        position
    }
}

fn reserve<T>(len: usize) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(len)
        .map_err(|_| limit_error("Virtual list extent storage allocation failed"))?;
    Ok(values)
}

const fn lowbit(index: usize) -> usize {
    index & index.wrapping_neg()
}
