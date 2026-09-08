//! Visible Windows host for the software-rendered Metis form.

use metis_core::error::{ErrorCode, MetisError, Result};
use metis_frontend::{FormState, FrontendApp};
use metis_ipc::{IpcTransport, StreamTransport};
use metis_platform::native::{
    CompositionPhase, MouseButton, NativeSurface, WindowConfig, WindowEvent,
};
use metis_platform::{Color, Rect};
use metis_ui_lang::{DisplayCommand, compute_layout};
use std::io::{stdin, stdout};
use std::time::Duration;

const INITIAL_WIDTH: u32 = 800;
const INITIAL_HEIGHT: u32 = 600;
const EVENT_WAIT: Duration = Duration::from_millis(250);
const MAX_PATIENT_ID_BYTES: usize = 128;
const RETURN_KEY: u32 = 0x0d;
const ESCAPE_KEY: u32 = 0x1b;
const BACKSPACE_KEY: u32 = 0x08;
const SUBMIT_LABEL: &str = "[ SUBMIT CALCULATION TO BACKEND ]";

/// Runs the visible Windows software-rendered form over the supervised pipe.
pub(crate) fn run(inputs: [String; 3]) -> Result<(), Box<dyn std::error::Error>> {
    let [weight, concentration, dose] = inputs;
    let transport = StreamTransport::new(stdin(), stdout());
    let mut app = FrontendApp::new(transport, INITIAL_WIDTH, INITIAL_HEIGHT)?;
    let pid = std::process::id();
    let mut principal_id = [0; 16];
    principal_id[..4].copy_from_slice(&pid.to_be_bytes());
    app.init(pid, principal_id)?;
    app.set_inputs(
        "demo",
        weight.parse()?,
        concentration.parse()?,
        dose.parse()?,
    )?;

    let config = WindowConfig::new("Metis native form", INITIAL_WIDTH, INITIAL_HEIGHT)?;
    let mut surface = NativeSurface::new(&config)?;
    surface.present(app.framebuffer())?;
    eprintln!(
        "native_frontend_pid={pid} window={}x{}",
        app.framebuffer().width(),
        app.framebuffer().height()
    );

    let mut patient_id = app.inputs().patient_id.clone();
    run_event_loop(&mut app, &mut surface, pid, &mut patient_id)
}

fn run_event_loop<T: IpcTransport>(
    app: &mut FrontendApp<T>,
    surface: &mut NativeSurface,
    pid: u32,
    patient_id: &mut String,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut focused = true;
    loop {
        let events = surface.wait_events(EVENT_WAIT)?;
        let mut repaint = false;
        for event in events {
            match event {
                WindowEvent::CloseRequested
                | WindowEvent::KeyDown {
                    virtual_key: ESCAPE_KEY,
                    ..
                } => {
                    surface.close()?;
                    return Ok(());
                }
                WindowEvent::Destroyed => return Ok(()),
                WindowEvent::FocusGained => focused = true,
                WindowEvent::FocusLost => {
                    focused = false;
                    if app.composition().is_some() {
                        app.set_composition(None)?;
                        repaint = true;
                    }
                }
                WindowEvent::Resized { width, height }
                    if width > 0
                        && height > 0
                        && (width != app.framebuffer().width()
                            || height != app.framebuffer().height()) =>
                {
                    app.resize(width, height)?;
                    repaint = true;
                }
                WindowEvent::DpiChanged { dpi } => {
                    eprintln!("native_dpi={dpi}");
                }
                WindowEvent::PointerUp {
                    x,
                    y,
                    button: MouseButton::Left,
                } => {
                    focused = true;
                    if submit_rect(app)?.contains(x, y) {
                        submit(app, pid)?;
                        repaint = true;
                    }
                }
                WindowEvent::KeyDown {
                    virtual_key: RETURN_KEY,
                    repeated: false,
                } => {
                    submit(app, pid)?;
                    repaint = true;
                }
                WindowEvent::KeyDown {
                    virtual_key: BACKSPACE_KEY,
                    ..
                } if focused => {
                    repaint |= remove_patient_character(app, patient_id)?;
                }
                WindowEvent::TextInput { character } => {
                    repaint |= focused && append_patient_character(app, patient_id, character)?;
                }
                WindowEvent::TextComposition { phase, text } if focused => {
                    repaint = true;
                    match phase {
                        CompositionPhase::Started | CompositionPhase::Updated => {
                            app.set_composition(Some(text))?;
                        }
                        CompositionPhase::Committed => {
                            app.set_composition(None)?;
                            append_patient_text(app, patient_id, &text)?;
                        }
                        CompositionPhase::Canceled => app.set_composition(None)?,
                    }
                }
                _ => {}
            }
        }
        if repaint {
            surface.present(app.framebuffer())?;
        }
    }
}

fn submit<T: IpcTransport>(app: &mut FrontendApp<T>, pid: u32) -> Result<()> {
    app.submit_calculation()?;
    if let FormState::Success(response) = app.state() {
        eprintln!(
            "native_submission=success frontend_pid={pid} rate_ml_hr={} drug_rate_mg_hr={} audit_sequence={}",
            response.rate_ml_hr, response.drug_rate_mg_hr, response.audit_sequence_id
        );
    }
    Ok(())
}

fn append_patient_character<T: IpcTransport>(
    app: &mut FrontendApp<T>,
    patient_id: &mut String,
    character: char,
) -> Result<bool> {
    let mut encoded = [0; 4];
    let text = character.encode_utf8(&mut encoded);
    append_patient_text(app, patient_id, text)
}

fn append_patient_text<T: IpcTransport>(
    app: &mut FrontendApp<T>,
    patient_id: &mut String,
    text: &str,
) -> Result<bool> {
    if text.chars().any(char::is_control) {
        return Ok(false);
    }
    // The bounded copy lets set_inputs commit the edit only after validation
    // and rendering succeed; native text input is a cold control-plane path.
    let mut updated = patient_id.clone();
    let next_bytes = updated
        .len()
        .checked_add(text.len())
        .ok_or_else(input_limit_error)?;
    if next_bytes > MAX_PATIENT_ID_BYTES {
        return Err(input_limit_error());
    }
    updated.try_reserve(text.len()).map_err(|_| {
        MetisError::ui(
            ErrorCode::SurfaceAllocationError,
            "Patient identifier storage reservation failed",
        )
    })?;
    updated.push_str(text);
    replace_patient_id(app, patient_id, updated)
}

fn remove_patient_character<T: IpcTransport>(
    app: &mut FrontendApp<T>,
    patient_id: &mut String,
) -> Result<bool> {
    let mut updated = patient_id.clone();
    if updated.pop().is_none() {
        return Ok(false);
    }
    replace_patient_id(app, patient_id, updated)
}

fn replace_patient_id<T: IpcTransport>(
    app: &mut FrontendApp<T>,
    patient_id: &mut String,
    updated: String,
) -> Result<bool> {
    if *patient_id == updated {
        return Ok(false);
    }
    let weight = app.inputs().weight_kg;
    let concentration = app.inputs().concentration_mg_ml;
    let dose = app.inputs().target_dose_mcg_kg_min;
    app.set_inputs(&updated, weight, concentration, dose)?;
    *patient_id = updated;
    Ok(true)
}

fn submit_rect<T: IpcTransport>(app: &FrontendApp<T>) -> Result<Rect> {
    let width = i32::try_from(app.framebuffer().width()).map_err(|_| layout_error())?;
    let height = i32::try_from(app.framebuffer().height()).map_err(|_| layout_error())?;
    let display = compute_layout(app.document(), width, height)?;
    let (label_x, label_y) = display
        .commands
        .iter()
        .find_map(|command| match command {
            DisplayCommand::DrawText { text, x, y, .. } if text == SUBMIT_LABEL => Some((*x, *y)),
            _ => None,
        })
        .ok_or_else(|| {
            MetisError::ui(
                ErrorCode::MalformedMarkup,
                "Authored form is missing the submit label",
            )
        })?;
    display
        .commands
        .iter()
        .find_map(|command| match command {
            DisplayCommand::FillRect { rect, color }
                if *color == Color::BLUE && rect.contains(label_x, label_y) =>
            {
                Some(*rect)
            }
            _ => None,
        })
        .ok_or_else(|| {
            MetisError::ui(
                ErrorCode::MalformedMarkup,
                "Authored form is missing the submit surface",
            )
        })
}

fn input_limit_error() -> MetisError {
    MetisError::protocol(
        ErrorCode::PayloadTooLarge,
        "Patient identifier exceeds the bounded native input limit",
    )
}

fn layout_error() -> MetisError {
    MetisError::ui(
        ErrorCode::LayoutOverflow,
        "Native surface dimensions exceed layout coordinates",
    )
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_PATIENT_ID_BYTES, append_patient_character, append_patient_text, input_limit_error,
        submit_rect,
    };
    use metis_frontend::FrontendApp;
    use metis_ipc::MemoryTransport;

    #[test]
    fn patient_text_rejects_controls_and_bounded_overflow() {
        let (transport, _peer) = MemoryTransport::pair();
        let mut app = FrontendApp::new(transport, 800, 600).expect("form");
        let mut value = String::from("patient");
        assert!(!append_patient_character(&mut app, &mut value, '\n').expect("control input"));
        value = "x".repeat(MAX_PATIENT_ID_BYTES);
        assert_eq!(
            append_patient_character(&mut app, &mut value, 'y').expect_err("bounded input"),
            input_limit_error()
        );
    }

    #[test]
    fn committed_composition_uses_the_same_patient_transition_as_text_input() {
        let (transport, _peer) = MemoryTransport::pair();
        let mut app = FrontendApp::new(transport, 800, 600).expect("form");
        let mut value = String::from("patient");
        app.set_inputs(&value, 70.0, 4.0, 0.5)
            .expect("initial patient value");
        app.set_composition(Some("東京".to_owned()))
            .expect("preedit value");
        append_patient_text(&mut app, &mut value, "東京").expect("commit value");
        assert_eq!(value, "patient東京");
        assert_eq!(app.inputs().patient_id, "patient東京");
        assert_eq!(app.composition(), None);
    }

    #[test]
    fn submit_hit_region_comes_from_the_authored_button_surface() {
        let (transport, _peer) = MemoryTransport::pair();
        let app = FrontendApp::new(transport, 800, 600).expect("form");
        let button = submit_rect(&app).expect("authored submit surface");
        assert!(button.contains(button.x, button.y));
        assert!(!button.contains(button.x - 1, button.y));
        assert!(!button.contains(button.x, button.y - 1));
    }
}
