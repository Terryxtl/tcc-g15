//! Settings persistence.
//!
//! The Python app used `QSettings`, which stores an INI file under
//! `%APPDATA%\<organization>\<application>.ini`. We keep the same keys in an
//! INI file of our own, so settings stay plain-text and easy to inspect.

use std::fmt::Write as _;
use std::path::PathBuf;

use crate::thermal::ThermalMode;
use crate::trayicon::TrayIconMode;

pub const FAN_SPEED_MIN: i32 = 0;
pub const FAN_SPEED_MAX: i32 = 120;
pub const FAN_SPEED_DEFAULT: i32 = (FAN_SPEED_MIN + FAN_SPEED_MAX) / 2;

pub const CPU_TEMP_MIN: i32 = 50;
pub const CPU_TEMP_MAX: i32 = 100;
pub const CPU_TEMP_DEFAULT: i32 = 95;

pub const GPU_TEMP_MIN: i32 = 50;
pub const GPU_TEMP_MAX: i32 = 90;
pub const GPU_TEMP_DEFAULT: i32 = 85;

/// The stored values, without any knowledge of where they live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsValues {
    pub mode: ThermalMode,
    pub cpu_fan_speed: i32,
    pub gpu_fan_speed: i32,
    pub cpu_threshold_temp: i32,
    pub gpu_threshold_temp: i32,
    pub failsafe: bool,
    pub tray_icon_mode: TrayIconMode,
    /// `None` until the user answers the "exit or minimize?" prompt.
    pub minimize_on_close: Option<bool>,
}

impl Default for SettingsValues {
    fn default() -> Self {
        SettingsValues {
            mode: ThermalMode::Balanced,
            cpu_fan_speed: FAN_SPEED_DEFAULT,
            gpu_fan_speed: FAN_SPEED_DEFAULT,
            cpu_threshold_temp: CPU_TEMP_DEFAULT,
            gpu_threshold_temp: GPU_TEMP_DEFAULT,
            failsafe: true,
            tray_icon_mode: TrayIconMode::Both,
            // Resident background behaviour: closing the window hides it to
            // the tray. Users can still exit from the tray menu.
            minimize_on_close: Some(true),
        }
    }
}

/// Stored values plus the file they are kept in.
#[derive(Debug, Clone)]
pub struct Settings {
    pub values: SettingsValues,
    path: PathBuf,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            values: SettingsValues::default(),
            path: settings_path(),
        }
    }
}

fn settings_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("tcc-g15").join("settings.ini")
}

fn clamp(value: i32, min: i32, max: i32) -> i32 {
    value.max(min).min(max)
}

impl Settings {
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn load() -> Settings {
        let mut settings = Settings::default();
        let Ok(text) = std::fs::read_to_string(&settings.path) else {
            return settings;
        };
        let values = &mut settings.values;

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim();
            match key {
                "mode" => {
                    values.mode = ThermalMode::from_key(value).unwrap_or(ThermalMode::Balanced)
                }
                "cpu_fan_speed" => {
                    values.cpu_fan_speed = clamp(
                        parse_or(value, FAN_SPEED_DEFAULT),
                        FAN_SPEED_MIN,
                        FAN_SPEED_MAX,
                    )
                }
                "gpu_fan_speed" => {
                    values.gpu_fan_speed = clamp(
                        parse_or(value, FAN_SPEED_DEFAULT),
                        FAN_SPEED_MIN,
                        FAN_SPEED_MAX,
                    )
                }
                "cpu_threshold_temp" => {
                    values.cpu_threshold_temp = clamp(
                        parse_or(value, CPU_TEMP_DEFAULT),
                        CPU_TEMP_MIN,
                        CPU_TEMP_MAX,
                    )
                }
                "gpu_threshold_temp" => {
                    values.gpu_threshold_temp = clamp(
                        parse_or(value, GPU_TEMP_DEFAULT),
                        GPU_TEMP_MIN,
                        GPU_TEMP_MAX,
                    )
                }
                "failsafe" => values.failsafe = parse_bool(value, true),
                "tray_icon_mode" => {
                    values.tray_icon_mode =
                        TrayIconMode::from_key(value).unwrap_or(TrayIconMode::Both)
                }
                "minimize_on_close" => values.minimize_on_close = Some(parse_bool(value, false)),
                _ => {}
            }
        }
        settings
    }

    pub fn save(&self) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut text = String::new();
        let values = &self.values;
        let _ = writeln!(text, "# Thermal Control Center for Dell G15 - settings");
        let _ = writeln!(text, "mode={}", values.mode.key());
        let _ = writeln!(text, "cpu_fan_speed={}", values.cpu_fan_speed);
        let _ = writeln!(text, "gpu_fan_speed={}", values.gpu_fan_speed);
        let _ = writeln!(text, "cpu_threshold_temp={}", values.cpu_threshold_temp);
        let _ = writeln!(text, "gpu_threshold_temp={}", values.gpu_threshold_temp);
        let _ = writeln!(
            text,
            "failsafe={}",
            if values.failsafe { "true" } else { "false" }
        );
        let _ = writeln!(text, "tray_icon_mode={}", values.tray_icon_mode.key());
        if let Some(minimize) = values.minimize_on_close {
            let _ = writeln!(
                text,
                "minimize_on_close={}",
                if minimize { "true" } else { "false" }
            );
        }
        std::fs::write(&self.path, text)
    }

    /// Removes every stored value (`QSettings::clear`).
    pub fn clear(&self) -> std::io::Result<()> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

fn parse_or(value: &str, fallback: i32) -> i32 {
    value.parse::<i32>().unwrap_or(fallback)
}

fn parse_bool(value: &str, fallback: bool) -> bool {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => true,
        "false" | "0" | "no" => false,
        _ => fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_resident_app() {
        let settings = SettingsValues::default();
        assert_eq!(settings.mode, ThermalMode::Balanced);
        assert_eq!(settings.cpu_fan_speed, 60);
        assert_eq!(settings.gpu_fan_speed, 60);
        assert_eq!(settings.cpu_threshold_temp, 95);
        assert_eq!(settings.gpu_threshold_temp, 85);
        assert!(settings.failsafe);
        assert_eq!(settings.tray_icon_mode, TrayIconMode::Both);
        assert_eq!(settings.minimize_on_close, Some(true));
    }

    #[test]
    fn unknown_mode_falls_back_to_balanced() {
        assert_eq!(ThermalMode::from_key("Turbo"), None);
        assert_eq!(ThermalMode::Balanced.key(), "Balanced");
    }

    #[test]
    fn threshold_values_are_clamped_to_the_combo_ranges() {
        assert_eq!(clamp(200, CPU_TEMP_MIN, CPU_TEMP_MAX), 100);
        assert_eq!(clamp(10, GPU_TEMP_MIN, GPU_TEMP_MAX), 50);
    }
}
