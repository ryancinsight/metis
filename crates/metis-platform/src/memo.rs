//! A bounded memo of values a repaint recomputes from the same inputs.
//!
//! Drawing repeats most of its expensive, deterministic work at exactly the
//! inputs it used before: the same glyph at the same pen, the same shadow of
//! the same box. A memo keyed on everything that determines a value returns
//! the value bit for bit instead of recomputing it.
//!
//! Memory is bounded by two generations of at most `BYTES` each. When the
//! current generation fills it becomes the previous one and the older
//! generation is dropped; a hit in the previous generation moves the entry
//! forward. Values used every frame therefore stay resident while values no
//! longer used age out, without per-entry recency bookkeeping.

use std::collections::HashMap;
use std::hash::Hash;

/// The bytes a memoized value keeps alive, counted against its generation.
pub(crate) trait Footprint {
    /// Heap and inline bytes the value retains.
    fn footprint(&self) -> usize;
}

/// Two generations of memoized values, each holding at most `BYTES`.
#[derive(Debug)]
pub(crate) struct GenerationalMemo<K, V, const BYTES: usize> {
    current: HashMap<K, V>,
    previous: HashMap<K, V>,
    current_bytes: usize,
}

impl<K, V, const BYTES: usize> Default for GenerationalMemo<K, V, BYTES> {
    fn default() -> Self {
        Self {
            current: HashMap::new(),
            previous: HashMap::new(),
            current_bytes: 0,
        }
    }
}

impl<K: Eq + Hash, V: Footprint, const BYTES: usize> GenerationalMemo<K, V, BYTES> {
    /// The value for `key`, computing it with `render` on a miss.
    ///
    /// `render` returns `None` when the value is not needed, such as when it
    /// lies wholly outside the clip; nothing is retained then. A value larger
    /// than one generation is rendered but not retained.
    pub(crate) fn get_or_render(
        &mut self,
        key: K,
        render: impl FnOnce() -> Option<V>,
    ) -> Option<Lookup<'_, V>> {
        if self.current.contains_key(&key) {
            return Some(Lookup::Cached(&self.current[&key]));
        }
        let value = match self.previous.remove(&key) {
            Some(value) => value,
            None => render()?,
        };
        let bytes = value.footprint();
        if bytes > BYTES {
            return Some(Lookup::Uncached(value));
        }
        if self.current_bytes + bytes > BYTES {
            self.previous = std::mem::take(&mut self.current);
            self.current_bytes = 0;
        }
        self.current_bytes += bytes;
        Some(Lookup::Cached(self.current.entry(key).or_insert(value)))
    }

    /// Bytes retained across both generations.
    #[cfg(test)]
    pub(crate) fn retained_bytes(&self) -> usize {
        self.current_bytes
            + self
                .previous
                .values()
                .map(Footprint::footprint)
                .sum::<usize>()
    }
}

/// A memo lookup result, borrowed when retained.
pub(crate) enum Lookup<'memo, V> {
    Cached(&'memo V),
    Uncached(V),
}

impl<V> Lookup<'_, V> {
    /// The looked-up value, wherever it lives.
    pub(crate) fn value(&self) -> &V {
        match self {
            Self::Cached(value) => value,
            Self::Uncached(value) => value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Footprint, GenerationalMemo};

    const BYTES: usize = 1024;

    struct Block(usize);

    impl Footprint for Block {
        fn footprint(&self) -> usize {
            self.0
        }
    }

    #[test]
    fn hits_skip_rendering() {
        let mut memo = GenerationalMemo::<u32, Block, BYTES>::default();
        let mut renders = 0;
        for _ in 0..3 {
            memo.get_or_render(7, || {
                renders += 1;
                Some(Block(16))
            });
        }
        assert_eq!(renders, 1);
        memo.get_or_render(8, || {
            renders += 1;
            Some(Block(16))
        });
        assert_eq!(renders, 2);
    }

    #[test]
    fn generations_bound_memory_and_keep_recent_values() {
        let mut memo = GenerationalMemo::<u32, Block, BYTES>::default();
        for key in 0..1_000 {
            memo.get_or_render(key, || Some(Block(BYTES / 64)));
            assert!(memo.retained_bytes() <= 2 * BYTES);
        }
        let mut rendered = false;
        memo.get_or_render(999, || {
            rendered = true;
            Some(Block(BYTES / 64))
        });
        assert!(!rendered, "the most recent value stays resident");
        // A value from two generations ago was dropped and renders again.
        memo.get_or_render(0, || {
            rendered = true;
            Some(Block(BYTES / 64))
        });
        assert!(rendered);
    }

    #[test]
    fn previous_generation_hits_move_forward() {
        let mut memo = GenerationalMemo::<u32, Block, BYTES>::default();
        memo.get_or_render(0, || Some(Block(BYTES / 2)));
        // Filling the current generation demotes key 0 to the previous one.
        memo.get_or_render(1, || Some(Block(BYTES / 2)));
        memo.get_or_render(2, || Some(Block(BYTES / 2)));
        let mut rendered = false;
        memo.get_or_render(0, || {
            rendered = true;
            Some(Block(BYTES / 2))
        });
        assert!(
            !rendered,
            "a previous-generation hit is served, not rendered"
        );
    }

    #[test]
    fn oversized_and_declined_values_are_not_retained() {
        let mut memo = GenerationalMemo::<u32, Block, BYTES>::default();
        let lookup = memo
            .get_or_render(0, || Some(Block(BYTES + 1)))
            .expect("rendered");
        assert_eq!(lookup.value().0, BYTES + 1);
        assert_eq!(memo.retained_bytes(), 0);
        assert!(memo.get_or_render(1, || None).is_none());
        assert_eq!(memo.retained_bytes(), 0);
    }
}
