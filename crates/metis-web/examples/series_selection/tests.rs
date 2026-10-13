use super::model::{Action, SERIES, STUDY_NAME, SelectionError, SelectionState, VIEWPORT_COUNT};

fn trace() -> tracing::subscriber::DefaultGuard {
    tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_test_writer()
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
            .finish(),
    )
}

#[test]
fn initial_state_has_one_active_viewport_and_no_assigned_series() {
    let _trace = trace();
    let state = tracing::info_span!("arrange").in_scope(SelectionState::<VIEWPORT_COUNT>::new);
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(state.active(), 0);
        assert_eq!(state.selected(), None);
        for index in 0..4 {
            assert_eq!(state.viewports()[index], None);
        }
        assert_eq!(STUDY_NAME, "Demonstration study · selection fixtures");
    });
}

fn check_assignment_algebra<const N: usize>() {
    for active in 0..N {
        for fixture in SERIES {
            let mut state = tracing::info_span!("arrange").in_scope(SelectionState::<N>::new);
            tracing::info_span!("act").in_scope(|| {
                assert_eq!(state.apply(Action::ActivateViewport(active)), Ok(()));
                assert_eq!(state.apply(Action::SelectSeries(fixture.id)), Ok(()));
            });
            tracing::info_span!("assert").in_scope(|| {
                assert_eq!(state.active(), active);
                assert_eq!(state.selected(), Some(fixture.id));
                for index in 0..N {
                    assert_eq!(
                        state.viewports()[index],
                        (index == active).then_some(fixture.id)
                    );
                }
            });
            let expected = state.clone();
            assert_eq!(state.apply(Action::SelectSeries(fixture.id)), Ok(()));
            assert_eq!(state.apply(Action::ActivateViewport(active)), Ok(()));
            assert_eq!(state, expected);
            for invalid in [N, usize::MAX] {
                assert_eq!(
                    state.apply(Action::ActivateViewport(invalid)),
                    Err(SelectionError::ViewportOutOfRange {
                        index: invalid,
                        count: N
                    }),
                );
                assert_eq!(state, expected);
            }
            for next in 0..N {
                assert_eq!(state.apply(Action::ActivateViewport(next)), Ok(()));
                assert_eq!(state.selected(), (next == active).then_some(fixture.id));
                assert_eq!(state.viewports()[active], Some(fixture.id));
            }
        }
    }
}

#[test]
fn selection_is_local_and_activation_restores_each_assignment() {
    let _trace = trace();
    check_assignment_algebra::<1>();
    check_assignment_algebra::<2>();
    check_assignment_algebra::<4>();
}

#[test]
fn parsed_controls_replace_the_active_assignment_and_preserve_other_viewports() {
    let _trace = trace();
    let mut state = tracing::info_span!("arrange").in_scope(SelectionState::<4>::new);
    tracing::info_span!("act").in_scope(|| {
        for id in [
            SERIES[0].control_id,
            "viewport-3",
            SERIES[1].control_id,
            SERIES[2].control_id,
        ] {
            assert_eq!(
                state.apply(Action::parse(id).expect("declared control")),
                Ok(())
            );
        }
    });
    tracing::info_span!("assert").in_scope(|| {
        assert_eq!(state.active(), 3);
        assert_eq!(state.selected(), Some(SERIES[2].id));
        assert_eq!(state.viewports()[0], Some(SERIES[0].id));
        assert_eq!(state.viewports()[1], None);
        assert_eq!(state.viewports()[2], None);
        assert_eq!(state.viewports()[3], Some(SERIES[2].id));
        assert_eq!(state.apply(Action::ActivateViewport(0)), Ok(()));
        assert_eq!(state.selected(), Some(SERIES[0].id));
        let fixture = state.selected().expect("active assignment").fixture();
        assert_eq!(fixture.key, "fixture-ct");
        assert_eq!(fixture.title, "Fixture CT series");
        assert_eq!(fixture.description, "CT-DEMO · selection metadata only");
    });
}

#[test]
fn rejected_controls_preserve_populated_state_and_report_the_failure_kind() {
    let _trace = trace();
    let mut state = tracing::info_span!("arrange").in_scope(|| {
        let mut state = SelectionState::<4>::new();
        assert_eq!(state.apply(Action::SelectSeries(SERIES[1].id)), Ok(()));
        state
    });
    let expected = state.clone();
    for (id, error) in [
        ("series-unknown", SelectionError::UnknownControl),
        ("", SelectionError::UnknownControl),
        ("viewport-", SelectionError::InvalidViewport),
        ("viewport--1", SelectionError::InvalidViewport),
        ("viewport-+1", SelectionError::InvalidViewport),
        ("viewport-01", SelectionError::InvalidViewport),
        ("viewport-1.0", SelectionError::InvalidViewport),
        (
            "viewport-18446744073709551616",
            SelectionError::InvalidViewport,
        ),
        (
            "viewport-4",
            SelectionError::ViewportOutOfRange { index: 4, count: 4 },
        ),
    ] {
        let actual = tracing::info_span!("act", control = id)
            .in_scope(|| Action::parse(id).and_then(|action| state.apply(action)));
        tracing::info_span!("assert").in_scope(|| {
            assert_eq!(actual, Err(error));
            assert_eq!(state, expected);
        });
    }
    assert_eq!(
        state.apply(Action::ActivateViewport(usize::MAX)),
        Err(SelectionError::ViewportOutOfRange {
            index: usize::MAX,
            count: 4
        }),
    );
    assert_eq!(state, expected);
}
