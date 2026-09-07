//! Monotonic browser lifecycle generations for async completion guards.

use std::io;
use std::num::NonZeroU64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Generation(NonZeroU64);

pub(crate) struct Epoch(NonZeroU64);

impl Epoch {
    pub(crate) const fn new() -> Self {
        Self(NonZeroU64::MIN)
    }

    #[cfg(test)]
    pub(crate) const fn current(&self) -> Generation {
        Generation(self.0)
    }

    pub(crate) fn advance(&mut self) -> io::Result<Generation> {
        let value = self
            .0
            .get()
            .checked_add(1)
            .ok_or_else(|| io::Error::other("browser lifecycle generation exhausted"))?;
        let generation = NonZeroU64::new(value)
            .ok_or_else(|| io::Error::other("browser lifecycle generation became zero"))?;
        self.0 = generation;
        Ok(Generation(generation))
    }

    pub(crate) fn accepts(&self, generation: Generation) -> bool {
        self.0 == generation.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_from_an_earlier_mount_is_rejected() {
        let mut epoch = Epoch::new();
        let first = epoch.current();
        let second = epoch.advance().expect("generation");
        assert!(!epoch.accepts(first));
        assert!(epoch.accepts(second));
    }

    #[test]
    fn generation_exhaustion_is_reported_without_wraparound() {
        let mut epoch = Epoch(NonZeroU64::new(u64::MAX).expect("nonzero"));
        assert!(epoch.advance().is_err());
        assert_eq!(
            epoch.current(),
            Generation(NonZeroU64::new(u64::MAX).expect("nonzero"))
        );
    }
}
