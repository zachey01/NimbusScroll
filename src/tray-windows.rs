use crate::tray::UiHandles;
use std::error::Error;

pub(crate) fn start(ui: UiHandles) -> Result<(), Box<dyn Error>> {
    std::thread::Builder::new()
        .name("tray-windows".into())
        .spawn(move || {
            use tray_icon::menu::{Menu, MenuEvent, MenuItem};
            use tray_icon::TrayIconBuilder;

            let menu = Menu::new();
            let settings_item = MenuItem::new("Settings", true, None);
            let about_item = MenuItem::new("About", true, None);
            let exit_item = MenuItem::new("Exit", true, None);

            let _ = menu.append_items(&[&settings_item, &about_item, &exit_item]);

            let tray = TrayIconBuilder::new()
                .with_tooltip("NimbusScroll")
                .with_menu(Box::new(menu))
                .build();

            let Ok(_tray) = tray else {
                return;
            };

            let receiver = MenuEvent::receiver();
            while let Ok(event) = receiver.recv() {
                if event.id == settings_item.id() {
                    ui.show_settings();
                } else if event.id == about_item.id() {
                    ui.show_about();
                } else if event.id == exit_item.id() {
                    crate::engine::request_exit();
                    let _ = slint::quit_event_loop();
                    break;
                }
            }
        })?;

    Ok(())
}
