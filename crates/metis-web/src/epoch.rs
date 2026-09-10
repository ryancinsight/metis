//! Monotonic browser lifecycle generations for async completion guards.

use std::io;
use std::num::NonZeroU64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Generation(NonZeroU64);

impl Generation {
    #[cfg(target_arch = "wasm32")]
    pub(crate) const fn value(self) -> u64 {
        self.0.get()
    }
}

pub(crate) struct Epoch {
    generation: NonZeroU64,
    exhausted: bool,
}

impl Epoch {
    pub(crate) const fn new() -> Self {
        Self {
            generation: NonZeroU64::MIN,
            exhausted: false,
        }
    }

    #[cfg(test)]
    pub(crate) const fn current(&self) -> Generation {
        Generation(self.generation)
    }

    pub(crate) fn advance(&mut self) -> io::Result<Generation> {
        if self.exhausted {
            return Err(io::Error::other("browser lifecycle generation exhausted"));
        }
        let Some(value) = self.generation.get().checked_add(1) else {
            self.exhausted = true;
            return Err(io::Error::other("browser lifecycle generation exhausted"));
        };
        let generation = NonZeroU64::new(value).ok_or_else(|| {
            self.exhausted = true;
            io::Error::other("browser lifecycle generation became zero")
        })?;
        self.generation = generation;
        Ok(Generation(generation))
    }

    pub(crate) fn accepts(&self, generation: Generation) -> bool {
        !self.exhausted && self.generation == generation.0
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
        let mut epoch = Epoch {
            generation: NonZeroU64::new(u64::MAX).expect("nonzero"),
            exhausted: false,
        };
        let last_generation = epoch.current();
        assert!(epoch.advance().is_err());
        assert_eq!(
            epoch.current(),
            Generation(NonZeroU64::new(u64::MAX).expect("nonzero"))
        );
        assert!(!epoch.accepts(last_generation));
        assert!(epoch.advance().is_err());
    }
}
