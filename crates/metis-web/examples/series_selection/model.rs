//! Selection algebra; fixture identity and viewport assignments stay in Rust.

use std::fmt;

pub(super) const VIEWPORT_COUNT: usize = 4;
pub(super) const STUDY_NAME: &str = "Demonstration study · selection fixtures";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SeriesId {
    Ct,
    Mr,
    Secondary,
}

pub(super) struct SeriesFixture {
    pub(super) id: SeriesId,
    pub(super) key: &'static str,
    pub(super) control_id: &'static str,
    pub(super) title: &'static str,
    pub(super) description: &'static str,
}

pub(super) const SERIES: [SeriesFixture; 3] = [
    SeriesFixture {
        id: SeriesId::Ct,
        key: "fixture-ct",
        control_id: "series-fixture-ct",
        title: "Fixture CT series",
        description: "CT-DEMO · selection metadata only",
    },
    SeriesFixture {
        id: SeriesId::Mr,
        key: "fixture-mr",
        control_id: "series-fixture-mr",
        title: "Fixture MR series",
        description: "MR-DEMO · selection metadata only",
    },
    SeriesFixture {
        id: SeriesId::Secondary,
        key: "fixture-secondary",
        control_id: "series-fixture-secondary",
        title: "Fixture secondary series",
        description: "SECONDARY-DEMO · selection metadata only",
    },
];

impl SeriesId {
    pub(super) const fn fixture(self) -> &'static SeriesFixture {
        match self {
            Self::Ct => &SERIES[0],
            Self::Mr => &SERIES[1],
            Self::Secondary => &SERIES[2],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    SelectSeries(SeriesId),
    ActivateViewport(usize),
}

impl Action {
    pub(super) fn parse(control_id: &str) -> Result<Self, SelectionError> {
        if let Some(series) = SERIES.iter().find(|series| series.control_id == control_id) {
            return Ok(Self::SelectSeries(series.id));
        }
        let index = control_id
            .strip_prefix("viewport-")
            .ok_or(SelectionError::UnknownControl)?;
        let value = index
            .parse::<usize>()
            .map_err(|_| SelectionError::InvalidViewport)?;
        if value.to_string() != index {
            return Err(SelectionError::InvalidViewport);
        }
        Ok(Self::ActivateViewport(value))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SelectionError {
    UnknownControl,
    InvalidViewport,
    ViewportOutOfRange { index: usize, count: usize },
}

impl fmt::Display for SelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownControl => formatter.write_str("Unknown selection control"),
            Self::InvalidViewport => formatter.write_str("Malformed viewport identifier"),
            Self::ViewportOutOfRange { index, count } => {
                write!(
                    formatter,
                    "Viewport index {index} is outside the {count}-viewport layout"
                )
            }
        }
    }
}

/// For N > 0, maintains a < N and selected = assignments[a].
///
/// Selection writes exactly one array cell. Activation writes only a after
/// checking its bound. Rejection writes nothing. These facts prove locality,
/// restoration and idempotence without a separate mutable selection field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SelectionState<const N: usize> {
    active: usize,
    assignments: [Option<SeriesId>; N],
}

impl<const N: usize> SelectionState<N> {
    pub(super) const fn new() -> Self {
        const { assert!(N > 0, "a selection layout requires a viewport") };
        Self {
            active: 0,
            assignments: [None; N],
        }
    }

    pub(super) const fn active(&self) -> usize {
        self.active
    }

    pub(super) const fn selected(&self) -> Option<SeriesId> {
        self.assignments[self.active]
    }

    pub(super) const fn viewports(&self) -> &[Option<SeriesId>; N] {
        &self.assignments
    }

    pub(super) fn apply(&mut self, action: Action) -> Result<(), SelectionError> {
        match action {
            Action::SelectSeries(series) => self.assignments[self.active] = Some(series),
            Action::ActivateViewport(index) => {
                if index >= N {
                    return Err(SelectionError::ViewportOutOfRange { index, count: N });
                }
                self.active = index;
            }
        }
        Ok(())
    }
}
