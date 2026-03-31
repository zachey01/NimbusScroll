#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]
#![allow(non_snake_case, non_camel_case_types, dead_code)]

mod app;
mod easing;
mod engine;
mod tray;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
mod wayland;

#[cfg(target_os = "windows")]
#[path = "tray-windows.rs"]
mod tray_windows;

#[cfg(target_os = "linux")]
#[path = "tray-wayland.rs"]
mod tray_wayland;

fn main() {
    let _ = app::run();
}
