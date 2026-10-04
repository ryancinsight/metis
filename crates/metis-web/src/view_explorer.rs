//! Cached-write renderer for the result explorer panel.

use super::BrowserState;
use super::view::{render_explorer_summary, set_attribute, set_disabled, set_text};
use crate::dom_cache::DomWriteCache;
use metis_frontend::{RESULT_PAGE_SIZE, VisibleEntry};
use moirai_pal::wasm::WebDocument;
use std::io;

pub(super) fn render_explorer(document: &WebDocument, state: &mut BrowserState) -> io::Result<()> {
    render_explorer_summary(document, state)?;
    render_explorer_entries(document, state)?;
    render_explorer_pagination(document, state)
}

fn render_explorer_entries(document: &WebDocument, state: &mut BrowserState) -> io::Result<()> {
    let BrowserState {
        result_explorer: explorer,
        dom_cache,
        ..
    } = &mut *state;
    for slot in 0..RESULT_PAGE_SIZE {
        let id = format!("explorer-entry-{slot}");
        let entry = explorer.visible_entry(slot);
        render_explorer_entry(
            document,
            &id,
            dom_cache,
            entry.as_ref(),
            explorer.selected_id(),
        )?;
    }
    Ok(())
}

fn render_explorer_entry(
    document: &WebDocument,
    id: &str,
    cache: &mut DomWriteCache,
    entry: Option<&VisibleEntry<'_>>,
    selected_id: Option<metis_frontend::ResultId>,
) -> io::Result<()> {
    match entry {
        Some(VisibleEntry::Group {
            id: group_id,
            label,
            expanded,
            row_count,
        }) => render_group_entry(document, id, cache, group_id, label, *expanded, *row_count),
        Some(VisibleEntry::Row(row)) => render_row_entry(document, id, cache, row, selected_id),
        None => render_empty_entry(document, id, cache),
    }
}

fn render_group_entry(
    document: &WebDocument,
    id: &str,
    cache: &mut DomWriteCache,
    group_id: &metis_frontend::GroupId,
    label: &str,
    expanded: bool,
    row_count: usize,
) -> io::Result<()> {
    let disclosure = if expanded { "expanded" } else { "collapsed" };
    let text = format!("{label} — {row_count} result(s) — {disclosure}");
    set_text(cache, document, id, &text)?;
    set_attribute(
        cache,
        document,
        id,
        "class",
        "explorer-entry explorer-group",
    )?;
    set_attribute(cache, document, id, "aria-label", &text)?;
    set_attribute(cache, document, id, "aria-hidden", "false")?;
    set_attribute(
        cache,
        document,
        id,
        "aria-expanded",
        if expanded { "true" } else { "false" },
    )?;
    set_attribute(cache, document, id, "aria-level", "1")?;
    set_attribute(cache, document, id, "data-entry-kind", "group")?;
    set_attribute(
        cache,
        document,
        id,
        "data-group-id",
        &group_id.get().to_string(),
    )?;
    set_disabled(cache, document, id, false)?;
    Ok(())
}

fn render_row_entry(
    document: &WebDocument,
    id: &str,
    cache: &mut DomWriteCache,
    row: &metis_frontend::ResultRow,
    selected_id: Option<metis_frontend::ResultId>,
) -> io::Result<()> {
    let selected = selected_id == Some(row.id());
    let selection = if selected { " — selected" } else { "" };
    let text = format!(
        "{} — sequence {} — {:.3} mL/hr — {:.3} mg/hr{selection}",
        row.patient_id(),
        row.id().get(),
        row.rate_ml_hr(),
        row.drug_rate_mg_hr(),
    );
    set_text(cache, document, id, &text)?;
    set_attribute(
        cache,
        document,
        id,
        "class",
        if selected {
            "explorer-entry explorer-row explorer-row-selected"
        } else {
            "explorer-entry explorer-row"
        },
    )?;
    set_attribute(cache, document, id, "aria-label", &text)?;
    set_attribute(cache, document, id, "aria-hidden", "false")?;
    set_attribute(cache, document, id, "aria-expanded", "false")?;
    set_attribute(cache, document, id, "aria-level", "2")?;
    set_attribute(
        cache,
        document,
        id,
        "aria-pressed",
        if selected { "true" } else { "false" },
    )?;
    set_attribute(cache, document, id, "data-entry-kind", "row")?;
    set_attribute(
        cache,
        document,
        id,
        "data-result-id",
        &row.id().get().to_string(),
    )?;
    set_disabled(cache, document, id, false)?;
    Ok(())
}

fn render_empty_entry(
    document: &WebDocument,
    id: &str,
    cache: &mut DomWriteCache,
) -> io::Result<()> {
    set_text(cache, document, id, "No visible result")?;
    set_attribute(
        cache,
        document,
        id,
        "class",
        "explorer-entry explorer-entry-empty",
    )?;
    set_attribute(cache, document, id, "aria-label", "No visible result")?;
    set_attribute(cache, document, id, "aria-hidden", "true")?;
    set_attribute(cache, document, id, "aria-expanded", "false")?;
    set_attribute(cache, document, id, "aria-level", "1")?;
    set_attribute(cache, document, id, "aria-pressed", "false")?;
    set_attribute(cache, document, id, "data-entry-kind", "empty")?;
    set_attribute(cache, document, id, "data-group-id", "")?;
    set_attribute(cache, document, id, "data-result-id", "")?;
    set_disabled(cache, document, id, true)?;
    Ok(())
}

fn render_explorer_pagination(document: &WebDocument, state: &mut BrowserState) -> io::Result<()> {
    let BrowserState {
        result_explorer: explorer,
        dom_cache,
        ..
    } = &mut *state;
    let window_status = if explorer.visible_count() == 0 {
        "Entries 0 of 0".to_owned()
    } else {
        let first = explorer.window_start() + 1;
        let last = (explorer.window_start() + RESULT_PAGE_SIZE).min(explorer.visible_count());
        format!(
            "Entries {first}–{last} of {}; {} retained",
            explorer.visible_count(),
            explorer.row_count()
        )
    };
    set_text(
        dom_cache,
        document,
        "explorer-window-status",
        &window_status,
    )?;
    set_disabled(
        dom_cache,
        document,
        "explorer-previous",
        !explorer.can_previous(),
    )?;
    set_disabled(dom_cache, document, "explorer-next", !explorer.can_next())?;
    Ok(())
}
