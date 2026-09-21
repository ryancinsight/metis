//! Visible Windows host for the software-rendered Metis form.

use metis_core::error::{ErrorCode, MetisError, Result};
use metis_frontend::{FormState, FrontendApp};
use metis_ipc::{IpcTransport, StreamTransport};
use metis_platform::native::{
    AccessibilityAction, AccessibilityActionRequest, AccessibilityTree, CompositionPhase,
    ModifierState, MouseButton, NativeApplication, NativeFlow, WindowConfig, WindowEvent,
    run_native_application,
};
use metis_platform::{Color, DisplayScale, Framebuffer, Rect};
use metis_ui_lang::{DisplayCommand, LayoutViewport, compute_layout};
use std::io::{stdin, stdout};
use std::time::Duration;

use super::native_accessibility;

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
    eprintln!(
        "native_frontend_pid={pid} window={}x{}",
        app.framebuffer().width(),
        app.framebuffer().height()
    );

    let patient_id = app.inputs().patient_id.clone();
    let application = NativeForm {
        app,
        pid,
        patient_id,
        focused: true,
    };
    run_native_application(&config, application, EVENT_WAIT)?;
    Ok(())
}

struct NativeForm<T> {
    app: FrontendApp<T>,
    pid: u32,
    patient_id: String,
    focused: bool,
}

impl<T: IpcTransport> NativeApplication for NativeForm<T> {
    type Error = MetisError;

    fn framebuffer(&self) -> &Framebuffer {
        self.app.framebuffer()
    }

    fn accessibility_tree(&self) -> Result<Option<AccessibilityTree>> {
        native_accessibility::project(&self.app).map(Some)
    }

    fn handle_events(&mut self, events: &[WindowEvent]) -> Result<NativeFlow> {
        let mut repaint = false;
        for event in events {
            match event {
                WindowEvent::CloseRequested
                | WindowEvent::Destroyed
                | WindowEvent::KeyDown {
                    virtual_key: ESCAPE_KEY,
                    ..
                } => return Ok(NativeFlow::Exit),
                WindowEvent::FocusGained => self.focused = true,
                WindowEvent::FocusLost => {
                    self.focused = false;
                    if self.app.composition().is_some() {
                        self.app.set_composition(None)?;
                        repaint = true;
                    }
                }
                WindowEvent::Resized { width, height }
                    if *width > 0
                        && *height > 0
                        && (*width != self.app.framebuffer().width()
                            || *height != self.app.framebuffer().height()) =>
                {
                    self.app.resize(*width, *height)?;
                    repaint = true;
                }
                WindowEvent::DpiChanged { dpi } => {
                    let display_scale = DisplayScale::from_dpi(*dpi)?;
                    self.app.set_display_scale(display_scale)?;
                    eprintln!("native_dpi={dpi} display_scale={display_scale}");
                    repaint = true;
                }
                WindowEvent::PointerUp {
                    x,
                    y,
                    button: MouseButton::Left,
                } => {
                    self.focused = true;
                    if submit_rect(&self.app)?.contains(*x, *y) {
                        submit(&mut self.app, self.pid)?;
                        repaint = true;
                    }
                }
                WindowEvent::KeyDown {
                    virtual_key: RETURN_KEY,
                    repeated: false,
                    modifiers,
                } if submit_shortcut(*modifiers).is_some() => {
                    submit(&mut self.app, self.pid)?;
                    repaint = true;
                }
                WindowEvent::KeyDown {
                    virtual_key: BACKSPACE_KEY,
                    ..
                } if self.focused => {
                    repaint |= remove_patient_character(&mut self.app, &mut self.patient_id)?;
                }
                WindowEvent::TextInput { character } => {
                    repaint |= self.focused
                        && append_patient_character(
                            &mut self.app,
                            &mut self.patient_id,
                            *character,
                        )?;
                }
                WindowEvent::TextComposition { phase, text } if self.focused => {
                    repaint = true;
                    match phase {
                        CompositionPhase::Started | CompositionPhase::Updated => {
                            self.app.set_composition(Some(text.clone()))?;
                        }
                        CompositionPhase::Committed => {
                            self.app.set_composition(None)?;
                            append_patient_text(&mut self.app, &mut self.patient_id, text)?;
                        }
                        CompositionPhase::Canceled => self.app.set_composition(None)?,
                    }
                }
                WindowEvent::AccessibilityAction { request } => {
                    repaint |= self.apply_accessibility_action(request)?;
                }
                _ => {}
            }
        }
        Ok(NativeFlow::Continue { repaint })
    }
}

impl<T: IpcTransport> NativeForm<T> {
    fn apply_accessibility_action(&mut self, request: &AccessibilityActionRequest) -> Result<bool> {
        match request.target_node {
            target if target == native_accessibility::submit_button_identity() => {
                match request.action {
                    AccessibilityAction::Focus => {
                        self.focused = true;
                        Ok(true)
                    }
                    AccessibilityAction::Activate => {
                        submit(&mut self.app, self.pid)?;
                        Ok(true)
                    }
                    _ => Ok(false),
                }
            }
            target if target == native_accessibility::patient_input_identity() => {
                match request.action {
                    AccessibilityAction::Focus => {
                        self.focused = true;
                        Ok(true)
                    }
                    AccessibilityAction::SetValue => {
                        let Some(value) = request.value.as_deref() else {
                            return Ok(false);
                        };
                        set_patient_value(&mut self.app, &mut self.patient_id, value)
                    }
                    _ => Ok(false),
                }
            }
            _ => Ok(false),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubmitShortcut {
    Plain,
    Control,
}

fn submit_shortcut(modifiers: ModifierState) -> Option<SubmitShortcut> {
    let class = if modifiers.shift() || modifiers.alt() || modifiers.meta() {
        SubmitModifierClass::System
    } else if modifiers.ctrl() {
        SubmitModifierClass::Control
    } else {
        SubmitModifierClass::Plain
    };
    classify_submit_modifier_class(class)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubmitModifierClass {
    Plain,
    Control,
    System,
}

const fn classify_submit_modifier_class(class: SubmitModifierClass) -> Option<SubmitShortcut> {
    match class {
        SubmitModifierClass::Plain => Some(SubmitShortcut::Plain),
        SubmitModifierClass::Control => Some(SubmitShortcut::Control),
        SubmitModifierClass::System => None,
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
    validate_patient_value(&updated)?;
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

fn set_patient_value<T: IpcTransport>(
    app: &mut FrontendApp<T>,
    patient_id: &mut String,
    value: &str,
) -> Result<bool> {
    validate_patient_value(value)?;
    replace_patient_id(app, patient_id, value.to_owned())
}

fn validate_patient_value(value: &str) -> Result<()> {
    if value.len() > MAX_PATIENT_ID_BYTES {
        return Err(input_limit_error());
    }
    if value.chars().any(char::is_control) {
        return Err(MetisError::protocol(
            ErrorCode::MalformedPayload,
            "Patient identifier contains a control character",
        ));
    }
    Ok(())
}

fn submit_rect<T: IpcTransport>(app: &FrontendApp<T>) -> Result<Rect> {
    let width = i32::try_from(app.framebuffer().width()).map_err(|_| layout_error())?;
    let height = i32::try_from(app.framebuffer().height()).map_err(|_| layout_error())?;
    let display = compute_layout(
        app.document(),
        LayoutViewport::with_scale(width, height, app.display_scale()),
    )?;
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
mod tests;
