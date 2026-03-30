use crate::engine::{self, ScrollConfig};
use std::error::Error;
use std::sync::Arc;

slint::include_modules!();

#[cfg(target_os = "linux")]
use slint::{ModelRc, SharedString, VecModel};
#[cfg(target_os = "linux")]
use std::rc::Rc;

#[derive(Clone)]
pub(crate) struct UiHandles {
    settings: slint::Weak<SettingsWindow>,
    about: slint::Weak<AboutWindow>,
    config: Arc<ScrollConfig>,
}

impl UiHandles {
    pub fn show_settings(&self) {
        let cfg = self.config.clone();
        let _ = self.settings.upgrade_in_event_loop(move |win| {
            sync_settings(&win, &cfg);
            let _ = win.show();
        });
    }

    pub fn show_about(&self) {
        let _ = self.about.upgrade_in_event_loop(move |win| {
            let _ = win.show();
        });
    }
}

pub(crate) fn run() -> Result<(), Box<dyn Error>> {
    let config = Arc::new(ScrollConfig::new());
    engine::init_config(config.clone());

    let settings = SettingsWindow::new()?;
    let about = AboutWindow::new()?;

    #[cfg(target_os = "linux")]
    {
        if let Err(e) = slint::set_xdg_app_id("org.qwaq.NimbusScroll") {
            eprintln!("Failed to set XDG app id: {e}");
        }
    }

    settings
        .window()
        .on_close_requested(|| slint::CloseRequestResponse::HideWindow);
    about
        .window()
        .on_close_requested(|| slint::CloseRequestResponse::HideWindow);

    sync_settings(&settings, &config);
    about.set_version(env!("CARGO_PKG_VERSION").into());

    #[cfg(target_os = "linux")]
    let (mouse_devices, selected_mouse_label, selected_mouse_path) = {
        let devices = match crate::wayland::list_mouse_devices() {
            Ok(devices) => devices,
            Err(e) => {
                eprintln!("{e}");
                Vec::new()
            }
        };

        let default_path = crate::wayland::default_mouse_path().ok().flatten();
        let selected_path = default_path.or_else(|| devices.first().map(|d| d.path.clone()));

        let selected_label = selected_path
            .as_ref()
            .and_then(|path| {
                devices
                    .iter()
                    .find(|d| d.path == *path)
                    .map(|d| d.label.clone())
            })
            .or_else(|| devices.first().map(|d| d.label.clone()))
            .unwrap_or_default();

        (devices, selected_label, selected_path)
    };

    #[cfg(target_os = "linux")]
    {
        settings.set_show_mouse_selector(true);

        let mouse_labels: Vec<SharedString> = mouse_devices
            .iter()
            .map(|d| SharedString::from(d.label.as_str()))
            .collect();
        let mouse_model = Rc::new(VecModel::from(mouse_labels));
        settings.set_mouse_devices(ModelRc::from(mouse_model));
        settings.set_selected_mouse(selected_mouse_label.clone().into());

        if let Some(path) = selected_mouse_path.clone() {
            config.set_mouse_device_path(Some(path));
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        settings.set_show_mouse_selector(false);
    }

    {
        let cfg = config.clone();
        settings.on_normal_gain_changed(move |v| cfg.set_normal_wheel_gain(v as f64));
    }
    {
        let cfg = config.clone();
        settings.on_normal_damping_changed(move |v| cfg.set_normal_wheel_damping(v as f64));
    }
    {
        let cfg = config.clone();
        settings.on_drag_gain_changed(move |v| cfg.set_drag_wheel_gain(v as f64));
    }
    {
        let cfg = config.clone();
        settings.on_drag_damping_changed(move |v| cfg.set_drag_wheel_damping(v as f64));
    }
    {
        let cfg = config.clone();
        settings.on_drag_deadzone_px_changed(move |v| cfg.set_drag_deadzone_px(v as f64));
    }
    {
        let cfg = config.clone();
        settings.on_tap_max_duration_ms_changed(move |v| cfg.set_tap_max_duration_ms(v as f64));
    }
    {
        let cfg = config.clone();
        settings.on_loop_sleep_ms_changed(move |v| cfg.set_loop_sleep_ms(v as f64));
    }
    {
        let cfg = config.clone();
        settings.on_max_velocity_hires_changed(move |v| cfg.set_max_velocity_hires(v as f64));
    }
    {
        let cfg = config.clone();
        settings.on_smooth_enabled_changed(move |v| cfg.set_smooth_enabled(v));
    }
    {
        let cfg = config.clone();
        settings.on_middle_scroll_enabled_changed(move |v| {
            cfg.set_middle_scroll_enabled(v);
        });
    }

    #[cfg(target_os = "linux")]
    {
        let cfg = config.clone();
        let weak = settings.as_weak();
        let default_mouse_label = selected_mouse_label.clone();
        let default_mouse_path = selected_mouse_path.clone();

        settings.on_mouse_device_selected(move |label| {
            if let Some(device) = mouse_devices.iter().find(|d| label == d.label) {
                cfg.set_mouse_device_path(Some(device.path.clone()));
            }
        });

        let cfg = config.clone();
        settings.on_reset_defaults(move || {
            cfg.reset_defaults();

            if let Some(path) = default_mouse_path.clone() {
                cfg.set_mouse_device_path(Some(path));
            } else {
                cfg.set_mouse_device_path(None);
            }

            let cfg = cfg.clone();
            let default_mouse_label = default_mouse_label.clone();
            let _ = weak.upgrade_in_event_loop(move |win| {
                sync_settings(&win, &cfg);
                win.set_selected_mouse(default_mouse_label.into());
            });
        });
    }

    #[cfg(not(target_os = "linux"))]
    {
        let cfg = config.clone();
        let weak = settings.as_weak();
        settings.on_reset_defaults(move || {
            cfg.reset_defaults();
            let cfg = cfg.clone();
            let _ = weak.upgrade_in_event_loop(move |win| {
                sync_settings(&win, &cfg);
            });
        });
    }

    {
        let weak = about.as_weak();
        settings.on_open_about(move || {
            let _ = weak.upgrade_in_event_loop(move |win| {
                let _ = win.show();
            });
        });
    }

    {
        let weak = settings.as_weak();
        settings.on_request_close(move || {
            let _ = weak.upgrade_in_event_loop(move |win| {
                let _ = win.hide();
            });
        });
    }

    {
        let weak = about.as_weak();
        about.on_request_close(move || {
            let _ = weak.upgrade_in_event_loop(move |win| {
                let _ = win.hide();
            });
        });
    }

    {
        about.on_open_github(move || {
            open_url_async("https://github.com/zachey01/NimbusScroll");
        });
    }

    {
        about.on_open_telegram(move || {
            open_url_async("https://t.me/qwaqdevv");
        });
    }

    let ui = UiHandles {
        settings: settings.as_weak(),
        about: about.as_weak(),
        config: config.clone(),
    };

    crate::tray::start(ui.clone())?;

    let engine_handle = spawn_engine();

    slint::run_event_loop_until_quit()?;

    engine::request_exit();
    #[cfg(target_os = "windows")]
    crate::windows::request_exit();

    let _ = engine_handle.join();
    Ok(())
}

fn spawn_engine() -> std::thread::JoinHandle<()> {
    #[cfg(target_os = "windows")]
    {
        crate::windows::spawn()
    }
    #[cfg(target_os = "linux")]
    {
        crate::wayland::spawn()
    }
}

fn sync_settings(win: &SettingsWindow, cfg: &ScrollConfig) {
    win.set_normal_gain(cfg.normal_wheel_gain() as f32);
    win.set_normal_damping(cfg.normal_wheel_damping() as f32);
    win.set_drag_gain(cfg.drag_wheel_gain() as f32);
    win.set_drag_damping(cfg.drag_wheel_damping() as f32);
    win.set_drag_deadzone_px(cfg.drag_deadzone_px() as f32);
    win.set_tap_max_duration_ms(cfg.tap_max_duration_ms() as f32);
    win.set_loop_sleep_ms(cfg.loop_sleep_ms() as f32);
    win.set_max_velocity_hires(cfg.max_velocity_hires() as f32);
    win.set_smooth_enabled(cfg.smooth_enabled());
    win.set_middle_scroll_enabled(cfg.middle_scroll_enabled());
}

fn open_url_async(url: &'static str) {
    std::thread::spawn(move || {
        #[cfg(target_os = "windows")]
        {
            let result = std::process::Command::new("cmd")
                .args(["/C", "start", "", url])
                .spawn();

            if let Err(e) = result {
                eprintln!("Failed to open URL {url}: {e}");
            }
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let result = std::process::Command::new("xdg-open").arg(url).spawn();

            if let Err(e) = result {
                eprintln!("Failed to open URL {url}: {e}");
            }
        }
    });
}
