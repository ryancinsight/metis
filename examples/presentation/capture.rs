//! Software capture evidence and deliberate comparator perturbations.
use iris::render::RenderBackend;
use metis_frontend::{FormState, FrontendApp};
use metis_ipc::transport::MemoryTransport;
use metis_platform::{Color, Framebuffer};
use metis_ui_lang::{DisplayCommand, DomDocument, LayoutViewport, compute_layout};
use std::io::Write;

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
    capture_details(app, name, actions, &display.commands, records)
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

fn write_oracle(writer: &mut impl Write, category: &str, oracle: Oracle) -> std::io::Result<()> {
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

fn row(writer: &mut impl Write, category: &str, key: &str, value: &str) -> std::io::Result<()> {
    // Quoting every field keeps commas, CR/LF and quotes lossless in standard CSV.
    writeln!(
        writer,
        "\"{}\",\"{}\",\"{}\"",
        category.replace('"', "\"\""),
        key.replace('"', "\"\""),
        value.replace('"', "\"\"")
    )
}

pub(super) fn comparator_probes(initial: &DomDocument) -> Result<(), Box<dyn std::error::Error>> {
    for name in ["probe-label", "probe-geometry", "probe-color"] {
        let mut document = initial.clone();
        match name {
            "probe-label" => {
                if !document.set_text_content("status-badge", "SYSTEM CHANGED") {
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
