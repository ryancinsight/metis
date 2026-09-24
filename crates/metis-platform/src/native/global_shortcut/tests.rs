use super::{GlobalHotkey, GlobalShortcuts, HotkeyHost, HotkeyId, hotkey_for};
use crate::native::{NativeSurface, WindowConfig, WindowVisibility};
use metis_core::input::Accelerator;
use std::io;

/// Records registrations and replays scripted presses.
#[derive(Default)]
struct FakeHost {
    registered: Vec<(HotkeyId, GlobalHotkey)>,
    presses: Vec<HotkeyId>,
    refuse: bool,
}

impl HotkeyHost for FakeHost {
    fn register_hotkey(&mut self, id: HotkeyId, hotkey: GlobalHotkey) -> io::Result<()> {
        if self.refuse {
            return Err(io::Error::other("chord owned elsewhere"));
        }
        self.registered.push((id, hotkey));
        Ok(())
    }

    fn unregister_hotkey(&mut self, id: HotkeyId) -> io::Result<bool> {
        let before = self.registered.len();
        self.registered.retain(|(held, _)| *held != id);
        Ok(self.registered.len() < before)
    }

    fn take_hotkey_presses(&mut self) -> Vec<HotkeyId> {
        std::mem::take(&mut self.presses)
    }
}

fn accelerator(text: &str) -> Accelerator {
    Accelerator::parse(text).expect("accelerator")
}

#[test]
fn accelerators_convert_to_hotkey_chords() {
    let hotkey = hotkey_for(accelerator("Ctrl+Alt+Shift+F23")).expect("hotkey");
    assert_eq!(hotkey.virtual_key(), 0x86);
    let modifiers = hotkey.modifiers();
    assert!(modifiers.ctrl() && modifiers.alt() && modifiers.shift() && !modifiers.meta());
    assert!(
        hotkey_for(accelerator("Meta+K"))
            .expect("hotkey")
            .modifiers()
            .meta()
    );
    let bare = hotkey_for(accelerator("F5")).expect_err("no modifier");
    assert_eq!(bare.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn presses_resolve_to_bound_commands() {
    let mut host = FakeHost::default();
    let mut shortcuts = GlobalShortcuts::new();
    shortcuts
        .register(&mut host, accelerator("Ctrl+Alt+P"), "palette")
        .expect("palette");
    shortcuts
        .register(&mut host, accelerator("Ctrl+Alt+S"), "screenshot")
        .expect("screenshot");
    let duplicate = shortcuts
        .register(&mut host, accelerator("Ctrl+Alt+P"), "again")
        .expect_err("duplicate");
    assert_eq!(duplicate.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(host.registered.len(), 2);

    let (palette, screenshot) = (host.registered[0].0, host.registered[1].0);
    assert_ne!(palette, screenshot);
    host.presses = vec![screenshot, HotkeyId::new(99).expect("id"), palette];
    assert_eq!(
        shortcuts.take_commands(&mut host),
        [&"screenshot", &"palette"]
    );

    assert_eq!(
        shortcuts
            .unregister(&mut host, accelerator("Ctrl+Alt+P"))
            .expect("unregister"),
        Some("palette")
    );
    assert_eq!(host.registered.len(), 1);
    host.presses = vec![palette];
    assert!(shortcuts.take_commands(&mut host).is_empty());
    shortcuts
        .register(&mut host, accelerator("Ctrl+Alt+Q"), "quit")
        .expect("identifier reused");
    assert_eq!(host.registered[1].0, palette);
}

#[test]
fn refused_registrations_leave_no_binding() {
    let mut host = FakeHost {
        refuse: true,
        ..FakeHost::default()
    };
    let mut shortcuts = GlobalShortcuts::new();
    assert!(
        shortcuts
            .register(&mut host, accelerator("Ctrl+Alt+P"), ())
            .is_err()
    );
    assert!(shortcuts.is_empty());
}

#[test]
fn native_surfaces_hold_global_shortcuts() {
    let config =
        WindowConfig::with_visibility("Metis global shortcut", 320, 240, WindowVisibility::Hidden)
            .expect("bounded native configuration");
    let mut surface = NativeSurface::new(&config).expect("native surface");
    let mut shortcuts = GlobalShortcuts::new();
    let chord = accelerator("Ctrl+Alt+Shift+F23");
    shortcuts
        .register(&mut surface, chord, "summon")
        .expect("register");
    surface.poll_events().expect("pump");
    assert!(shortcuts.take_commands(&mut surface).is_empty());
    assert_eq!(
        shortcuts
            .unregister(&mut surface, chord)
            .expect("unregister"),
        Some("summon")
    );
    surface.close().expect("close");
}
