use super::{PopupMenu, PopupMenuItem, TrayEvent, TrayHost, tray_image};
use crate::native::{NativeSurface, WindowConfig, WindowVisibility};
use crate::rasterizer::CornerRadius;
use crate::{Color, Framebuffer, Rect, fill_rect};

fn icon(size: u32) -> Framebuffer {
    let mut framebuffer = Framebuffer::new(size, size).expect("framebuffer");
    framebuffer.clear(Color::rgba(0, 0, 0, 0));
    let edge = i32::try_from(size).expect("edge");
    let tile = Rect::new(2, 2, edge - 4, edge - 4);
    fill_rect(
        &mut framebuffer,
        tile,
        CornerRadius::clamped(edge / 4, tile),
        Color::rgb(0x00, 0x78, 0xd4),
    );
    framebuffer
}

#[test]
fn only_square_supported_framebuffers_become_tray_images() {
    assert_eq!(tray_image(&icon(16)).expect("16").size(), 16);
    assert_eq!(tray_image(&icon(32)).expect("32").size(), 32);
    assert!(tray_image(&icon(24)).is_err());
    let wide = Framebuffer::new(32, 16).expect("framebuffer");
    assert!(tray_image(&wide).is_err());
}

#[test]
fn native_surfaces_show_tray_icons_and_notifications() {
    let config = WindowConfig::with_visibility("Metis tray", 320, 240, WindowVisibility::Hidden)
        .expect("bounded native configuration");
    let mut surface = NativeSurface::new(&config).expect("native surface");
    let image = tray_image(&icon(16)).expect("image");
    assert!(surface.show_notification("Metis", "No icon yet").is_err());
    surface
        .show_tray_icon(&image, "Metis tray test")
        .expect("show icon");
    surface
        .show_notification("Metis", "Tray notification test")
        .expect("notify");
    surface.poll_events().expect("pump");
    assert!(
        surface
            .take_tray_events()
            .iter()
            .all(|event| matches!(event, TrayEvent::NotificationClicked)),
        "no activation without user input"
    );
    assert!(surface.remove_tray_icon().expect("remove"));
    surface.close().expect("close");
}

#[test]
fn popup_menus_are_validated_before_the_menu_loop() {
    let config = WindowConfig::with_visibility("Metis menu", 320, 240, WindowVisibility::Hidden)
        .expect("bounded native configuration");
    let mut surface = NativeSurface::new(&config).expect("native surface");
    assert!(PopupMenu::new(vec![PopupMenuItem::separator()]).is_err());
    let menu = PopupMenu::new(vec![
        PopupMenuItem::action("Show window", true).expect("item"),
        PopupMenuItem::separator(),
        PopupMenuItem::action("Quit", true).expect("item"),
    ])
    .expect("menu");
    assert_eq!(menu.len(), 3);
    surface.close().expect("close");
    assert!(surface.show_popup_menu(&menu, 0, 0).is_err());
}
