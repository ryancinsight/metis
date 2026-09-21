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
        if request.target_node != native_accessibility::submit_button_identity() {
            return Ok(false);
        }
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
mod tests {
    use super::{
        MAX_PATIENT_ID_BYTES, NativeForm, append_patient_character, append_patient_text,
        input_limit_error, submit_rect,
    };
    use metis_frontend::FrontendApp;
    use metis_ipc::MemoryTransport;
    use metis_platform::DisplayScale;
    use metis_platform::native::{
        AccessibilityAction, AccessibilityActionRequest, CompositionPhase, ModifierState,
        NativeApplication, NativeFlow, WindowEvent,
    };

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
    fn native_composition_events_keep_preedit_transient_and_cancel_on_focus_loss() {
        let (transport, _peer) = MemoryTransport::pair();
        let mut app = FrontendApp::new(transport, 800, 600).expect("form");
        app.set_inputs("patient", 70.0, 4.0, 0.5)
            .expect("initial patient value");
        let mut form = NativeForm {
            app,
            pid: 1,
            patient_id: "patient".to_owned(),
            focused: true,
        };

        let flow = form
            .handle_events(&[
                WindowEvent::TextComposition {
                    phase: CompositionPhase::Started,
                    text: String::new(),
                },
                WindowEvent::TextComposition {
                    phase: CompositionPhase::Updated,
                    text: "東京".to_owned(),
                },
            ])
            .expect("preedit events");
        assert!(matches!(flow, NativeFlow::Continue { repaint: true }));
        assert_eq!(form.app.composition(), Some("東京"));
        assert_eq!(form.patient_id, "patient");
        assert_eq!(form.app.inputs().patient_id, "patient");

        let flow = form
            .handle_events(&[WindowEvent::FocusLost])
            .expect("focus loss");
        assert!(matches!(flow, NativeFlow::Continue { repaint: true }));
        assert_eq!(form.app.composition(), None);
        assert!(!form.focused);

        form.handle_events(&[
            WindowEvent::FocusGained,
            WindowEvent::TextComposition {
                phase: CompositionPhase::Started,
                text: String::new(),
            },
            WindowEvent::TextComposition {
                phase: CompositionPhase::Committed,
                text: "東京".to_owned(),
            },
        ])
        .expect("committed composition");
        assert_eq!(form.app.composition(), None);
        assert_eq!(form.patient_id, "patient東京");
        assert_eq!(form.app.inputs().patient_id, "patient東京");
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

    #[test]
    fn native_accessibility_focus_targets_the_authored_submit_control() {
        let (transport, _peer) = MemoryTransport::pair();
        let app = FrontendApp::new(transport, 800, 600).expect("form");
        let mut form = NativeForm {
            app,
            pid: 1,
            patient_id: "patient".to_owned(),
            focused: false,
        };
        let flow = form
            .handle_events(&[WindowEvent::AccessibilityAction {
                request: AccessibilityActionRequest {
                    target_node: super::native_accessibility::submit_button_identity(),
                    action: AccessibilityAction::Focus,
                    value: None,
                    delta: None,
                },
            }])
            .expect("accessibility focus");
        assert!(matches!(flow, NativeFlow::Continue { repaint: true }));
        assert!(form.focused);
    }

    #[test]
    fn dpi_event_repaints_and_scales_the_submit_hit_region() {
        let (transport, _peer) = MemoryTransport::pair();
        let app = FrontendApp::new(transport, 800, 600).expect("form");
        let initial = submit_rect(&app).expect("initial submit surface");
        let mut form = NativeForm {
            app,
            pid: 1,
            patient_id: "patient".to_owned(),
            focused: true,
        };
        let flow = form
            .handle_events(&[WindowEvent::DpiChanged { dpi: 144 }])
            .expect("DPI event");
        assert!(matches!(flow, NativeFlow::Continue { repaint: true }));
        assert_eq!(
            form.app.display_scale(),
            DisplayScale::from_dpi(144).expect("144 DPI")
        );
        let scaled = submit_rect(&form.app).expect("scaled submit surface");
        assert!(scaled.height > initial.height);
        assert!(scaled.x > initial.x);
    }

    #[test]
    fn enter_submission_accepts_plain_and_control_shortcuts() {
        assert_eq!(
            super::classify_submit_modifier_class(super::SubmitModifierClass::Plain),
            Some(super::SubmitShortcut::Plain)
        );
        assert_eq!(
            super::classify_submit_modifier_class(super::SubmitModifierClass::Control),
            Some(super::SubmitShortcut::Control)
        );
        assert_eq!(
            super::classify_submit_modifier_class(super::SubmitModifierClass::System),
            None
        );
        assert_eq!(
            super::submit_shortcut(ModifierState::NONE),
            Some(super::SubmitShortcut::Plain)
        );
    }
}
