//! DOM presentation derives every assignment label from the selection state.

use super::model::{SERIES, STUDY_NAME, SelectionState, VIEWPORT_COUNT};
use moirai_pal::wasm::{WebDocument, WebElement};
use std::{fmt::Write, io};

pub(super) fn markup() -> String {
    let mut markup = format!(
        "<div class=\"viewer-layout\"><aside class=\"viewer-sidebar\" aria-label=\"Demonstration study and series\"><h2>Study</h2><p>{STUDY_NAME}</p><h3>Series fixtures</h3><div role=\"group\" aria-label=\"Assign a fixture to the active viewport\">"
    );
    // All interpolated labels are immutable, source-owned fixture strings.
    for series in SERIES {
        write!(
            markup,
            "<button id=\"{}\" class=\"series-row\" type=\"button\" data-selection-control aria-pressed=\"false\" aria-describedby=\"fixture-notice\"><strong>{}</strong><small>{}</small></button>",
            series.control_id, series.title, series.description,
        )
        .expect("writing to a String is infallible");
    }
    markup.push_str("</div></aside><div class=\"viewer-grid\" role=\"group\" aria-label=\"Select the active viewport\">");
    for index in 0..VIEWPORT_COUNT {
        write!(
            markup,
            "<button id=\"viewport-{index}\" class=\"series-viewport\" type=\"button\" data-selection-control aria-pressed=\"false\" aria-describedby=\"fixture-notice\"><span>Viewport {}</span><strong id=\"viewport-{index}-series\"></strong><span id=\"viewport-{index}-detail\"></span><small>No image loaded</small></button>",
            index + 1,
        )
        .expect("writing to a String is infallible");
    }
    markup.push_str("</div></div><p id=\"selection-summary\" role=\"status\" aria-live=\"polite\" aria-atomic=\"true\"></p><p id=\"selection-error\" role=\"alert\"></p>");
    markup
}

pub(super) fn element(document: &WebDocument, id: &str) -> io::Result<WebElement> {
    document.get_element_by_id(id).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("Selection element #{id} is absent"),
        )
    })
}

pub(super) fn render(
    document: &WebDocument,
    state: &SelectionState<VIEWPORT_COUNT>,
) -> io::Result<()> {
    let selected = state.selected();
    let root = element(document, "selection-app")?;
    root.set_attribute("data-active-viewport", &state.active().to_string())?;
    root.set_attribute(
        "data-selected-series",
        selected.map_or("", |id| id.fixture().key),
    )?;
    for series in SERIES {
        element(document, series.control_id)?.set_attribute(
            "aria-pressed",
            if selected == Some(series.id) {
                "true"
            } else {
                "false"
            },
        )?;
    }
    for (index, assignment) in state.viewports().iter().enumerate() {
        let id = format!("viewport-{index}");
        let fixture = assignment.map(super::model::SeriesId::fixture);
        let viewport = element(document, &id)?;
        viewport.set_attribute(
            "aria-pressed",
            if state.active() == index {
                "true"
            } else {
                "false"
            },
        )?;
        viewport.set_attribute("data-series-id", fixture.map_or("", |series| series.key))?;
        element(document, &format!("{id}-series"))?
            .set_text(fixture.map_or("No demonstration series assigned", |series| series.title));
        element(document, &format!("{id}-detail"))?.set_text(
            fixture.map_or("Select a series fixture for this viewport", |series| {
                series.description
            }),
        );
    }
    element(document, "selection-summary")?.set_text(&format!(
        "Active viewport {}; selected series: {}",
        state.active() + 1,
        selected.map_or("none", |id| id.fixture().title),
    ));
    Ok(())
}

pub(super) fn error(document: &WebDocument, message: &str) {
    // Error reporting cannot depend on a successful application mount.
    if let Some(status) = document.get_element_by_id("selection-host-error") {
        status.set_text(message);
    }
}
