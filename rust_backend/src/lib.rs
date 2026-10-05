//! Shared backend for the Thermal Control Center GUI and CLI.

pub mod autorun;
pub mod colors;
pub mod gui;
pub mod hardware;
pub mod logging;
pub mod settings;
pub mod thermal;
pub mod trayicon;
pub mod win32;
pub mod wmi;

use std::path::PathBuf;

pub const APP_NAME: &str = "Thermal Control Center for Dell G15";
pub const APP_VERSION: &str = "1.6.5";
pub const APP_DESCRIPTION: &str =
    "This app is an open-source replacement for Alienware Control Center ";
pub const APP_URL: &str = "github.com/AlexIII/tcc-g15";

pub const ICON_FILE: &str = "icons/gaugeIcon.ico";

/// Directory the executable lives in.
///
/// Resources are resolved relative to the executable rather than the working
/// directory, so autostart (Task Scheduler) still finds the icons.
pub fn install_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn icon_path() -> PathBuf {
    install_dir().join(ICON_FILE)
}
