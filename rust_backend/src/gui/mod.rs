//! The application window. Port of `GUI/AppGUI.py`.
//!
//! Window and GDI handles are opaque values, so the helpers below stay safe
//! functions even though they pass pointers around.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::ptr::{null, null_mut};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crate::autorun::{self, AutorunAction};
use crate::colors::{self, Rgb};
use crate::gui::widgets::{Dialog, TooltipStrings};
use crate::hardware::{DetectHardware, CPU_FAN_INDEX, GPU_FAN_INDEX};
use crate::settings::{
    Settings, SettingsValues, CPU_TEMP_MAX, CPU_TEMP_MIN, FAN_SPEED_MAX, FAN_SPEED_MIN,
    GPU_TEMP_MAX, GPU_TEMP_MIN,
};
use crate::thermal::{Thermal, ThermalMode};
use crate::trayicon::{self, TrayIconMode};
use crate::win32::*;
use crate::{logging, logln};
use crate::{APP_DESCRIPTION, APP_NAME, APP_URL, APP_VERSION};

pub mod widgets;

pub const WINDOW_CLASS_NAME: &str = "TCC_G15_MainWindow";

const TEMP_UPDATE_PERIOD_MS: u32 = 1000;
const SLIDER_DEBOUNCE_MS: u32 = 500;
const FAILSAFE_TRIGGER_DELAY: Duration = Duration::from_secs(8);
const FAILSAFE_RESET_AFTER: Duration = Duration::from_secs(60);

/// Key the G-mode hotkey uses, as in the Python app.
const G_MODE_KEY: u32 = 0x80;

const TIMER_UPDATE: usize = 1;
const TIMER_SLIDER_DEBOUNCE: usize = 2;

/// Posted once the hardware-name worker thread has results.
const WM_NAMES_READY: UINT = WM_APP + 12;
/// Posted by `App::fatal`; the window is destroyed once construction finished.
const WM_FATAL: UINT = WM_APP + 13;
/// Posted by a second instance to bring the existing hidden window forward.
const WM_SHOW_REQUEST: UINT = WM_APP + 14;
/// Posted by the installer (`--quit`) so the resident process exits cleanly.
const WM_FORCE_EXIT: UINT = WM_APP + 15;

fn taskbar_created_message() -> UINT {
    static MESSAGE: OnceLock<UINT> = OnceLock::new();
    *MESSAGE.get_or_init(|| unsafe { RegisterWindowMessageW(to_wide("TaskbarCreated").as_ptr()) })
}

// Child control identifiers.
const ID_GAUGE_TEMP_GPU: i32 = 100;
const ID_LABEL_TEMP_GPU: i32 = 101;
const ID_GAUGE_FAN_GPU: i32 = 102;
const ID_LABEL_FAN_GPU: i32 = 103;
const ID_SLIDER_GPU: i32 = 104;
const ID_LABEL_SLIDER_GPU: i32 = 105;
const ID_NAME_GPU: i32 = 106;
const ID_NAME_TEXT_GPU: i32 = 107;
const ID_GAUGE_TEMP_CPU: i32 = 120;
const ID_LABEL_TEMP_CPU: i32 = 121;
const ID_GAUGE_FAN_CPU: i32 = 122;
const ID_LABEL_FAN_CPU: i32 = 123;
const ID_SLIDER_CPU: i32 = 124;
const ID_LABEL_SLIDER_CPU: i32 = 125;
const ID_NAME_CPU: i32 = 126;
const ID_NAME_TEXT_CPU: i32 = 127;
const ID_RADIO_BALANCED: i32 = 140;
const ID_RADIO_GMODE: i32 = 141;
const ID_RADIO_CUSTOM: i32 = 142;
const ID_FAILSAFE_CHECKBOX: i32 = 150;
const ID_LIMIT_GPU: i32 = 151;
const ID_LIMIT_CPU: i32 = 152;
const ID_INDICATOR: i32 = 153;

// Tray menu commands.
const CMD_MODE_FIRST: usize = 2000;
const CMD_SHOW: usize = 2010;
const CMD_AUTORUN_ENABLE: usize = 2011;
const CMD_AUTORUN_DISABLE: usize = 2012;
const CMD_RESTORE_DEFAULTS: usize = 2013;
const CMD_EXIT: usize = 2014;
const CMD_HIDE: usize = 2015;
const CMD_TRAY_ICON: usize = 2016;

const TRAY_ICON_ID: u32 = 1;

/// Colour transitions and ranges, from the Python app.
const GPU_COLOR_LIMITS: (u32, u32) = (72, 85);
const CPU_COLOR_LIMITS: (u32, u32) = (85, 95);
const GPU_TEMP_RANGE: (i32, i32) = (0, 95);
const CPU_TEMP_RANGE: (i32, i32) = (0, 110);
const FAN_RANGE: (i32, i32) = (0, 5500);
const SLIDER_TICK_INTERVAL: i32 = 20;

const SS_OWNERDRAW: u32 = 0x0000_000B;
const ES_READONLY: u32 = 0x0000_0800;
const ES_AUTOHSCROLL: u32 = 0x0000_0080;

/// Passed to the window procedure through `CreateWindowExW`.
pub struct InitParams {
    pub thermal: Thermal,
}

/// One thermal unit: name, temperature bar, fan bar and fan-speed slider.
struct ThermalUnit {
    name_static: HWND,
    name_text: HWND,
    gauge_temp: HWND,
    label_temp: HWND,
    gauge_fan: HWND,
    label_fan: HWND,
    slider: HWND,
    slider_label: HWND,
    last_temp: Option<u32>,
    last_rpm: Option<u32>,
}

impl ThermalUnit {
    /// Short names are centred in a static, long ones go into a read-only edit
    /// so they can be selected and copied.
    fn set_title(&self, title: &str) {
        let use_text_control = title.chars().count() >= 10;
        let target = if use_text_control {
            self.name_text
        } else {
            self.name_static
        };
        let other = if use_text_control {
            self.name_static
        } else {
            self.name_text
        };
        let wide = to_wide(title);
        unsafe {
            SetWindowTextW(target, wide.as_ptr());
            ShowWindow(other, SW_HIDE);
            ShowWindow(target, SW_SHOW);
        }
    }

    fn set_temp(&mut self, value: u32) {
        if self.last_temp == Some(value) {
            return;
        }
        self.last_temp = Some(value);
        widgets::gauge_set_value(self.gauge_temp, value as i32);
        let text = to_wide(&format!("{value} °C"));
        unsafe { SetWindowTextW(self.label_temp, text.as_ptr()) };
    }

    fn set_fan_rpm(&mut self, value: u32) {
        if self.last_rpm == Some(value) {
            return;
        }
        self.last_rpm = Some(value);
        widgets::gauge_set_value(self.gauge_fan, value as i32);
        let text = to_wide(&format!("{value} RPM"));
        unsafe { SetWindowTextW(self.label_fan, text.as_ptr()) };
    }

    fn speed(&self) -> i32 {
        widgets::slider_get(self.slider)
    }

    fn set_speed(&self, value: i32) {
        widgets::slider_set(self.slider, value);
    }

    fn set_speed_enabled(&self, enabled: bool) {
        unsafe { EnableWindow(self.slider, enabled as BOOL) };
    }
}

/// Fail-safe decisions, mirroring `updateAppState()` from the Python app.
struct Failsafe {
    enabled: bool,
    gpu_limit: u32,
    cpu_limit: u32,
    high_since: Option<Instant>,
    cool_since: Option<Instant>,
    last_high: Option<(Instant, SYSTEMTIME)>,
    tripped_previous: Option<ThermalMode>,
}

enum FailsafeEvent {
    Tripped,
    Reset(ThermalMode),
}

impl Failsafe {
    fn tick(
        &mut self,
        now: Instant,
        mode: ThermalMode,
        gpu_temp: Option<u32>,
        cpu_temp: Option<u32>,
    ) -> Option<FailsafeEvent> {
        // A missing reading counts as "too hot", exactly like the Python app.
        let is_high = gpu_temp.map_or(true, |temp| temp >= self.gpu_limit)
            || cpu_temp.map_or(true, |temp| temp >= self.cpu_limit);

        if is_high {
            self.cool_since = None;
            let mut timestamp = SYSTEMTIME::default();
            unsafe { GetLocalTime(&mut timestamp) };
            self.last_high = Some((now, timestamp));
            self.high_since.get_or_insert(now);
        } else {
            self.high_since = None;
        }

        if self.enabled && mode != ThermalMode::GMode && is_high {
            if let Some(since) = self.high_since {
                if now.duration_since(since) > FAILSAFE_TRIGGER_DELAY {
                    self.tripped_previous = Some(mode);
                    return Some(FailsafeEvent::Tripped);
                }
            }
        }

        if let Some(previous) = self.tripped_previous {
            if mode == ThermalMode::GMode && !is_high {
                let cool_since = *self.cool_since.get_or_insert(now);
                if now.duration_since(cool_since) >= FAILSAFE_RESET_AFTER {
                    // Keep the previous selection until restoring it succeeds.
                    // A failed restore must not persist automatic G-mode.
                    return Some(FailsafeEvent::Reset(previous));
                }
            } else {
                self.cool_since = None;
            }
        }
        None
    }

    fn selected_mode(&self, current: ThermalMode) -> ThermalMode {
        self.tripped_previous.unwrap_or(current)
    }

    fn clear_auto_mode(&mut self) {
        self.tripped_previous = None;
        self.cool_since = None;
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.high_since = None;
        if self.tripped_previous.is_none() {
            self.reset();
        }
    }

    fn reset(&mut self) {
        self.high_since = None;
        self.last_high = None;
        self.clear_auto_mode();
    }
}

/// Dedicated thread that owns the G-mode hotkey registration.
struct Hotkey {
    thread_id: u32,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Hotkey {
    fn start(target: HWND, key: u32) -> Option<Hotkey> {
        let (sender, receiver) = std::sync::mpsc::channel::<u32>();
        let target = target as isize;
        let handle = std::thread::spawn(move || unsafe {
            let target = target as HWND;
            let thread_id = GetCurrentThreadId();
            if RegisterHotKey(null_mut(), 1, 0, key) == 0 {
                let _ = sender.send(0);
                return;
            }
            let _ = sender.send(thread_id);
            let mut message: MSG = std::mem::zeroed();
            while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
                if message.message == WM_HOTKEY && message.wParam == 1 {
                    PostMessageW(target, WM_HOTKEY_TRIGGERED, 0, 0);
                }
            }
            UnregisterHotKey(null_mut(), 1);
        });
        match receiver.recv_timeout(Duration::from_secs(5)) {
            Ok(thread_id) if thread_id != 0 => Some(Hotkey {
                thread_id,
                handle: Some(handle),
            }),
            _ => {
                let _ = handle.join();
                None
            }
        }
    }
}

impl Drop for Hotkey {
    fn drop(&mut self) {
        const WM_QUIT: UINT = 0x0012;
        unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0) };
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Values shown by the tray icon and tooltip. Keeping the previous snapshot
/// lets the app avoid rebuilding the HICON and calling `Shell_NotifyIcon` when
/// nothing visible has changed.
#[derive(Clone, Copy, PartialEq, Eq)]
struct TrayState {
    icon_size: i32,
    gpu_temp: Option<u32>,
    cpu_temp: Option<u32>,
    gpu_rpm: Option<u32>,
    cpu_rpm: Option<u32>,
    mode: ThermalMode,
    tray_icon_mode: TrayIconMode,
}

pub struct App {
    hwnd: HWND,
    dpi: u32,
    fonts: widgets::Fonts,
    normal_font_controls: Vec<HWND>,
    value_font_controls: Vec<HWND>,
    thermal: Thermal,
    settings: Settings,
    saved: SettingsValues,
    gpu: ThermalUnit,
    cpu: ThermalUnit,
    radios: Vec<(ThermalMode, HWND)>,
    failsafe_checkbox: HWND,
    limit_gpu: HWND,
    limit_cpu: HWND,
    indicator: HWND,
    indicator_color: Rgb,
    last_indicator: Option<(Rgb, String)>,
    mode: ThermalMode,
    failsafe: Failsafe,
    g_mode_previous: Option<ThermalMode>,
    gpu_temp: Option<u32>,
    cpu_temp: Option<u32>,
    gpu_rpm: Option<u32>,
    cpu_rpm: Option<u32>,
    tray_menu: HMENU,
    last_tray_state: Option<TrayState>,
    tooltips: HWND,
    tooltip_strings: TooltipStrings,
    tooltip_failsafe: usize,
    tooltip_indicator: usize,
    /// Kept alive for the lifetime of the window (WM_SETICON does not copy it).
    _window_icon: HICON,
    hotkey: Option<Hotkey>,
    exit_code: i32,
    pending_error: Option<(String, String)>,
}

impl App {
    fn new(hwnd: HWND, params: InitParams) -> App {
        let instance = unsafe { GetModuleHandleW(null()) };
        let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
        let fonts = widgets::Fonts::new(dpi);
        let mut normal_font_controls = Vec::new();
        let mut value_font_controls = Vec::new();

        let gpu = Self::create_unit(
            hwnd,
            instance,
            UnitIds {
                name: ID_NAME_GPU,
                name_text: ID_NAME_TEXT_GPU,
                gauge_temp: ID_GAUGE_TEMP_GPU,
                label_temp: ID_LABEL_TEMP_GPU,
                gauge_fan: ID_GAUGE_FAN_GPU,
                label_fan: ID_LABEL_FAN_GPU,
                slider: ID_SLIDER_GPU,
                slider_label: ID_LABEL_SLIDER_GPU,
            },
            Some(GPU_COLOR_LIMITS),
            GPU_TEMP_RANGE,
            "GPU",
            &mut normal_font_controls,
            &mut value_font_controls,
        );
        let cpu = Self::create_unit(
            hwnd,
            instance,
            UnitIds {
                name: ID_NAME_CPU,
                name_text: ID_NAME_TEXT_CPU,
                gauge_temp: ID_GAUGE_TEMP_CPU,
                label_temp: ID_LABEL_TEMP_CPU,
                gauge_fan: ID_GAUGE_FAN_CPU,
                label_fan: ID_LABEL_FAN_CPU,
                slider: ID_SLIDER_CPU,
                slider_label: ID_LABEL_SLIDER_CPU,
            },
            Some(CPU_COLOR_LIMITS),
            CPU_TEMP_RANGE,
            "CPU",
            &mut normal_font_controls,
            &mut value_font_controls,
        );
        gpu.set_title("GPU");
        cpu.set_title("CPU");

        let mut radios = Vec::new();
        for (index, mode) in ThermalMode::ALL.into_iter().enumerate() {
            let id = match mode {
                ThermalMode::Balanced => ID_RADIO_BALANCED,
                ThermalMode::GMode => ID_RADIO_GMODE,
                ThermalMode::Custom => ID_RADIO_CUSTOM,
            };
            let style = BS_AUTORADIOBUTTON | WS_TABSTOP | if index == 0 { WS_GROUP } else { 0 };
            let radio = widgets::create_control(hwnd, "Button", mode.label(), style, id, instance);
            widgets::untheme(radio);
            normal_font_controls.push(radio);
            radios.push((mode, radio));
        }

        let failsafe_checkbox = widgets::create_control(
            hwnd,
            "Button",
            "Fail-safe",
            BS_AUTOCHECKBOX | WS_TABSTOP,
            ID_FAILSAFE_CHECKBOX,
            instance,
        );
        widgets::untheme(failsafe_checkbox);
        normal_font_controls.push(failsafe_checkbox);

        let limit_gpu = Self::create_threshold_combo(
            hwnd,
            instance,
            &fonts,
            ID_LIMIT_GPU,
            GPU_TEMP_MIN,
            GPU_TEMP_MAX,
        );
        let limit_cpu = Self::create_threshold_combo(
            hwnd,
            instance,
            &fonts,
            ID_LIMIT_CPU,
            CPU_TEMP_MIN,
            CPU_TEMP_MAX,
        );
        normal_font_controls.push(limit_gpu);
        normal_font_controls.push(limit_cpu);

        let indicator =
            widgets::create_control(hwnd, "Static", "", SS_OWNERDRAW, ID_INDICATOR, instance);

        let tooltips = widgets::create_tooltip(hwnd, instance);
        let mut tooltip_strings = TooltipStrings::default();
        let tooltip_failsafe = tooltip_strings.add(
            tooltips,
            failsafe_checkbox,
            "Switch to G-mode (fans on max) when the temperature reaches the threshold",
        );
        let tooltip_indicator = tooltip_strings.add(tooltips, indicator, "Normal");
        tooltip_strings.add(tooltips, limit_gpu, "Threshold GPU temp");
        tooltip_strings.add(tooltips, limit_cpu, "Threshold CPU temp");
        tooltip_strings.add(
            tooltips,
            gpu.name_text,
            "Triple left-click and Crtl+C to copy",
        );
        tooltip_strings.add(
            tooltips,
            cpu.name_text,
            "Triple left-click and Crtl+C to copy",
        );

        let window_icon = widgets::load_app_icon(0);
        if !window_icon.is_null() {
            unsafe {
                SendMessageW(hwnd, WM_SETICON, 0, window_icon as LPARAM);
                SendMessageW(hwnd, WM_SETICON, 1, window_icon as LPARAM);
            }
        }

        let settings = Settings::load();
        let mut app = App {
            hwnd,
            dpi,
            fonts,
            normal_font_controls,
            value_font_controls,
            thermal: params.thermal,
            settings,
            saved: SettingsValues::default(),
            gpu,
            cpu,
            radios,
            failsafe_checkbox,
            limit_gpu,
            limit_cpu,
            indicator,
            indicator_color: colors::GREEN,
            last_indicator: None,
            mode: ThermalMode::Balanced,
            failsafe: Failsafe {
                enabled: true,
                gpu_limit: 85,
                cpu_limit: 95,
                high_since: None,
                cool_since: None,
                last_high: None,
                tripped_previous: None,
            },
            g_mode_previous: None,
            gpu_temp: None,
            cpu_temp: None,
            gpu_rpm: None,
            cpu_rpm: None,
            tray_menu: null_mut(),
            last_tray_state: None,
            tooltips,
            tooltip_strings,
            tooltip_failsafe,
            tooltip_indicator,
            _window_icon: window_icon,
            hotkey: None,
            exit_code: 0,
            pending_error: None,
        };

        app.tray_menu = app.build_tray_menu();
        logln!("controls created");
        app.add_tray_icon();
        logln!("tray icon added");
        app.relayout();
        logln!("layout applied");
        app.apply_fonts();
        app.load_settings_into_ui();
        if app.exit_code != 0 {
            return app;
        }
        logln!("settings applied");
        app.hotkey = Hotkey::start(hwnd, G_MODE_KEY);
        if app.hotkey.is_none() {
            logln!("Could not register the G-mode hot key ({G_MODE_KEY:#x})");
        }
        unsafe { SetTimer(hwnd, TIMER_UPDATE, TEMP_UPDATE_PERIOD_MS, null()) };
        app.detect_hardware_names();
        logln!("startup complete");
        app.tick();
        app
    }

    #[allow(clippy::too_many_arguments)]
    fn create_unit(
        parent: HWND,
        instance: HINSTANCE,
        ids: UnitIds,
        temp_limits: Option<(u32, u32)>,
        temp_range: (i32, i32),
        title: &str,
        normal_font_controls: &mut Vec<HWND>,
        value_font_controls: &mut Vec<HWND>,
    ) -> ThermalUnit {
        let name_static = widgets::create_control(
            parent,
            "Static",
            title,
            SS_CENTER | SS_CENTERIMAGE | SS_NOPREFIX,
            ids.name,
            instance,
        );
        let name_text = widgets::create_control(
            parent,
            "Edit",
            title,
            WS_TABSTOP | ES_READONLY | ES_AUTOHSCROLL,
            ids.name_text,
            instance,
        );
        normal_font_controls.push(name_static);
        normal_font_controls.push(name_text);

        let gauge_temp = widgets::create_gauge(
            parent,
            ids.gauge_temp,
            temp_range.0,
            temp_range.1,
            colors::BLUE,
            temp_limits,
            instance,
        );
        let label_temp = widgets::create_control(
            parent,
            "Static",
            "",
            SS_LEFT | SS_CENTERIMAGE | SS_NOPREFIX,
            ids.label_temp,
            instance,
        );
        value_font_controls.push(label_temp);

        let gauge_fan = widgets::create_gauge(
            parent,
            ids.gauge_fan,
            FAN_RANGE.0,
            FAN_RANGE.1,
            colors::BLUE,
            None,
            instance,
        );
        let label_fan = widgets::create_control(
            parent,
            "Static",
            "",
            SS_LEFT | SS_CENTERIMAGE | SS_NOPREFIX,
            ids.label_fan,
            instance,
        );
        value_font_controls.push(label_fan);

        let slider = widgets::create_control(
            parent,
            "msctls_trackbar32",
            "",
            TBS_HORZ | TBS_AUTOTICKS | TBS_BOTTOM | WS_TABSTOP,
            ids.slider,
            instance,
        );
        widgets::slider_range(slider, FAN_SPEED_MIN, FAN_SPEED_MAX);
        widgets::slider_set_tick_freq(slider, SLIDER_TICK_INTERVAL);
        widgets::slider_set(slider, (FAN_SPEED_MIN + FAN_SPEED_MAX) / 2);
        widgets::untheme(slider);
        let slider_label = widgets::create_control(
            parent,
            "Static",
            "Fan Speed",
            SS_LEFT | SS_CENTERIMAGE | SS_NOPREFIX,
            ids.slider_label,
            instance,
        );
        normal_font_controls.push(slider_label);

        ThermalUnit {
            name_static,
            name_text,
            gauge_temp,
            label_temp,
            gauge_fan,
            label_fan,
            slider,
            slider_label,
            last_temp: None,
            last_rpm: None,
        }
    }

    fn create_threshold_combo(
        parent: HWND,
        instance: HINSTANCE,
        fonts: &widgets::Fonts,
        id: i32,
        min: i32,
        max: i32,
    ) -> HWND {
        let combo = widgets::create_control(
            parent,
            "ComboBox",
            "",
            CBS_DROPDOWNLIST | WS_TABSTOP | WS_VSCROLL,
            id,
            instance,
        );
        widgets::set_font(combo, fonts.normal);
        widgets::untheme(combo);
        let items: Vec<String> = (min..=max).map(|value| value.to_string()).collect();
        widgets::combo_add_items(combo, &items);
        combo
    }

    // ---------------------------------------------------------------- layout

    fn relayout(&mut self) {
        let px = |dip: i32| (dip as f32 * self.dpi as f32 / 96.0).round() as i32;
        let client_width = px(600);
        let client_height = px(172);
        let unit_width = px(260);
        let bar_width = px(200);
        let label_x = px(206);
        let label_width = unit_width - label_x;

        for (index, unit) in [&self.gpu, &self.cpu].into_iter().enumerate() {
            let x = px(20) + index as i32 * (unit_width + px(40));
            widgets::move_control(unit.name_static, x, px(10), unit_width, px(20));
            widgets::move_control(unit.name_text, x, px(10), unit_width, px(20));
            widgets::move_control(unit.gauge_temp, x, px(37), bar_width, px(22));
            widgets::move_control(unit.label_temp, x + label_x, px(37), label_width, px(22));
            widgets::move_control(unit.gauge_fan, x, px(66), bar_width, px(22));
            widgets::move_control(unit.label_fan, x + label_x, px(66), label_width, px(22));
            widgets::move_control(unit.slider, x, px(93), bar_width, px(28));
            widgets::move_control(unit.slider_label, x + label_x, px(99), label_width, px(20));
        }

        let row_y = px(136);
        let radio_width = px(92);
        for (index, (_, radio)) in self.radios.iter().enumerate() {
            widgets::move_control(
                *radio,
                px(20) + index as i32 * radio_width,
                row_y,
                radio_width,
                px(24),
            );
        }

        let checkbox_width = px(82);
        let combo_width = px(52);
        let indicator_size = px(16);
        let indicator_x = client_width - px(20) - indicator_size;
        let cpu_combo_x = indicator_x - px(10) - combo_width;
        let gpu_combo_x = cpu_combo_x - px(6) - combo_width;
        let checkbox_x = gpu_combo_x - px(6) - checkbox_width;
        widgets::move_control(
            self.indicator,
            indicator_x,
            row_y + px(3),
            indicator_size,
            indicator_size,
        );
        widgets::move_control(
            self.limit_cpu,
            cpu_combo_x,
            row_y + px(1),
            combo_width,
            px(180),
        );
        widgets::move_control(
            self.limit_gpu,
            gpu_combo_x,
            row_y + px(1),
            combo_width,
            px(180),
        );
        widgets::move_control(
            self.failsafe_checkbox,
            checkbox_x,
            row_y + px(2),
            checkbox_width,
            px(24),
        );

        let mut rect = RECT::new(0, 0, client_width, client_height);
        unsafe {
            let style = GetWindowLongPtrW(self.hwnd, GWL_STYLE) as u32;
            let ex_style = GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE) as u32;
            AdjustWindowRectEx(&mut rect, style, 0, ex_style);
            SetWindowPos(
                self.hwnd,
                null_mut(),
                0,
                0,
                rect.width(),
                rect.height(),
                widgets::SWP_NOZORDER | widgets::SWP_NOACTIVATE | widgets::SWP_NOMOVE,
            );
        }
    }

    fn apply_fonts(&self) {
        for control in &self.normal_font_controls {
            widgets::set_font(*control, self.fonts.normal);
        }
        for control in &self.value_font_controls {
            widgets::set_font(*control, self.fonts.value);
        }
    }

    // -------------------------------------------------------------- settings

    fn load_settings_into_ui(&mut self) {
        let values = self.settings.values.clone();
        self.gpu.set_speed(values.gpu_fan_speed);
        self.cpu.set_speed(values.cpu_fan_speed);
        widgets::combo_select_text(self.limit_gpu, &values.gpu_threshold_temp.to_string());
        widgets::combo_select_text(self.limit_cpu, &values.cpu_threshold_temp.to_string());
        widgets::set_checked(self.failsafe_checkbox, values.failsafe);
        self.failsafe.enabled = values.failsafe;
        self.failsafe.gpu_limit = values.gpu_threshold_temp as u32;
        self.failsafe.cpu_limit = values.cpu_threshold_temp as u32;
        self.failsafe.reset();
        self.update_failsafe_tooltip();
        self.update_indicator();
        self.saved = self.current_values();
        // Applies the stored mode to the hardware and updates the radio buttons.
        self.apply_mode(values.mode);
    }

    fn current_values(&self) -> SettingsValues {
        SettingsValues {
            mode: self.failsafe.selected_mode(self.mode),
            cpu_fan_speed: self.cpu.speed(),
            gpu_fan_speed: self.gpu.speed(),
            cpu_threshold_temp: widgets::combo_value(
                self.limit_cpu,
                self.settings.values.cpu_threshold_temp,
            ),
            gpu_threshold_temp: widgets::combo_value(
                self.limit_gpu,
                self.settings.values.gpu_threshold_temp,
            ),
            failsafe: self.failsafe.enabled,
            tray_icon_mode: self.settings.values.tray_icon_mode,
            minimize_on_close: self.settings.values.minimize_on_close,
        }
    }

    fn save_settings(&mut self) {
        let current = self.current_values();
        if current == self.saved {
            return;
        }
        self.saved = current.clone();
        self.settings.values = current;
        let _ = self.settings.save();
    }

    // ------------------------------------------------------------------ tray

    fn build_tray_menu(&self) -> HMENU {
        let menu = unsafe { CreatePopupMenu() };
        let append = |menu: HMENU, flags: u32, id: usize, text: &str| {
            let wide = to_wide(text);
            unsafe { AppendMenuW(menu, flags, id, wide.as_ptr()) };
        };
        append(menu, MF_STRING | MF_DISABLED | MF_GRAYED, 0, "Mode");
        for (index, mode) in ThermalMode::ALL.into_iter().enumerate() {
            append(
                menu,
                MF_STRING,
                CMD_MODE_FIRST + index,
                &self.menu_mode_text(mode),
            );
        }
        append(menu, MF_STRING | MF_DISABLED | MF_GRAYED, 0, "Settings");
        for (text, id) in [
            ("Show", CMD_SHOW),
            ("Minimize to tray", CMD_HIDE),
            ("Enable autorun", CMD_AUTORUN_ENABLE),
            ("Disable autorun", CMD_AUTORUN_DISABLE),
            ("Restore Default", CMD_RESTORE_DEFAULTS),
            ("Exit", CMD_EXIT),
        ] {
            append(menu, MF_STRING, id, text);
        }
        menu
    }

    fn menu_mode_text(&self, mode: ThermalMode) -> String {
        format!(
            "{} {}",
            if mode == self.mode { "\u{2022}" } else { " " },
            mode.label()
        )
    }

    fn menu_tray_icon_text(&self) -> String {
        format!("Tray icon: {}", self.settings.values.tray_icon_mode.label())
    }

    fn refresh_tray_menu(&self) {
        for (index, mode) in ThermalMode::ALL.into_iter().enumerate() {
            let text = to_wide(&self.menu_mode_text(mode));
            unsafe {
                ModifyMenuW(
                    self.tray_menu,
                    (CMD_MODE_FIRST + index) as UINT,
                    MF_STRING | MF_BYCOMMAND,
                    CMD_MODE_FIRST + index,
                    text.as_ptr(),
                )
            };
        }
        let text = to_wide(&self.menu_tray_icon_text());
        unsafe {
            ModifyMenuW(
                self.tray_menu,
                CMD_TRAY_ICON as UINT,
                MF_STRING | MF_BYCOMMAND,
                CMD_TRAY_ICON,
                text.as_ptr(),
            )
        };
    }

    fn cycle_tray_icon_mode(&mut self) {
        let current = self.settings.values.tray_icon_mode;
        let index = TrayIconMode::ALL
            .iter()
            .position(|mode| *mode == current)
            .unwrap_or(0);
        let next = TrayIconMode::ALL[(index + 1) % TrayIconMode::ALL.len()];
        self.settings.values.tray_icon_mode = next;
        self.saved.tray_icon_mode = next;
        let _ = self.settings.save();
        self.refresh_tray_menu();
        self.update_tray();
    }

    fn tray_data(&self, icon: HICON, tooltip: &str) -> NOTIFYICONDATAW {
        let mut data = NOTIFYICONDATAW {
            hWnd: self.hwnd,
            uID: TRAY_ICON_ID,
            ..Default::default()
        };
        data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        data.uCallbackMessage = WM_TRAYICON;
        data.hIcon = icon;
        copy_wide(&mut data.szTip, tooltip);
        data
    }

    fn add_tray_icon(&mut self) {
        self.last_tray_state = None;
        let icon = self.render_tray_icon(trayicon::best_size());
        let data = self.tray_data(icon, APP_NAME);
        if unsafe { Shell_NotifyIconW(NIM_ADD, &data) } == 0 {
            logln!("Could not add the tray icon");
        }
        if !icon.is_null() {
            unsafe { DestroyIcon(icon) };
        }
    }

    fn render_tray_icon(&self, size: i32) -> HICON {
        let spec = trayicon::spec_for(
            size,
            self.gpu_temp,
            self.cpu_temp,
            GPU_COLOR_LIMITS,
            CPU_COLOR_LIMITS,
            self.mode == ThermalMode::GMode,
            self.settings.values.tray_icon_mode,
        );
        match trayicon::draw(&spec).and_then(|bitmap| bitmap.to_hicon()) {
            Some(icon) => icon,
            None => null_mut(),
        }
    }

    fn update_tray(&mut self) {
        let icon_size = trayicon::best_size();
        let state = TrayState {
            icon_size,
            gpu_temp: self.gpu_temp,
            cpu_temp: self.cpu_temp,
            gpu_rpm: self.gpu_rpm,
            cpu_rpm: self.cpu_rpm,
            mode: self.mode,
            tray_icon_mode: self.settings.values.tray_icon_mode,
        };
        if self.last_tray_state == Some(state) {
            return;
        }
        let icon = self.render_tray_icon(icon_size);
        let tooltip = format!(
            "GPU:    {} °C    {} RPM\r\nCPU:    {} °C    {} RPM\r\nMode:    {}",
            reading(self.gpu_temp),
            reading(self.gpu_rpm),
            reading(self.cpu_temp),
            reading(self.cpu_rpm),
            self.mode.label()
        );
        let data = self.tray_data(icon, &tooltip);
        unsafe {
            // Explorer may have discarded our registration since the last tick.
            if Shell_NotifyIconW(NIM_MODIFY, &data) != 0 || Shell_NotifyIconW(NIM_ADD, &data) != 0 {
                self.last_tray_state = Some(state);
            }
            if !icon.is_null() {
                DestroyIcon(icon);
            }
        }
    }

    fn notify(&self, title: &str, lines: &[String], warning: bool) {
        let mut data = NOTIFYICONDATAW {
            hWnd: self.hwnd,
            uID: TRAY_ICON_ID,
            ..Default::default()
        };
        data.uFlags = NIF_INFO;
        data.dwInfoFlags = if warning { NIIF_WARNING } else { NIIF_INFO };
        copy_wide(&mut data.szInfoTitle, title);
        copy_wide(&mut data.szInfo, &lines.join("\r\n"));
        unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) };
    }

    fn notify_mode(&self, failsafe: bool) {
        let lines = vec![
            format!(
                "GPU: {}°C, CPU: {}°C",
                reading(self.gpu_temp),
                reading(self.cpu_temp)
            ),
            format!(
                "Thermal mode changed{}",
                if failsafe { " [Fail-safe]" } else { "" }
            ),
        ];
        self.notify(self.mode.label(), &lines, false);
    }

    // -------------------------------------------------------------- behavior

    fn apply_mode(&mut self, mode: ThermalMode) -> bool {
        if self.exit_code != 0 {
            return false;
        }
        let applied = self.thermal.set_mode(mode);
        logln!(
            "Set mode {}: {}",
            mode.label(),
            if applied { "ok" } else { "fail" }
        );
        if !applied {
            self.fatal(
                &format!("Failed to set mode {}", mode.label()),
                "Program is terminated",
            );
            return false;
        }
        self.mode = mode;
        for (candidate, radio) in &self.radios {
            widgets::set_checked(*radio, *candidate == mode);
        }
        self.refresh_tray_menu();
        self.gpu.set_speed_enabled(mode == ThermalMode::Custom);
        self.cpu.set_speed_enabled(mode == ThermalMode::Custom);
        self.update_indicator();
        self.update_fan_speeds();
        self.update_tray();
        true
    }

    fn update_fan_speeds(&mut self) {
        if self.exit_code != 0 || self.mode != ThermalMode::Custom {
            return;
        }
        for (index, speed) in [
            (GPU_FAN_INDEX, self.gpu.speed()),
            (CPU_FAN_INDEX, self.cpu.speed()),
        ] {
            let applied = self.thermal.set_fan_speed(index, speed as u32);
            logln!(
                "Set {} fan speed to {speed}: {}",
                if index == GPU_FAN_INDEX { "GPU" } else { "CPU" },
                if applied { "ok" } else { "fail" }
            );
        }
    }

    fn tick(&mut self) {
        if self.exit_code != 0 {
            return;
        }
        let gpu_temp = self.thermal.fan_related_temp(GPU_FAN_INDEX);
        let gpu_rpm = self.thermal.fan_rpm(GPU_FAN_INDEX);
        let cpu_temp = self.thermal.fan_related_temp(CPU_FAN_INDEX);
        let cpu_rpm = self.thermal.fan_rpm(CPU_FAN_INDEX);
        self.gpu_temp = gpu_temp;
        self.cpu_temp = cpu_temp;
        self.gpu_rpm = gpu_rpm;
        self.cpu_rpm = cpu_rpm;

        if self.is_window_visible() {
            self.refresh_gauges();
        }

        match self
            .failsafe
            .tick(Instant::now(), self.mode, gpu_temp, cpu_temp)
        {
            Some(FailsafeEvent::Tripped) => {
                logln!("Fail-safe tripped at GPU={gpu_temp:?} CPU={cpu_temp:?}");
                if self.apply_mode(ThermalMode::GMode) {
                    self.notify_mode(true);
                }
            }
            Some(FailsafeEvent::Reset(previous)) => {
                logln!("Fail-safe reset");
                if self.apply_mode(previous) {
                    self.failsafe.clear_auto_mode();
                    self.notify_mode(true);
                }
            }
            None => {}
        }

        if self.exit_code != 0 {
            return;
        }

        self.update_indicator();
        self.update_tray();
        self.save_settings();
    }

    fn update_failsafe_tooltip(&mut self) {
        let text = format!(
            "Switch to G-mode (fans on max) when GPU temp reaches {}°C or CPU reaches {}°C. Restore the previous mode after both stay below their thresholds for 60 seconds. Manual G-mode stays on.",
            self.failsafe.gpu_limit, self.failsafe.cpu_limit
        );
        let (tooltip, control) = (self.tooltips, self.failsafe_checkbox);
        self.tooltip_strings
            .update(self.tooltip_failsafe, tooltip, control, &text);
    }

    fn update_indicator(&mut self) {
        let color = if !self.failsafe.enabled {
            colors::DARK_GREY
        } else if self.failsafe.tripped_previous.is_some() {
            colors::RED
        } else if self.failsafe.last_high.is_some() {
            colors::YELLOW
        } else {
            colors::GREEN
        };
        let tooltip = match self.failsafe.last_high {
            Some((_, when)) => format!("Last high temp at {}", when.as_string()),
            None => "Normal".to_string(),
        };

        if let Some((last_color, last_tooltip)) = &self.last_indicator {
            if *last_color == color && last_tooltip == &tooltip {
                return;
            }
        }
        self.last_indicator = Some((color, tooltip.clone()));
        self.indicator_color = color;

        let (tooltips, indicator) = (self.tooltips, self.indicator);
        self.tooltip_strings
            .update(self.tooltip_indicator, tooltips, indicator, &tooltip);
        unsafe { InvalidateRect(indicator, null(), 0) };
    }

    fn is_window_visible(&self) -> bool {
        unsafe { IsWindowVisible(self.hwnd) != 0 }
    }

    /// Pushes the latest readings into the gauge controls. Called while the
    /// window is visible and again immediately when it is restored from tray.
    fn refresh_gauges(&mut self) {
        if let Some(value) = self.gpu_temp {
            self.gpu.set_temp(value);
        }
        if let Some(value) = self.gpu_rpm {
            self.gpu.set_fan_rpm(value);
        }
        if let Some(value) = self.cpu_temp {
            self.cpu.set_temp(value);
        }
        if let Some(value) = self.cpu_rpm {
            self.cpu.set_fan_rpm(value);
        }
    }

    fn draw_indicator(&self, hdc: HDC, rect: RECT) {
        unsafe {
            let pen = CreatePen(0, 1, colors::colorref(colors::BORDER));
            let old_pen = SelectObject(hdc, pen);
            let old_brush = SelectObject(hdc, widgets::brush_for(self.indicator_color));
            Ellipse(hdc, rect.left, rect.top, rect.right, rect.bottom);
            SelectObject(hdc, old_brush);
            SelectObject(hdc, old_pen);
            DeleteObject(pen);
        }
    }

    fn detect_hardware_names(&self) {
        let hwnd = self.hwnd as isize;
        std::thread::spawn(move || {
            let hwnd = hwnd as HWND;
            // COM must be initialised on this thread as well.
            let Ok(_com) = crate::wmi::Com::init(false) else {
                return;
            };
            let Some(detect) = DetectHardware::connect() else {
                return;
            };
            let gpu = detect.gpu_name();
            let cpu = detect.cpu_name();
            logln!("Detected GPU: {gpu:?}, CPU: {cpu:?}");
            let payload = Box::into_raw(Box::new((gpu, cpu)));
            unsafe { PostMessageW(hwnd, WM_NAMES_READY, 0, payload as LPARAM) };
        });
    }

    fn on_names_ready(&mut self, payload: LPARAM) {
        let names = unsafe { Box::from_raw(payload as *mut (Option<String>, Option<String>)) };
        if let Some(name) = &names.0 {
            self.gpu.set_title(name);
        }
        if let Some(name) = &names.1 {
            self.cpu.set_title(name);
        }
    }

    fn on_mode_radio(&mut self, id: i32) {
        let mode = match id {
            ID_RADIO_BALANCED => ThermalMode::Balanced,
            ID_RADIO_GMODE => ThermalMode::GMode,
            ID_RADIO_CUSTOM => ThermalMode::Custom,
            _ => return,
        };
        self.switch_mode(mode);
    }

    fn switch_mode(&mut self, mode: ThermalMode) {
        if self.exit_code != 0 {
            return;
        }
        if mode == ThermalMode::GMode {
            if let Some(previous) = self.failsafe.tripped_previous {
                self.g_mode_previous = Some(previous);
            } else if self.mode != ThermalMode::GMode {
                self.g_mode_previous = Some(self.mode);
            }
        }
        if mode == self.mode {
            // Selecting the active G-mode explicitly transfers it to the user.
            self.failsafe.clear_auto_mode();
            self.update_indicator();
            self.save_settings();
            return;
        }
        if self.apply_mode(mode) {
            self.failsafe.clear_auto_mode();
            self.update_indicator();
            self.save_settings();
        }
    }

    /// G-mode hotkey: toggle between G-mode and whatever came before.
    fn on_hotkey(&mut self) {
        let target = if self.mode == ThermalMode::GMode {
            self.failsafe
                .tripped_previous
                .or(self.g_mode_previous)
                .unwrap_or(ThermalMode::Balanced)
        } else {
            self.g_mode_previous = Some(self.mode);
            ThermalMode::GMode
        };
        if target == self.mode {
            return;
        }
        self.switch_mode(target);
        if self.exit_code == 0 {
            self.notify_mode(false);
        }
    }

    fn on_failsafe_toggled(&mut self) {
        self.failsafe
            .set_enabled(widgets::checkbox_checked(self.failsafe_checkbox));
        self.update_indicator();
    }

    fn on_threshold_changed(&mut self, id: i32) {
        match id {
            ID_LIMIT_GPU => {
                let current = self.failsafe.gpu_limit as i32;
                self.failsafe.gpu_limit =
                    widgets::combo_value(self.limit_gpu, current).max(0) as u32;
            }
            ID_LIMIT_CPU => {
                let current = self.failsafe.cpu_limit as i32;
                self.failsafe.cpu_limit =
                    widgets::combo_value(self.limit_cpu, current).max(0) as u32;
            }
            _ => return,
        }
        self.update_failsafe_tooltip();
        self.failsafe.cool_since = None;
    }

    fn on_tray_menu(&mut self, command: usize) {
        match command {
            CMD_SHOW => self.show_window(),
            CMD_HIDE => self.hide_to_tray(),
            CMD_TRAY_ICON => self.cycle_tray_icon_mode(),
            CMD_AUTORUN_ENABLE => self.on_autorun(AutorunAction::Enable),
            CMD_AUTORUN_DISABLE => self.on_autorun(AutorunAction::Disable),
            CMD_RESTORE_DEFAULTS => self.restore_defaults(),
            CMD_EXIT => {
                self.prepare_exit();
                unsafe { DestroyWindow(self.hwnd) };
            }
            command if (CMD_MODE_FIRST..CMD_MODE_FIRST + 3).contains(&command) => {
                self.switch_mode(ThermalMode::ALL[command - CMD_MODE_FIRST]);
            }
            _ => {}
        }
    }

    fn show_window(&mut self) {
        unsafe {
            ShowWindow(self.hwnd, SW_RESTORE);
            SetForegroundWindow(self.hwnd);
        }
        self.refresh_gauges();
    }

    fn hide_to_tray(&self) {
        unsafe { ShowWindow(self.hwnd, SW_HIDE) };
        logln!("Window hidden to tray");
    }

    fn about(&self) {
        Dialog::info("About", &format!("{APP_NAME} v{APP_VERSION}"))
            .with_content(&format!("{APP_DESCRIPTION}\n{APP_URL}"))
            .show(self.hwnd);
    }

    fn on_autorun(&self, action: AutorunAction) {
        let enabled = action == AutorunAction::Enable;
        match autorun::apply(action) {
            Ok(0) => {
                Dialog::info(
                    "Success",
                    &format!(
                        "Autorun on system startup {}",
                        if enabled { "Enabled" } else { "Disabled" }
                    ),
                )
                .show(self.hwnd);
            }
            Ok(code) => {
                Dialog::error(
                    "Error",
                    &format!(
                        "Failed to {} autorun task. Error={code}",
                        if enabled { "add" } else { "remove" }
                    ),
                )
                .show(self.hwnd);
            }
            Err(error) => {
                Dialog::error("Error", &format!("Failed to change autorun task. {error}"))
                    .show(self.hwnd);
            }
        }
    }

    fn restore_defaults(&mut self) {
        let result = Dialog::warning(
            "Reset to Default",
            "Do you want to reset all settings to default?",
        )
        .with_buttons(vec![
            (widgets::ID_YES, "Reset".to_string()),
            (widgets::ID_NO, "Cancel".to_string()),
        ])
        .show(self.hwnd);
        if result.button != widgets::ID_YES {
            return;
        }
        let _ = self.settings.clear();
        self.settings = Settings::load();
        self.load_settings_into_ui();
    }

    /// Switches back to Balanced, as the Python app does when exiting.
    fn prepare_exit(&mut self) {
        // The mode is restored to Balanced only for the shutdown path. Save the
        // user's chosen mode first so the next launch can restore it, and keep
        // `saved` in sync with the transient Balanced state so `cleanup` does
        // not overwrite the file with it.
        self.save_settings();
        if self.mode != ThermalMode::Balanced && self.thermal.set_mode(ThermalMode::Balanced) {
            self.mode = ThermalMode::Balanced;
            self.saved.mode = ThermalMode::Balanced;
            logln!("Set mode Balanced on exit");
            self.notify_mode(false);
        }
    }

    fn fatal(&mut self, message: &str, content: &str) {
        if self.exit_code != 0 {
            return;
        }
        self.exit_code = 1;
        unsafe {
            KillTimer(self.hwnd, TIMER_UPDATE);
            KillTimer(self.hwnd, TIMER_SLIDER_DEBOUNCE);
        }
        self.hotkey = None;
        self.pending_error = Some((message.to_string(), content.to_string()));
        // Display only after this callback returns, so the modal message loop
        // cannot re-enter or destroy an App borrowed by the failed operation.
        unsafe { PostMessageW(self.hwnd, WM_FATAL, 0, 0) };
    }

    /// Returns `true` when the window should really close.
    fn on_close(&mut self) -> bool {
        // Resident background mode: the close button hides the window. Users
        // who stored an explicit "exit on close" preference keep that behavior.
        let minimize = self.settings.values.minimize_on_close.unwrap_or(true);
        if minimize {
            self.hide_to_tray();
            false
        } else {
            true
        }
    }

    fn cleanup(&mut self) {
        unsafe {
            KillTimer(self.hwnd, TIMER_UPDATE);
            KillTimer(self.hwnd, TIMER_SLIDER_DEBOUNCE);
            let data = NOTIFYICONDATAW {
                hWnd: self.hwnd,
                uID: TRAY_ICON_ID,
                ..Default::default()
            };
            Shell_NotifyIconW(NIM_DELETE, &data);
        }
        self.hotkey = None;
        self.save_settings();
        logln!("Cleanup: done");
    }
}

struct UnitIds {
    name: i32,
    name_text: i32,
    gauge_temp: i32,
    label_temp: i32,
    gauge_fan: i32,
    label_fan: i32,
    slider: i32,
    slider_label: i32,
}

fn reading(value: Option<u32>) -> String {
    value.map_or_else(|| "n/a".to_string(), |value| value.to_string())
}

pub fn register_window_class(instance: HINSTANCE) -> bool {
    let name = to_wide(WINDOW_CLASS_NAME);
    let class = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: 0,
        lpfnWndProc: Some(window_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance,
        hIcon: widgets::load_app_icon(0),
        hCursor: unsafe { LoadCursorW(null_mut(), IDC_ARROW) },
        hbrBackground: widgets::brushes().background,
        lpszMenuName: null(),
        lpszClassName: name.as_ptr(),
        hIconSm: widgets::load_app_icon(16),
    };
    unsafe { RegisterClassExW(&class) != 0 }
}

/// User-facing entry point: wires up COM, the window and the message loop.
pub fn run() -> i32 {
    use crate::thermal::ThermalError;
    use crate::wmi::Com;

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|argument| argument == "--install-autorun") {
        return autorun_cli(AutorunAction::Enable);
    }
    if args
        .iter()
        .any(|argument| argument == "--uninstall-autorun")
    {
        return autorun_cli(AutorunAction::Disable);
    }
    if args.iter().any(|argument| argument == "--quit") {
        return request_quit();
    }

    enable_dpi_awareness();
    logln!(
        "{APP_NAME} v{APP_VERSION} starting (elevated: {}, log: {})",
        is_process_elevated(),
        logging::log_path().display()
    );

    let _com = match Com::init(true) {
        Ok(com) => com,
        Err(error) => {
            Dialog::error(APP_NAME, &format!("COM initialization failed: {error}"))
                .show(null_mut());
            return 1;
        }
    };

    unsafe {
        let controls = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_STANDARD_CLASSES | ICC_BAR_CLASSES | ICC_WIN95_CLASSES,
        };
        InitCommonControlsEx(&controls);
    }

    let instance = unsafe { GetModuleHandleW(null()) };
    if !widgets::register_gauge_class(instance) || !register_window_class(instance) {
        Dialog::error(APP_NAME, "Could not register the window classes").show(null_mut());
        return 1;
    }

    // Only one instance may control the thermal interface at a time. A second
    // launch is treated as a request to bring the existing resident window up.
    let mutex_name = to_wide("Local\\TCC_G15_SingleInstance");
    let mutex = unsafe { CreateMutexW(null(), 1, mutex_name.as_ptr()) };
    if mutex.is_null() {
        Dialog::error(APP_NAME, "Could not create the single-instance guard").show(null_mut());
        return 1;
    }
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        let class = to_wide(WINDOW_CLASS_NAME);
        let existing = unsafe { FindWindowW(class.as_ptr(), null()) };
        if !existing.is_null() && unsafe { PostMessageW(existing, WM_SHOW_REQUEST, 0, 0) } != 0 {
            return 0;
        }
        Dialog::error(APP_NAME, "Another instance of this app is already running").show(null_mut());
        return 1;
    }

    if !is_process_elevated() {
        let result = Dialog::warning(
            APP_NAME,
            "This app needs administrator privileges to control the thermal interface.",
        )
        .with_buttons(vec![
            (widgets::ID_YES, "Restart as administrator".to_string()),
            (widgets::ID_NO, "Continue anyway".to_string()),
        ])
        .show(null_mut());
        if result.button == widgets::ID_YES {
            relaunch_elevated();
            return 0;
        }
    }

    let thermal = match Thermal::connect() {
        Ok(thermal) => thermal,
        Err(ThermalError::NoWmiClass) => {
            Dialog::error("Oh-oh", "AWCC WMI class not found in the system.")
                .with_content(
                    "You don't have some drivers installed or your system is not supported.",
                )
                .show(null_mut());
            return 1;
        }
        Err(ThermalError::CannotInstantiate) => {
            Dialog::error("Oh-oh", "Couldn't instantiate AWCC WMI class.")
                .with_content("Make sure you're running as Admin.")
                .show(null_mut());
            return 1;
        }
    };
    logln!("thermal interface ready: {} fans", thermal.fan_count());

    let start_minimized = std::env::args().any(|argument| argument == "--minimized");
    logln!("creating the main window (minimized: {start_minimized})");
    let params = Box::into_raw(Box::new(InitParams { thermal }));
    let class = to_wide(WINDOW_CLASS_NAME);
    let title = to_wide(APP_NAME);
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPED_WINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            null_mut(),
            null_mut(),
            instance,
            params as *mut std::ffi::c_void,
        )
    };
    if hwnd.is_null() {
        unsafe { drop(Box::from_raw(params)) };
        Dialog::error(APP_NAME, "Could not create the main window").show(null_mut());
        return 1;
    }

    // Allow a later, possibly lower-integrity process to ask this window to
    // come to the foreground without tripping User Interface Privilege
    // Isolation (UIPI).
    unsafe {
        ChangeWindowMessageFilterEx(hwnd, WM_SHOW_REQUEST, MSGFLT_ALLOW, null_mut());
        ChangeWindowMessageFilterEx(hwnd, WM_FORCE_EXIT, MSGFLT_ALLOW, null_mut());
        let taskbar_message = taskbar_created_message();
        if taskbar_message != 0 {
            ChangeWindowMessageFilterEx(hwnd, taskbar_message, MSGFLT_ALLOW, null_mut());
        }
    }

    if !start_minimized {
        center_window(hwnd);
        unsafe {
            ShowWindow(hwnd, SW_SHOW);
            UpdateWindow(hwnd);
        }
        if let Some(application) = app(hwnd) {
            application.refresh_gauges();
        }
    }
    logln!("window shown, entering the message loop");

    let mut message: MSG = unsafe { std::mem::zeroed() };
    loop {
        let result = unsafe { GetMessageW(&mut message, null_mut(), 0, 0) };
        if result <= 0 {
            break;
        }
        unsafe {
            if IsDialogMessageW(hwnd, &mut message) == 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
    let exit_code = message.wParam as i32;
    logln!("exiting with code {exit_code}");
    exit_code
}

fn autorun_cli(action: AutorunAction) -> i32 {
    match autorun::apply(action) {
        Ok(code) => {
            logln!(
                "autorun {}: exit {code}",
                if action == AutorunAction::Enable {
                    "enable"
                } else {
                    "disable"
                }
            );
            code
        }
        Err(error) => {
            logln!("autorun failed: {error}");
            1
        }
    }
}

/// Asks a running instance to exit and waits until its window is gone.
fn request_quit() -> i32 {
    let class = to_wide(WINDOW_CLASS_NAME);
    let existing = unsafe { FindWindowW(class.as_ptr(), null()) };
    if existing.is_null() {
        return 0;
    }
    if unsafe { PostMessageW(existing, WM_FORCE_EXIT, 0, 0) } == 0 {
        logln!("--quit: could not signal the running instance");
        return 1;
    }
    for _ in 0..80 {
        std::thread::sleep(Duration::from_millis(100));
        if unsafe { FindWindowW(class.as_ptr(), null()) }.is_null() {
            return 0;
        }
    }
    logln!("--quit: timed out waiting for the running instance");
    1
}

const CW_USEDEFAULT: i32 = i32::MIN;
const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: isize = -4;

/// Per-monitor-v2 DPI awareness when available, plain DPI awareness otherwise.
fn enable_dpi_awareness() {
    unsafe {
        let user32 = GetModuleHandleW(to_wide("user32.dll").as_ptr());
        if !user32.is_null() {
            let address = GetProcAddress(
                user32,
                c"SetProcessDpiAwarenessContext".as_ptr() as *const u8,
            );
            if !address.is_null() {
                let set: unsafe extern "system" fn(isize) -> BOOL = std::mem::transmute(address);
                if set(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) != 0 {
                    return;
                }
            }
        }
        SetProcessDPIAware();
    }
}

fn center_window(hwnd: HWND) {
    let mut rect = RECT::new(0, 0, 0, 0);
    unsafe {
        if GetWindowRect(hwnd, &mut rect) == 0 {
            return;
        }
        let width = rect.width();
        let height = rect.height();
        let x = (GetSystemMetrics(SM_CXSCREEN) - width) / 2;
        let y = (GetSystemMetrics(SM_CYSCREEN) - height) / 3;
        SetWindowPos(
            hwnd,
            null_mut(),
            x,
            y,
            0,
            0,
            widgets::SWP_NOSIZE | widgets::SWP_NOZORDER | widgets::SWP_NOACTIVATE,
        );
    }
}

/// Restarts the app elevated through the UAC prompt.
fn relaunch_elevated() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let arguments: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    let operation = to_wide("runas");
    let file = to_wide(&exe.to_string_lossy());
    let parameters = to_wide(&arguments.join(" "));
    let result = unsafe {
        ShellExecuteW(
            null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            parameters.as_ptr(),
            null(),
            SW_SHOWNORMAL,
        )
    };
    result as isize > 32
}

fn app<'a>(hwnd: HWND) -> Option<&'a mut App> {
    unsafe { (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App).as_mut() }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let taskbar_message = taskbar_created_message();
    if taskbar_message != 0 && msg == taskbar_message {
        if let Some(application) = app(hwnd) {
            if application.exit_code == 0 {
                application.add_tray_icon();
                application.update_tray();
            }
        }
        return 0;
    }
    match msg {
        WM_CREATE => {
            let params = (*(lparam as *const CREATESTRUCTW)).lpCreateParams as *mut InitParams;
            let params = Box::from_raw(params);
            let application = Box::new(App::new(hwnd, *params));
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(application) as isize);
            0
        }
        WM_SYSCOMMAND => {
            let command = wparam & 0xFFF0;
            if command == SC_MINIMIZE {
                if let Some(application) = app(hwnd) {
                    application.hide_to_tray();
                }
                0
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
        WM_SIZE => {
            if wparam == SIZE_MINIMIZED {
                if let Some(application) = app(hwnd) {
                    application.hide_to_tray();
                }
            }
            0
        }
        WM_SHOW_REQUEST => {
            if let Some(application) = app(hwnd) {
                application.show_window();
            }
            0
        }
        WM_FORCE_EXIT => {
            if let Some(application) = app(hwnd) {
                application.prepare_exit();
                DestroyWindow(hwnd);
            }
            0
        }
        WM_TIMER => {
            if let Some(application) = app(hwnd) {
                match wparam {
                    TIMER_UPDATE => application.tick(),
                    TIMER_SLIDER_DEBOUNCE => {
                        KillTimer(hwnd, TIMER_SLIDER_DEBOUNCE);
                        application.update_fan_speeds();
                    }
                    _ => {}
                }
            }
            0
        }
        WM_HSCROLL => {
            if let Some(application) = app(hwnd) {
                let slider = lparam as HWND;
                if slider == application.gpu.slider || slider == application.cpu.slider {
                    SetTimer(hwnd, TIMER_SLIDER_DEBOUNCE, SLIDER_DEBOUNCE_MS, null());
                }
            }
            0
        }
        WM_COMMAND => {
            let id = (wparam & 0xFFFF) as i32;
            let notification = ((wparam >> 16) & 0xFFFF) as u32;
            if let Some(application) = app(hwnd) {
                match id {
                    ID_RADIO_BALANCED | ID_RADIO_GMODE | ID_RADIO_CUSTOM if notification == 0 => {
                        application.on_mode_radio(id)
                    }
                    ID_FAILSAFE_CHECKBOX if notification == 0 => application.on_failsafe_toggled(),
                    ID_LIMIT_GPU | ID_LIMIT_CPU if notification == 1 => {
                        application.on_threshold_changed(id)
                    }
                    _ => {}
                }
            }
            0
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            let hdc = wparam as HDC;
            SetTextColor(hdc, colors::colorref(colors::WHITE));
            SetBkColor(hdc, colors::colorref(colors::DARK_GREY));
            SetBkMode(
                hdc,
                if msg == WM_CTLCOLOREDIT || msg == WM_CTLCOLORBTN {
                    OPAQUE
                } else {
                    TRANSPARENT
                },
            );
            widgets::brushes().background as LRESULT
        }
        WM_DRAWITEM => {
            let item = &*(lparam as *const DRAWITEMSTRUCT);
            if item.CtlID == ID_INDICATOR as u32 {
                if let Some(application) = app(hwnd) {
                    application.draw_indicator(item.hDC, item.rcItem);
                }
            }
            1
        }
        WM_CONTEXTMENU => {
            if let Some(application) = app(hwnd) {
                application.about();
            }
            0
        }
        WM_TRAYICON => {
            if let Some(application) = app(hwnd) {
                match lparam as u32 {
                    WM_LBUTTONDBLCLK => application.show_window(),
                    WM_RBUTTONUP => {
                        let mut point = POINT { x: 0, y: 0 };
                        GetCursorPos(&mut point);
                        SetForegroundWindow(hwnd);
                        let command = TrackPopupMenu(
                            application.tray_menu,
                            TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY,
                            point.x,
                            point.y,
                            0,
                            hwnd,
                            null(),
                        );
                        // Documented workaround: a stray message makes the menu
                        // close properly when the user clicks elsewhere.
                        PostMessageW(hwnd, 0, 0, 0);
                        if command != 0 {
                            application.on_tray_menu(command as usize);
                        }
                    }
                    _ => {}
                }
            }
            0
        }
        WM_HOTKEY_TRIGGERED => {
            if let Some(application) = app(hwnd) {
                application.on_hotkey();
            }
            0
        }
        WM_NAMES_READY => {
            if let Some(application) = app(hwnd) {
                application.on_names_ready(lparam);
            }
            0
        }
        WM_FATAL => {
            let error = app(hwnd).and_then(|application| application.pending_error.take());
            if let Some((message, content)) = error {
                Dialog::error("Oh-oh", &message)
                    .with_content(&content)
                    .show(hwnd);
            }
            // WM_QUIT must be posted after the modal loop has finished.
            unsafe { DestroyWindow(hwnd) };
            0
        }
        WM_DPICHANGED => {
            if let Some(application) = app(hwnd) {
                application.dpi = (wparam & 0xFFFF) as u32;
                application.fonts.delete();
                application.fonts = widgets::Fonts::new(application.dpi);
                application.apply_fonts();
                application.relayout();
            }
            0
        }
        WM_ERASEBKGND => {
            let hdc = wparam as HDC;
            let mut rect = RECT::new(0, 0, 0, 0);
            GetClientRect(hwnd, &mut rect);
            FillRect(hdc, &rect, widgets::brushes().background);
            1
        }
        WM_QUERYENDSESSION => {
            // Persist the latest values before Windows ends the session; the
            // normal close path (and its exit-to-Balanced policy) does not run.
            if let Some(application) = app(hwnd) {
                application.save_settings();
            }
            1
        }
        WM_ENDSESSION => {
            // Windows may send this after our window has already been hidden.
            // Nothing to do: the process is terminated after the shutdown.
            0
        }
        WM_CLOSE => {
            if let Some(application) = app(hwnd) {
                if application.on_close() {
                    application.prepare_exit();
                    DestroyWindow(hwnd);
                }
            }
            0
        }
        WM_DESTROY => {
            let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
            if !pointer.is_null() {
                let mut exit_code = 0;
                if let Some(application) = pointer.as_mut() {
                    exit_code = application.exit_code;
                    application.cleanup();
                }
                drop(Box::from_raw(pointer));
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                PostQuitMessage(exit_code);
            } else {
                PostQuitMessage(0);
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> Failsafe {
        Failsafe {
            enabled: true,
            gpu_limit: 85,
            cpu_limit: 95,
            high_since: None,
            cool_since: None,
            last_high: None,
            tripped_previous: None,
        }
    }

    #[test]
    fn failsafe_trips_only_after_the_delay() {
        let mut failsafe = limits();
        let start = Instant::now();
        assert!(failsafe
            .tick(start, ThermalMode::Balanced, Some(90), Some(50))
            .is_none());
        assert!(failsafe
            .tick(
                start + Duration::from_secs(7),
                ThermalMode::Balanced,
                Some(90),
                Some(50)
            )
            .is_none());
        assert!(matches!(
            failsafe.tick(
                start + Duration::from_secs(9),
                ThermalMode::Balanced,
                Some(90),
                Some(50)
            ),
            Some(FailsafeEvent::Tripped)
        ));
    }

    #[test]
    fn failsafe_ignores_a_momentary_spike() {
        let mut failsafe = limits();
        let start = Instant::now();
        failsafe.tick(start, ThermalMode::Balanced, Some(90), Some(50));
        // Temperature drops below the limit before the delay elapses.
        failsafe.tick(
            start + Duration::from_secs(3),
            ThermalMode::Balanced,
            Some(60),
            Some(50),
        );
        assert!(failsafe
            .tick(
                start + Duration::from_secs(20),
                ThermalMode::Balanced,
                Some(60),
                Some(50)
            )
            .is_none());
    }

    #[test]
    fn failsafe_resets_after_the_temperature_stays_ok() {
        let mut failsafe = limits();
        let start = Instant::now();
        failsafe.tick(start, ThermalMode::Balanced, Some(90), Some(50));
        failsafe.tick(
            start + Duration::from_secs(10),
            ThermalMode::Balanced,
            Some(90),
            Some(50),
        );
        assert!(failsafe.tripped_previous.is_some());

        let cool = start + Duration::from_secs(11);
        failsafe.tick(cool, ThermalMode::GMode, Some(60), Some(50));
        assert!(failsafe
            .tick(
                cool + Duration::from_secs(30),
                ThermalMode::GMode,
                Some(60),
                Some(50)
            )
            .is_none());
        assert!(matches!(
            failsafe.tick(
                cool + Duration::from_secs(61),
                ThermalMode::GMode,
                Some(60),
                Some(50)
            ),
            Some(FailsafeEvent::Reset(ThermalMode::Balanced))
        ));
        assert_eq!(
            failsafe.selected_mode(ThermalMode::GMode),
            ThermalMode::Balanced
        );
        // Ownership is released only after the hardware has restored the mode.
        failsafe.clear_auto_mode();
        assert!(failsafe.tripped_previous.is_none());
    }

    fn automatically_triggered(previous: ThermalMode) -> (Failsafe, Instant) {
        let mut failsafe = limits();
        let start = Instant::now();
        failsafe.tick(start, previous, Some(90), Some(50));
        assert!(matches!(
            failsafe.tick(start + Duration::from_secs(9), previous, Some(90), Some(50)),
            Some(FailsafeEvent::Tripped)
        ));
        (failsafe, start)
    }

    #[test]
    fn manual_g_mode_stays_on_after_temperatures_recover() {
        let mut failsafe = limits();
        let start = Instant::now();
        for (seconds, gpu, cpu) in [(0, 90, 99), (10, 90, 99), (11, 60, 50), (100, 60, 50)] {
            assert!(failsafe
                .tick(
                    start + Duration::from_secs(seconds),
                    ThermalMode::GMode,
                    Some(gpu),
                    Some(cpu)
                )
                .is_none());
        }
        assert_eq!(
            failsafe.selected_mode(ThermalMode::GMode),
            ThermalMode::GMode
        );
    }

    #[test]
    fn manually_selecting_automatic_g_mode_cancels_automatic_exit() {
        let (mut failsafe, start) = automatically_triggered(ThermalMode::Custom);
        failsafe.tick(
            start + Duration::from_secs(10),
            ThermalMode::GMode,
            Some(60),
            Some(50),
        );
        // A manual selection, including selecting the same active mode.
        failsafe.clear_auto_mode();
        assert!(failsafe
            .tick(
                start + Duration::from_secs(100),
                ThermalMode::GMode,
                Some(60),
                Some(50)
            )
            .is_none());
        assert_eq!(
            failsafe.selected_mode(ThermalMode::GMode),
            ThermalMode::GMode
        );
    }

    #[test]
    fn automatic_g_mode_restores_custom_after_sixty_cool_seconds() {
        let (mut failsafe, start) = automatically_triggered(ThermalMode::Custom);
        failsafe.tick(
            start + Duration::from_secs(10),
            ThermalMode::GMode,
            Some(60),
            Some(50),
        );
        assert!(failsafe
            .tick(
                start + Duration::from_secs(69),
                ThermalMode::GMode,
                Some(60),
                Some(50)
            )
            .is_none());
        assert!(matches!(
            failsafe.tick(
                start + Duration::from_secs(70),
                ThermalMode::GMode,
                Some(60),
                Some(50)
            ),
            Some(FailsafeEvent::Reset(ThermalMode::Custom))
        ));
        // A failed restore still saves the chosen mode, not transient G-mode.
        assert_eq!(
            failsafe.selected_mode(ThermalMode::GMode),
            ThermalMode::Custom
        );
    }

    #[test]
    fn high_or_missing_readings_restart_the_cooldown() {
        for (gpu, cpu) in [
            (Some(85), Some(50)),
            (Some(60), Some(95)),
            (None, Some(50)),
            (Some(60), None),
        ] {
            let (mut failsafe, start) = automatically_triggered(ThermalMode::Balanced);
            failsafe.tick(
                start + Duration::from_secs(10),
                ThermalMode::GMode,
                Some(60),
                Some(50),
            );
            assert!(failsafe
                .tick(
                    start + Duration::from_secs(69),
                    ThermalMode::GMode,
                    gpu,
                    cpu
                )
                .is_none());
            failsafe.tick(
                start + Duration::from_secs(70),
                ThermalMode::GMode,
                Some(60),
                Some(50),
            );
            assert!(failsafe
                .tick(
                    start + Duration::from_secs(129),
                    ThermalMode::GMode,
                    Some(60),
                    Some(50)
                )
                .is_none());
            assert!(matches!(
                failsafe.tick(
                    start + Duration::from_secs(130),
                    ThermalMode::GMode,
                    Some(60),
                    Some(50)
                ),
                Some(FailsafeEvent::Reset(ThermalMode::Balanced))
            ));
        }
    }

    #[test]
    fn first_cool_reading_after_a_gap_starts_the_full_cooldown() {
        let (mut failsafe, start) = automatically_triggered(ThermalMode::Balanced);
        assert!(failsafe
            .tick(
                start + Duration::from_secs(120),
                ThermalMode::GMode,
                Some(60),
                Some(50)
            )
            .is_none());
        assert!(failsafe
            .tick(
                start + Duration::from_secs(179),
                ThermalMode::GMode,
                Some(60),
                Some(50)
            )
            .is_none());
        assert!(matches!(
            failsafe.tick(
                start + Duration::from_secs(180),
                ThermalMode::GMode,
                Some(60),
                Some(50)
            ),
            Some(FailsafeEvent::Reset(ThermalMode::Balanced))
        ));
    }

    #[test]
    fn disabling_failsafe_preserves_automatic_mode_recovery() {
        let (mut failsafe, start) = automatically_triggered(ThermalMode::Balanced);
        failsafe.set_enabled(false);
        failsafe.tick(
            start + Duration::from_secs(10),
            ThermalMode::GMode,
            Some(60),
            Some(50),
        );
        assert!(matches!(
            failsafe.tick(
                start + Duration::from_secs(70),
                ThermalMode::GMode,
                Some(60),
                Some(50)
            ),
            Some(FailsafeEvent::Reset(ThermalMode::Balanced))
        ));
    }

    #[test]
    fn missing_readings_count_as_high() {
        let mut failsafe = limits();
        let start = Instant::now();
        failsafe.tick(start, ThermalMode::Balanced, None, Some(50));
        assert!(matches!(
            failsafe.tick(
                start + Duration::from_secs(9),
                ThermalMode::Balanced,
                None,
                Some(50)
            ),
            Some(FailsafeEvent::Tripped)
        ));
    }

    #[test]
    fn disabled_failsafe_never_trips() {
        let mut failsafe = limits();
        failsafe.enabled = false;
        let start = Instant::now();
        failsafe.tick(start, ThermalMode::Balanced, Some(99), Some(99));
        assert!(failsafe
            .tick(
                start + Duration::from_secs(60),
                ThermalMode::Balanced,
                Some(99),
                Some(99)
            )
            .is_none());
    }
}
