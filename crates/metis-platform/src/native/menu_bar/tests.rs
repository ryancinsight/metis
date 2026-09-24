use super::{MenuBar, MenuBarHost, MenuCommand, menu_label};
use crate::native::{NativeSurface, PopupMenu, PopupMenuItem, WindowConfig, WindowVisibility};
use metis_core::input::Accelerator;

#[test]
fn labels_carry_the_bound_shortcut() {
    let save = Accelerator::parse("Ctrl+S").expect("accelerator");
    assert_eq!(menu_label("&Save", Some(save)), "&Save\tCtrl+S");
    assert_eq!(menu_label("E&xit", None), "E&xit");
}

#[test]
fn native_surfaces_attach_and_remove_menu_bars() {
    let config =
        WindowConfig::with_visibility("Metis menu bar", 320, 240, WindowVisibility::Hidden)
            .expect("bounded native configuration");
    let mut surface = NativeSurface::new(&config).expect("native surface");
    let save = Accelerator::parse("Ctrl+S").expect("accelerator");
    let file = PopupMenu::new(vec![
        PopupMenuItem::action(&menu_label("&Save", Some(save)), true).expect("item"),
        PopupMenuItem::separator(),
        PopupMenuItem::action("E&xit", true).expect("item"),
    ])
    .expect("menu");
    let bar = MenuBar::new(vec![("&File", file)]).expect("bar");
    surface.set_menu_bar(Some(&bar)).expect("attach");
    surface.poll_events().expect("pump");
    assert_eq!(surface.take_menu_commands(), Vec::<MenuCommand>::new());
    surface.set_menu_bar(None).expect("remove");
    surface.close().expect("close");
    assert!(surface.set_menu_bar(Some(&bar)).is_err());
}
