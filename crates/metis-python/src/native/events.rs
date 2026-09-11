//! Native event conversion for the Python host.

use metis_platform::native::{CompositionPhase, ModifierState, MouseButton, WindowEvent};
use pyo3::prelude::{Bound, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods};
pub(super) enum Event {
    CloseRequested,
    Destroyed,
    FocusGained,
    FocusLost,
    PointerMove {
        x: i32,
        y: i32,
    },
    PointerDown {
        x: i32,
        y: i32,
        button: &'static str,
    },
    PointerUp {
        x: i32,
        y: i32,
        button: &'static str,
    },
    PointerWheel {
        x: i32,
        y: i32,
        delta_x: i16,
        delta_y: i16,
        modifiers: ModifierSnapshot,
    },
    KeyDown {
        virtual_key: u32,
        repeated: bool,
    },
    KeyUp {
        virtual_key: u32,
    },
    TextInput {
        character: char,
    },
    TextComposition {
        phase: &'static str,
        text: String,
    },
    Resized {
        width: u32,
        height: u32,
    },
    DpiChanged {
        dpi: u32,
    },
}

#[derive(Debug)]
pub(super) struct ModifierSnapshot {
    bits: u8,
}

impl ModifierSnapshot {
    const CTRL: u8 = 1;
    const SHIFT: u8 = 2;
    const ALT: u8 = 4;
    const META: u8 = 8;

    fn ctrl(&self) -> bool {
        self.bits & Self::CTRL != 0
    }
    fn shift(&self) -> bool {
        self.bits & Self::SHIFT != 0
    }
    fn alt(&self) -> bool {
        self.bits & Self::ALT != 0
    }
    fn meta(&self) -> bool {
        self.bits & Self::META != 0
    }
}

fn modifier_snapshot(value: ModifierState) -> ModifierSnapshot {
    let mut bits = 0;
    if value.ctrl() {
        bits |= ModifierSnapshot::CTRL;
    }
    if value.shift() {
        bits |= ModifierSnapshot::SHIFT;
    }
    if value.alt() {
        bits |= ModifierSnapshot::ALT;
    }
    if value.meta() {
        bits |= ModifierSnapshot::META;
    }
    ModifierSnapshot { bits }
}

fn button_name(value: MouseButton) -> &'static str {
    match value {
        MouseButton::Left => "left",
        MouseButton::Right => "right",
        MouseButton::Middle => "middle",
        MouseButton::X1 => "x1",
        MouseButton::X2 => "x2",
    }
}

fn phase_name(value: CompositionPhase) -> &'static str {
    match value {
        CompositionPhase::Started => "started",
        CompositionPhase::Updated => "updated",
        CompositionPhase::Committed => "committed",
        CompositionPhase::Canceled => "canceled",
    }
}

pub(super) fn event(value: WindowEvent) -> Event {
    match value {
        WindowEvent::CloseRequested => Event::CloseRequested,
        WindowEvent::Destroyed => Event::Destroyed,
        WindowEvent::FocusGained => Event::FocusGained,
        WindowEvent::FocusLost => Event::FocusLost,
        WindowEvent::PointerMove { x, y } => Event::PointerMove { x, y },
        WindowEvent::PointerDown {
            x,
            y,
            button: value,
        } => Event::PointerDown {
            x,
            y,
            button: button_name(value),
        },
        WindowEvent::PointerUp {
            x,
            y,
            button: value,
        } => Event::PointerUp {
            x,
            y,
            button: button_name(value),
        },
        WindowEvent::PointerWheel {
            x,
            y,
            delta_x,
            delta_y,
            modifiers: value,
        } => Event::PointerWheel {
            x,
            y,
            delta_x,
            delta_y,
            modifiers: modifier_snapshot(value),
        },
        WindowEvent::KeyDown {
            virtual_key,
            repeated,
        } => Event::KeyDown {
            virtual_key,
            repeated,
        },
        WindowEvent::KeyUp { virtual_key } => Event::KeyUp { virtual_key },
        WindowEvent::TextInput { character } => Event::TextInput { character },
        WindowEvent::TextComposition { phase: value, text } => Event::TextComposition {
            phase: phase_name(value),
            text,
        },
        WindowEvent::Resized { width, height } => Event::Resized { width, height },
        WindowEvent::DpiChanged { dpi } => Event::DpiChanged { dpi },
    }
}

pub(super) fn append_event<'py>(
    py: Python<'py>,
    list: &Bound<'py, PyList>,
    value: Event,
) -> PyResult<()> {
    let item = PyDict::new(py);
    macro_rules! set {
        ($key:literal, $value:expr) => {{ item.set_item($key, $value)? }};
    }
    match value {
        Event::CloseRequested => set!("kind", "close_requested"),
        Event::Destroyed => set!("kind", "destroyed"),
        Event::FocusGained => set!("kind", "focus_gained"),
        Event::FocusLost => set!("kind", "focus_lost"),
        Event::PointerMove { x, y } => {
            set!("kind", "pointer_move");
            set!("x", x);
            set!("y", y);
        }
        Event::PointerDown { x, y, button } => {
            set!("kind", "pointer_down");
            set!("x", x);
            set!("y", y);
            set!("button", button);
        }
        Event::PointerUp { x, y, button } => {
            set!("kind", "pointer_up");
            set!("x", x);
            set!("y", y);
            set!("button", button);
        }
        Event::PointerWheel {
            x,
            y,
            delta_x,
            delta_y,
            modifiers,
        } => {
            set!("kind", "pointer_wheel");
            set!("x", x);
            set!("y", y);
            set!("delta_x", delta_x);
            set!("delta_y", delta_y);
            set!("ctrl", modifiers.ctrl());
            set!("shift", modifiers.shift());
            set!("alt", modifiers.alt());
            set!("meta", modifiers.meta());
        }
        Event::KeyDown {
            virtual_key,
            repeated,
        } => {
            set!("kind", "key_down");
            set!("virtual_key", virtual_key);
            set!("repeated", repeated);
        }
        Event::KeyUp { virtual_key } => {
            set!("kind", "key_up");
            set!("virtual_key", virtual_key);
        }
        Event::TextInput { character } => {
            set!("kind", "text_input");
            set!("character", character);
        }
        Event::TextComposition { phase, text } => {
            set!("kind", "text_composition");
            set!("phase", phase);
            set!("text", text);
        }
        Event::Resized { width, height } => {
            set!("kind", "resized");
            set!("width", width);
            set!("height", height);
        }
        Event::DpiChanged { dpi } => {
            set!("kind", "dpi_changed");
            set!("dpi", dpi);
        }
    }
    list.append(item)
}
