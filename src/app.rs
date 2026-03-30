use crate::engine::{
    self, InputEvent, MouseDeviceInfo, OutputEvent, ScrollConfig, ScrollController,
};
use crate::tray::{AboutWindow, SettingsWindow, UiHandles};
use slint::ComponentHandle;
use std::error::Error;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use slint::{ModelRc, SharedString, VecModel};

pub(crate) trait Backend {
    type Mouse;
    type Keyboard;
    type Output;

    fn list_mouse_devices() -> Result<Vec<MouseDeviceInfo>, Box<dyn Error>>;
    fn default_mouse_path() -> Result<Option<String>, Box<dyn Error>>;

    fn open_mouse_device(path: Option<&str>) -> Result<Self::Mouse, Box<dyn Error>>;
    fn set_mouse_nonblocking(mouse: &mut Self::Mouse) -> Result<(), Box<dyn Error>>;
    fn grab_mouse(mouse: &mut Self::Mouse) -> Result<(), Box<dyn Error>>;

    fn open_keyboard_devices() -> Result<Vec<Self::Keyboard>, Box<dyn Error>>;
    fn set_keyboard_nonblocking(kb: &mut Self::Keyboard) -> Result<(), Box<dyn Error>>;

    fn poll_mouse_events(mouse: &mut Self::Mouse) -> Result<Vec<InputEvent>, Box<dyn Error>>;
    fn poll_keyboard_events(kb: &mut Self::Keyboard) -> Result<Vec<InputEvent>, Box<dyn Error>>;

    fn build_virtual_mouse() -> Result<Self::Output, Box<dyn Error>>;
    fn reset_virtual_mouse_buttons(out: &mut Self::Output) -> Result<(), Box<dyn Error>>;
    fn emit_output(out: &mut Self::Output, event: OutputEvent) -> Result<(), Box<dyn Error>>;

    fn active_window_signature() -> Option<String>;
    fn sleep(duration: Duration);
}

#[cfg(target_os = "linux")]
type ActiveBackend = crate::wayland::WaylandBackend;

#[cfg(target_os = "windows")]
type ActiveBackend = crate::windows::WindowsBackend;

pub(crate) fn run() -> Result<(), Box<dyn Error>> {
    let config = Arc::new(ScrollConfig::new());
    engine::init_config(config.clone());

    let settings = SettingsWindow::new()?;
    let about = AboutWindow::new()?;

    #[cfg(target_os = "linux")]
    {
        let _ = slint::set_xdg_app_id("org.qwaq.NimbusScroll");
    }

    settings
        .window()
        .on_close_requested(|| slint::CloseRequestResponse::HideWindow);
    about
        .window()
        .on_close_requested(|| slint::CloseRequestResponse::HideWindow);

    crate::tray::sync_settings(&settings, &config);
    about.set_version(env!("CARGO_PKG_VERSION").into());

    let mouse_devices = ActiveBackend::list_mouse_devices().unwrap_or_default();
    let default_path = config
        .mouse_device_path()
        .or_else(|| ActiveBackend::default_mouse_path().ok().flatten());
    let selected_path = default_path.or_else(|| mouse_devices.first().map(|d| d.path.clone()));

    let selected_label = selected_path
        .as_ref()
        .and_then(|path| {
            mouse_devices
                .iter()
                .find(|d| d.path == *path)
                .map(|d| d.label.clone())
        })
        .or_else(|| mouse_devices.first().map(|d| d.label.clone()))
        .unwrap_or_default();

    settings.set_show_mouse_selector(!mouse_devices.is_empty());

    #[cfg(target_os = "linux")]
    {
        let mouse_labels: Vec<SharedString> = mouse_devices
            .iter()
            .map(|d| SharedString::from(d.label.as_str()))
            .collect();
        let mouse_model = Rc::new(VecModel::from(mouse_labels));
        settings.set_mouse_devices(ModelRc::from(mouse_model));
        settings.set_selected_mouse(selected_label.clone().into());
    }

    if let Some(path) = selected_path.clone() {
        config.set_mouse_device_path(Some(path));
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
        settings.on_middle_scroll_enabled_changed(move |v| cfg.set_middle_scroll_enabled(v));
    }

    {
        let cfg_mouse = config.clone();
        let cfg_reset = config.clone();
        let weak = settings.as_weak();
        let default_mouse_label = selected_label.clone();
        let default_mouse_path = selected_path.clone();
        let mouse_devices_for_select = mouse_devices.clone();

        settings.on_mouse_device_selected(move |label| {
            if let Some(device) = mouse_devices_for_select
                .iter()
                .find(|d| label.as_str() == d.label)
            {
                cfg_mouse.set_mouse_device_path(Some(device.path.clone()));
            }
        });

        settings.on_reset_defaults(move || {
            cfg_reset.reset_defaults();

            if let Some(path) = default_mouse_path.clone() {
                cfg_reset.set_mouse_device_path(Some(path));
            } else {
                cfg_reset.set_mouse_device_path(None);
            }

            let cfg = cfg_reset.clone();
            let default_mouse_label = default_mouse_label.clone();
            let _ = weak.upgrade_in_event_loop(move |win| {
                crate::tray::sync_settings(&win, &cfg);
                #[cfg(target_os = "linux")]
                win.set_selected_mouse(default_mouse_label.into());
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

    let _ = slint::run_event_loop_until_quit();

    engine::request_exit();

    #[cfg(target_os = "windows")]
    crate::windows::request_exit();

    let _ = engine_handle.join();
    Ok(())
}

fn spawn_engine() -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let _ = run_backend::<ActiveBackend>();
    })
}

fn run_backend<B: Backend>() -> Result<(), Box<dyn Error>> {
    let cfg = engine::config();

    let initial_mouse_path = cfg
        .mouse_device_path()
        .or_else(|| B::default_mouse_path().ok().flatten());

    let mut mouse = B::open_mouse_device(initial_mouse_path.as_deref())?;
    let _ = B::set_mouse_nonblocking(&mut mouse);
    let _ = B::grab_mouse(&mut mouse);

    if cfg.mouse_device_path().is_none() {
        if let Some(path) = initial_mouse_path.clone() {
            cfg.set_mouse_device_path(Some(path));
        }
    }

    let mut keyboards = B::open_keyboard_devices()?;
    for kb in &mut keyboards {
        let _ = B::set_keyboard_nonblocking(kb);
    }

    let mut virtual_mouse = B::build_virtual_mouse()?;
    let _ = B::reset_virtual_mouse_buttons(&mut virtual_mouse);

    let mut controller = ScrollController::new();
    let mut last_tick = Instant::now();
    let mut current_mouse_path = initial_mouse_path;
    let mut last_focus_signature = B::active_window_signature();

    loop {
        if engine::should_exit() {
            break;
        }

        let mut saw_raw_input = false;

        let current_focus = B::active_window_signature();
        if current_focus != last_focus_signature {
            last_focus_signature = current_focus;
            controller.clear_scroll_state();
        }

        if let Some(desired_path) = cfg.mouse_device_path() {
            if current_mouse_path.as_deref() != Some(desired_path.as_str()) {
                if let Ok(mut new_mouse) = B::open_mouse_device(Some(desired_path.as_str())) {
                    if B::set_mouse_nonblocking(&mut new_mouse).is_ok()
                        && B::grab_mouse(&mut new_mouse).is_ok()
                    {
                        mouse = new_mouse;
                        current_mouse_path = Some(desired_path);
                        controller.clear_scroll_state();
                        let _ = B::reset_virtual_mouse_buttons(&mut virtual_mouse);
                    }
                }
            }
        }

        for kb in keyboards.iter_mut() {
            let events = B::poll_keyboard_events(kb)?;
            for ev in events {
                saw_raw_input = true;
                let outputs = controller.handle_input(ev, cfg);
                emit_all::<B>(&mut virtual_mouse, outputs)?;
            }
        }

        let mouse_events = B::poll_mouse_events(&mut mouse)?;
        for ev in mouse_events {
            saw_raw_input = true;
            let outputs = controller.handle_input(ev, cfg);
            emit_all::<B>(&mut virtual_mouse, outputs)?;
        }

        let now = Instant::now();
        let dt = now.saturating_duration_since(last_tick);
        last_tick = now;

        let smooth_outputs = controller.advance(cfg, dt);
        let emitted_smooth = !smooth_outputs.is_empty();
        emit_all::<B>(&mut virtual_mouse, smooth_outputs)?;

        let sleep_ms = if saw_raw_input {
            1
        } else if emitted_smooth {
            cfg.loop_sleep_ms()
        } else {
            cfg.loop_sleep_ms().max(1)
        };

        B::sleep(Duration::from_millis(sleep_ms));
    }

    Ok(())
}

fn emit_all<B: Backend>(
    out: &mut B::Output,
    events: Vec<OutputEvent>,
) -> Result<(), Box<dyn Error>> {
    for event in events {
        B::emit_output(out, event)?;
    }
    Ok(())
}

fn open_url_async(url: &'static str) {
    std::thread::spawn(move || {
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("cmd")
                .args(["/C", "start", "", url])
                .spawn();
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let _ = std::process::Command::new("xdg-open").arg(url).spawn();
        }
    });
}
