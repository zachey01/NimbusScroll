use crate::engine::ScrollConfig;
use slint::ComponentHandle;
use std::error::Error;
use std::sync::Arc;

slint::include_modules!();

#[derive(Clone)]
pub(crate) struct UiHandles {
    pub(crate) settings: slint::Weak<SettingsWindow>,
    pub(crate) about: slint::Weak<AboutWindow>,
    pub(crate) config: Arc<ScrollConfig>,
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

pub(crate) fn sync_settings(win: &SettingsWindow, cfg: &ScrollConfig) {
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

pub(crate) fn start(ui: UiHandles) -> Result<(), Box<dyn Error>> {
    #[cfg(target_os = "linux")]
    {
        return crate::tray_wayland::start(ui);
    }

    #[cfg(target_os = "windows")]
    {
        return crate::tray_windows::start(ui);
    }

    #[allow(unreachable_code)]
    Ok(())
}
