use super::{
    ExplorerStatus, MAX_RESULT_FILTER_BYTES, MAX_RESULT_ROWS, RESULT_PAGE_SIZE, ResultExplorer,
    ResultId, ResultRow, SortDirection, SortKey, SortOrder, VisibleEntry,
};
use metis_core::error::ErrorCode;
use metis_core::protocol::ClinicalCalcResponsePayload;

fn response(id: u64, volume: f64, drug: f64) -> ClinicalCalcResponsePayload {
    ClinicalCalcResponsePayload {
        audit_sequence_id: id,
        rate_ml_hr: volume,
        drug_rate_mg_hr: drug,
        is_pediatric: false,
        result_signature: [0; 32],
    }
}

fn ids(explorer: &ResultExplorer) -> Vec<u64> {
    explorer.visible_rows().map(|row| row.id().get()).collect()
}

#[test]
fn sort_filter_and_selection_preserve_value_semantics() {
    let mut explorer = ResultExplorer::new();
    explorer
        .record_response("PT-B", &response(2, 4.0, 8.0))
        .expect("valid row");
    explorer
        .record_response("PT-A", &response(1, 2.0, 4.0))
        .expect("valid row");
    explorer
        .record_response("PT-C", &response(3, 6.0, 12.0))
        .expect("valid row");
    assert_eq!(ids(&explorer), vec![3, 2, 1]);
    explorer
        .set_sort(SortOrder::new(SortKey::Patient, SortDirection::Ascending))
        .expect("sort index");
    assert_eq!(ids(&explorer), vec![1, 2, 3]);
    assert!(explorer.select(ResultId::new(2).expect("nonzero")));
    explorer.set_filter("PT-B").expect("filter index");
    assert_eq!(ids(&explorer), vec![2]);
    assert_eq!(explorer.selected_id().map(ResultId::get), Some(2));
    assert_eq!(explorer.selected().map(ResultRow::patient_id), Some("PT-B"));
}

#[test]
fn updates_preserve_selected_identity_and_replacements_clear_removed_rows() {
    let mut explorer = ResultExplorer::new();
    explorer
        .record_response("PT-A", &response(1, 2.0, 4.0))
        .expect("valid row");
    explorer
        .record_response("PT-B", &response(2, 4.0, 8.0))
        .expect("valid row");
    assert!(explorer.select(ResultId::new(1).expect("nonzero")));
    explorer
        .record_response("PT-A", &response(1, 3.0, 6.0))
        .expect("updated row");
    assert_eq!(explorer.selected_id().map(ResultId::get), Some(1));
    assert_eq!(explorer.selected().map(ResultRow::rate_ml_hr), Some(3.0));
    let replacement = ResultRow::try_new("PT-B", &response(2, 4.0, 8.0)).expect("row");
    explorer.replace_rows([replacement]).expect("replace rows");
    assert_eq!(explorer.selected_id(), None);
    assert_eq!(explorer.status(), &ExplorerStatus::Ready);
}

#[test]
fn disclosure_and_paging_bound_visible_entries() {
    let mut explorer = ResultExplorer::new();
    for id in 1_u32..=12 {
        explorer
            .record_response(
                if id % 2 == 0 { "PT-A" } else { "PT-B" },
                &response(u64::from(id), f64::from(id), f64::from(id)),
            )
            .expect("valid row");
    }
    assert_eq!(explorer.visible_count(), 14);
    assert_eq!(explorer.visible_entries().count(), RESULT_PAGE_SIZE);
    let group_id = match explorer.visible_entry(0).expect("group") {
        VisibleEntry::Group { id, expanded, .. } => {
            assert!(expanded);
            id
        }
        VisibleEntry::Row(_) => panic!("first entry is a group"),
    };
    explorer.toggle_group(group_id).expect("collapse");
    assert_eq!(explorer.visible_count(), 8);
    explorer.toggle_group(group_id).expect("expand");
    explorer.next_page();
    assert!(explorer.window_start() > 0);
    assert!(explorer.visible_entries().count() <= RESULT_PAGE_SIZE);
    explorer.previous_page();
    assert_eq!(explorer.window_start(), 0);
}

#[test]
fn bounds_and_malformed_values_are_rejected() {
    let invalid = response(0, 1.0, 2.0);
    assert_eq!(
        match ResultRow::try_new("PT-A", &invalid) {
            Ok(_) => panic!("zero sequence was accepted"),
            Err(error) => error.code,
        },
        ErrorCode::MalformedPayload
    );
    let invalid = response(1, f64::NAN, 2.0);
    assert_eq!(
        match ResultRow::try_new("PT-A", &invalid) {
            Ok(_) => panic!("nonfinite rate was accepted"),
            Err(error) => error.code,
        },
        ErrorCode::MalformedPayload
    );
    let mut explorer = ResultExplorer::new();
    assert_eq!(
        explorer
            .set_filter(&"x".repeat(MAX_RESULT_FILTER_BYTES + 1))
            .map_or_else(|error| error.code, |()| panic!("filter bound was accepted")),
        ErrorCode::PayloadTooLarge
    );
    let too_many = (1..=MAX_RESULT_ROWS + 1).map(|id| {
        ResultRow::try_new(
            "PT-A",
            &response(u64::try_from(id).expect("bounded row id"), 1.0, 2.0),
        )
        .expect("row")
    });
    assert_eq!(
        explorer
            .replace_rows(too_many)
            .map_or_else(|error| error.code, |()| panic!("row bound was accepted")),
        ErrorCode::PayloadTooLarge
    );
    assert_eq!(explorer.row_count(), 0);
}

/// One row as `(patient, audit id, volume rate, drug rate)`.
type Spec = (String, u64, f64, f64);

/// The flattened tree as the specification describes it, computed naively:
/// groups in first-seen order, each group's filtered rows in row order, and
/// groups ordered by label or by their first row.
fn reference_tree(explorer: &ResultExplorer, rows: &[Spec]) -> Vec<String> {
    let filter = explorer.filter();
    let sort = explorer.sort();
    let row_order = |left: &Spec, right: &Spec| {
        let ordering = match sort.key() {
            SortKey::Sequence => left.1.cmp(&right.1),
            SortKey::Patient => left.0.cmp(&right.0),
            SortKey::VolumeRate => left.2.total_cmp(&right.2),
            SortKey::DrugRate => left.3.total_cmp(&right.3),
        };
        let ordering = match sort.direction() {
            SortDirection::Ascending => ordering,
            SortDirection::Descending => ordering.reverse(),
        };
        ordering.then_with(|| left.1.cmp(&right.1))
    };
    let mut labels: Vec<&str> = Vec::new();
    for (label, ..) in rows {
        if !labels.contains(&label.as_str()) {
            labels.push(label);
        }
    }
    let mut groups: Vec<(&str, Vec<&Spec>)> = labels
        .iter()
        .map(|label| {
            let mut members: Vec<_> = rows
                .iter()
                .filter(|row| row.0 == *label && row.0.contains(filter))
                .collect();
            members.sort_by(|left, right| row_order(left, right));
            (*label, members)
        })
        .filter(|(_, members)| !members.is_empty())
        .collect();
    groups.sort_by(|left, right| {
        if sort.key() == SortKey::Patient {
            match sort.direction() {
                SortDirection::Ascending => left.0.cmp(right.0),
                SortDirection::Descending => right.0.cmp(left.0),
            }
        } else {
            row_order(left.1[0], right.1[0])
        }
    });
    let total = |label: &str| rows.iter().filter(|row| row.0 == label).count();
    groups
        .into_iter()
        .flat_map(|(label, members)| {
            std::iter::once(format!("group {label} {}", total(label)))
                .chain(members.into_iter().map(|row| format!("row {}", row.1)))
        })
        .collect()
}

fn flattened_tree(explorer: &mut ResultExplorer) -> Vec<String> {
    while explorer.can_previous() {
        explorer.previous_page();
    }
    let mut entries = Vec::new();
    loop {
        let start = explorer.window_start();
        let skip = entries.len().saturating_sub(start);
        entries.extend(
            explorer
                .visible_entries()
                .skip(skip)
                .map(|entry| match entry {
                    VisibleEntry::Group {
                        label, row_count, ..
                    } => format!("group {label} {row_count}"),
                    VisibleEntry::Row(row) => format!("row {}", row.id().get()),
                }),
        );
        if !explorer.can_next() {
            return entries;
        }
        explorer.next_page();
    }
}

#[test]
fn bucketed_tree_matches_the_reference_under_every_sort_and_filter() {
    let rows: Vec<Spec> = (1_u64..=40)
        .map(|id| {
            // Deterministic scatter across five patients with repeated rates,
            // so ties fall through to the identity order.
            let patient = format!("PT-{}", ["D", "A", "C", "E", "B"][(id * 7 % 5) as usize]);
            let volume = f64::from(u32::try_from(id * 13 % 9).expect("small"));
            let drug = f64::from(u32::try_from(id * 5 % 11).expect("small"));
            (patient, id, volume, drug)
        })
        .collect();
    let mut explorer = ResultExplorer::new();
    for (patient, id, volume, drug) in &rows {
        explorer
            .record_response(patient, &response(*id, *volume, *drug))
            .expect("valid row");
    }
    for key in [
        SortKey::Sequence,
        SortKey::Patient,
        SortKey::VolumeRate,
        SortKey::DrugRate,
    ] {
        for direction in [SortDirection::Ascending, SortDirection::Descending] {
            explorer
                .set_sort(SortOrder::new(key, direction))
                .expect("sort");
            for filter in ["", "PT-", "A", "E", "missing"] {
                explorer.set_filter(filter).expect("filter");
                assert_eq!(
                    flattened_tree(&mut explorer),
                    reference_tree(&explorer, &rows),
                    "{key:?} {direction:?} {filter:?}"
                );
            }
        }
    }
}
