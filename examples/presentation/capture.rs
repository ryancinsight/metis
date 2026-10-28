//! Software capture evidence and deliberate comparator perturbations.
use iris::render::RenderBackend;
use metis_frontend::{BADGE_CLOSED, BADGE_READY, FormState, FrontendApp};
use metis_ipc::transport::MemoryTransport;
use metis_platform::{Color, Framebuffer};
use metis_ui_lang::{
    DisplayCommand, DomDocument, Edit, LayoutViewport, TransitionProperty, compute_layout,
};
use std::fmt::Write;
use std::io::Write as IoWrite;
use std::time::Duration;

#[derive(Clone, Copy)]
pub(super) enum Oracle {
    Idle,
    Success {
        rate: f64,
        drug_rate: f64,
        sequence: u64,
    },
    Rejected(u16),
    Disconnected(u16),
}

pub(super) fn capture(
    app: &FrontendApp<MemoryTransport>,
    name: &str,
    expected: Oracle,
    actions: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let framebuffer = app.framebuffer();
    let display_scale = app.display_scale();
    let display = compute_layout(
        app.document(),
        LayoutViewport::with_scale(
            i32::try_from(framebuffer.width())?,
            i32::try_from(framebuffer.height())?,
            display_scale,
        ),
    )?;
    let mut records = std::io::BufWriter::new(std::fs::File::create(format!("output/{name}.csv"))?);
    row(&mut records, "category", "key", "value")?;
    for (key, value) in [
        ("schema", "1"),
        ("scenario", name),
        ("target", "software"),
        ("font", "atkinson-hyperlegible"),
    ] {
        row(&mut records, "meta", key, value)?;
    }
    row(&mut records, "meta", "scale", &display_scale.to_string())?;
    row(
        &mut records,
        "meta",
        "width",
        &framebuffer.width().to_string(),
    )?;
    row(
        &mut records,
        "meta",
        "height",
        &framebuffer.height().to_string(),
    )?;
    let inputs = app.inputs();
    row(&mut records, "input", "patient_id", &inputs.patient_id)?;
    for (key, value) in [
        ("weight_kg", inputs.weight_kg),
        ("concentration_mg_ml", inputs.concentration_mg_ml),
        ("target_dose_mcg_kg_min", inputs.target_dose_mcg_kg_min),
    ] {
        row(&mut records, "input", key, &value.to_string())?;
    }
    let observed = match app.state() {
        FormState::Idle => Oracle::Idle,
        FormState::Success(response) => Oracle::Success {
            rate: response.rate_ml_hr,
            drug_rate: response.drug_rate_mg_hr,
            sequence: response.audit_sequence_id,
        },
        FormState::Rejected(error) => Oracle::Rejected(error.error_code),
        FormState::Disconnected(error) => Oracle::Disconnected(error.code as u16),
        state => return Err(format!("Unsupported stable capture state: {state:?}").into()),
    };
    write_oracle(&mut records, "observed", observed)?;
    write_oracle(&mut records, "expected", expected)?;
    capture_details(app, name, actions, display.commands(), records)
}

fn capture_details(
    app: &FrontendApp<MemoryTransport>,
    name: &str,
    actions: &[String],
    commands: &[DisplayCommand],
    mut records: std::io::BufWriter<std::fs::File>,
) -> Result<(), Box<dyn std::error::Error>> {
    for id in [
        "status-badge",
        "label-patient",
        "label-weight",
        "label-conc",
        "label-dose",
        "output-rate",
        "output-status",
        "output-signature",
    ] {
        let element = app
            .document()
            .find_element_by_id(id)
            .ok_or("Missing authored label")?;
        row(&mut records, "label", id, &element.text_content())?;
    }
    for (index, action) in actions.iter().enumerate() {
        row(&mut records, "action", &index.to_string(), action)?;
    }
    let mut index = 0;
    for command in commands {
        if let DisplayCommand::DrawText { text, x, y, style } = command {
            let width = style.advance(text);
            let height = style.line_height();
            assert!(
                *x >= 0 && f64::from(*x) + width <= f64::from(app.framebuffer().width()),
                "horizontal clipping: {text}"
            );
            assert!(
                *y >= 0 && f64::from(*y) + height <= f64::from(app.framebuffer().height()),
                "vertical clipping: {text}"
            );
            row(
                &mut records,
                "geometry",
                &index.to_string(),
                // Size in pixels per em, then the measured run width and
                // line height rounded up to whole pixels.
                &format!(
                    "{x} {y} {} {:.0} {:.0}",
                    style.size.pixels(),
                    width.ceil(),
                    height.ceil()
                ),
            )?;
            row(&mut records, "text", &index.to_string(), text)?;
            index += 1;
        }
    }
    records.flush()?;
    write_framebuffer(app.framebuffer(), name)
}

/// One sampled frame of the authored badge transition.
struct TransitionStep {
    millis: u64,
    color: Color,
    repainted: bool,
}

/// The color the shown frame paints for the badge.
fn painted_badge(app: &FrontendApp<MemoryTransport>) -> Result<Color, Box<dyn std::error::Error>> {
    let display = app
        .painted_display()
        .ok_or("the form has not painted a frame")?;
    let badge = display
        .element_index("status-badge")
        .ok_or("the badge declares no transition")?;
    display
        .element_color(badge, TransitionProperty::Color)
        .ok_or_else(|| "the badge paints no text color".into())
}

fn hex(color: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}

/// Whether `color` lies within the endpoints on every channel without being
/// either endpoint.
///
/// A channel with a short distance to travel can reach its endpoint before the
/// run does — the badge's blue moves two steps against the green's fifty-two —
/// so containment is inclusive per channel while the color as a whole must
/// still be a sample rather than an endpoint.
fn inside(color: Color, from: Color, to: Color) -> bool {
    let within = |value: u8, start: u8, end: u8| value >= start.min(end) && value <= start.max(end);
    within(color.r, from.r, to.r)
        && within(color.g, from.g, to.g)
        && within(color.b, from.b, to.b)
        && color != from
        && color != to
}

/// Whether every channel moves towards its target and never past it.
fn advances(steps: &[TransitionStep]) -> bool {
    let towards = |earlier: u8, later: u8, start: u8, end: u8| {
        if start <= end {
            later >= earlier
        } else {
            later <= earlier
        }
    };
    steps.windows(2).all(|pair| {
        let (earlier, later) = (pair[0].color, pair[1].color);
        towards(earlier.r, later.r, BADGE_READY.r, BADGE_CLOSED.r)
            && towards(earlier.g, later.g, BADGE_READY.g, BADGE_CLOSED.g)
            && towards(earlier.b, later.b, BADGE_READY.b, BADGE_CLOSED.b)
    })
}

/// Records the authored badge transition as it moves.
///
/// The overlay samples at the paint boundary, so each step is a frame the
/// software host actually painted: the color is read back from the list that
/// painted it, and the recorded image is that same frame. The oracle is that
/// the first step is the ready color, the last the closed color, every interior
/// step is a sample between them, and no channel reverses or overshoots — so a
/// run that jumped, stalled or overshot cannot be recorded as a transition.
pub(super) fn capture_transition(
    app: &mut FrontendApp<MemoryTransport>,
) -> Result<(), Box<dyn std::error::Error>> {
    // 150 ms declared: 0 is the source, 150 the target, and the rest interior.
    const STEPS: [u64; 5] = [0, 50, 75, 100, 150];
    // The step whose painted frame becomes the manual's figure.
    const RECORDED: u64 = 75;
    let mut steps = Vec::with_capacity(STEPS.len());
    for millis in STEPS {
        app.set_now(Duration::from_millis(millis));
        let repainted = app.advance_transitions();
        let color = painted_badge(app)?;
        if millis == RECORDED {
            write_framebuffer(app.framebuffer(), "form-transition")?;
        }
        steps.push(TransitionStep {
            millis,
            color,
            repainted,
        });
    }
    let (Some(first), Some(last)) = (steps.first(), steps.last()) else {
        return Err("empty transition recording".into());
    };
    if first.color != BADGE_READY || last.color != BADGE_CLOSED {
        return Err(format!(
            "badge transition ran {} -> {} instead of {} -> {}",
            hex(first.color),
            hex(last.color),
            hex(BADGE_READY),
            hex(BADGE_CLOSED)
        )
        .into());
    }
    for step in steps.iter().skip(1).take(steps.len().saturating_sub(2)) {
        if !inside(step.color, BADGE_READY, BADGE_CLOSED) {
            return Err(format!(
                "step {} sampled {}, which is not a sample between the endpoints",
                step.millis,
                hex(step.color)
            )
            .into());
        }
    }
    if !advances(&steps) {
        return Err("badge transition reversed or overshot a channel".into());
    }
    write_transition_steps(&steps, app, RECORDED)
}

/// Writes the sampled steps beside the recorded frame, with the declaration
/// that produced them.
fn write_transition_steps(
    steps: &[TransitionStep],
    app: &FrontendApp<MemoryTransport>,
    recorded: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let display = app
        .painted_display()
        .ok_or("the form has not painted a frame")?;
    let badge = display
        .element_index("status-badge")
        .ok_or("the badge declares no transition")?;
    let transition = display
        .element_transition(badge)
        .ok_or("the badge declares no transition")?;
    let mut json = String::from("{\n");
    for (key, value) in [
        ("schema", "1".to_owned()),
        ("scenario", "form-transition".to_owned()),
        ("target", "software".to_owned()),
        ("element", "status-badge".to_owned()),
        ("property", transition.property().as_str().to_owned()),
        ("duration_ms", transition.duration_millis().to_string()),
        ("easing", transition.easing().as_str().to_owned()),
        ("recorded_step_ms", recorded.to_string()),
    ] {
        writeln!(json, "  \"{key}\": \"{value}\",").map_err(Box::<dyn std::error::Error>::from)?;
    }
    json.push_str("  \"steps\": [\n");
    for (index, step) in steps.iter().enumerate() {
        let comma = if index + 1 == steps.len() { "" } else { "," };
        writeln!(
            json,
            "    {{\"elapsed_ms\": {}, \"color\": \"{}\", \"repainted\": {}}}{comma}",
            step.millis,
            hex(step.color),
            step.repainted
        )
        .map_err(Box::<dyn std::error::Error>::from)?;
    }
    json.push_str("  ]\n}\n");
    std::fs::write("output/form-transition-steps.json", json)?;
    Ok(())
}

fn write_oracle(writer: &mut impl IoWrite, category: &str, oracle: Oracle) -> std::io::Result<()> {
    match oracle {
        Oracle::Idle => row(writer, category, "state", "idle"),
        Oracle::Success {
            rate,
            drug_rate,
            sequence,
        } => {
            row(writer, category, "state", "success")?;
            row(writer, category, "rate_ml_hr", &rate.to_string())?;
            row(writer, category, "drug_rate_mg_hr", &drug_rate.to_string())?;
            row(writer, category, "audit_sequence_id", &sequence.to_string())
        }
        Oracle::Rejected(code) | Oracle::Disconnected(code) => {
            row(
                writer,
                category,
                "state",
                if matches!(oracle, Oracle::Rejected(_)) {
                    "rejected"
                } else {
                    "disconnected"
                },
            )?;
            row(writer, category, "error_code", &code.to_string())
        }
    }
}

fn row(writer: &mut impl IoWrite, category: &str, key: &str, value: &str) -> std::io::Result<()> {
    // Quoting every field keeps commas, CR/LF and quotes lossless in standard CSV.
    writer.write_fmt(format_args!(
        "\"{}\",\"{}\",\"{}\"\n",
        category.replace('"', "\"\""),
        key.replace('"', "\"\""),
        value.replace('"', "\"\"")
    ))
}

pub(super) fn comparator_probes(initial: &DomDocument) -> Result<(), Box<dyn std::error::Error>> {
    for name in ["probe-label", "probe-geometry", "probe-color"] {
        let mut document = initial.clone();
        match name {
            "probe-label" => {
                if document.set_text_content("status-badge", "SYSTEM CHANGED") == Edit::Missing {
                    return Err("Missing probe label".into());
                }
            }
            "probe-geometry" => {
                let header = document
                    .find_element_by_id_mut("header")
                    .ok_or("Missing probe header")?;
                header.computed_style.padding.left += 1;
            }
            "probe-color" => {
                let badge = document
                    .find_element_by_id_mut("status-badge")
                    .ok_or("Missing probe label")?;
                badge.computed_style.text_color = Color::RED;
            }
            _ => unreachable!("invariant: the probe set is declared above"),
        }
        let display = compute_layout(&document, LayoutViewport::new(800, 600))?;
        let mut framebuffer = Framebuffer::new(800, 600)?;
        framebuffer.clear(Color::rgb(240, 244, 248));
        // Deliberately altered renderer fixtures never represent application outcomes.
        framebuffer.render(&display)?;
        write_framebuffer(&framebuffer, name)?;
    }
    Ok(())
}

fn write_framebuffer(
    framebuffer: &Framebuffer,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::write(
        format!("output/{name}.bmp"),
        crate::framebuffer_artifacts::bmp_bytes(framebuffer)?,
    )?;
    std::fs::write(
        format!("output/{name}.png"),
        crate::framebuffer_artifacts::png_bytes(framebuffer)?,
    )?;
    Ok(())
}
