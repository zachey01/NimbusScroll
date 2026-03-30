use crate::tray::UiHandles;
use ksni::menu::StandardItem;
use ksni::TrayMethods;
use std::error::Error;

#[derive(Clone)]
struct NimbusTray {
    ui: UiHandles,
}

impl ksni::Tray for NimbusTray {
    fn id(&self) -> String {
        "NimbusScroll".into()
    }

    fn icon_name(&self) -> String {
        "input-mouse".into()
    }

    fn title(&self) -> String {
        "NimbusScroll".into()
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        let settings = self.ui.clone();
        let about = self.ui.clone();

        vec![
            StandardItem {
                label: "Settings".into(),
                icon_name: "preferences-system".into(),
                activate: Box::new(move |_| {
                    settings.show_settings();
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "About".into(),
                icon_name: "help-about".into(),
                activate: Box::new(move |_| {
                    about.show_about();
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Exit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(move |_| {
                    crate::engine::request_exit();
                    let _ = slint::quit_event_loop();
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub(crate) fn start(ui: UiHandles) -> Result<(), Box<dyn Error>> {
    std::thread::Builder::new()
        .name("tray-wayland".into())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(_) => return,
            };

            rt.block_on(async move {
                let tray = NimbusTray { ui };
                let handle = match tray.assume_sni_available(true).spawn().await {
                    Ok(handle) => handle,
                    Err(_) => return,
                };

                let _keep_alive = handle;
                std::future::pending::<()>().await;
            });
        })?;

    Ok(())
}
