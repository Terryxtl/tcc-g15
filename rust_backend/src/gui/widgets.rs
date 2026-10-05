//! Small Win32 helpers shared by the GUI: fonts, brushes, controls, tooltips,
//! the custom gauge control and task dialogs.
//!
//! `HWND`, `HBRUSH` and friends are opaque handle values - they are passed
//! through, never dereferenced, so these helpers stay safe functions.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::ptr::{null, null_mut};
use std::sync::OnceLock;

use crate::colors::{self, Rgb};
use crate::win32::*;

/// Posted to a gauge control to change its value.
pub const WM_GAUGE_SET_VALUE: UINT = WM_APP + 11;

// --------------------------------------------------------------------- fonts

/// Fonts are recreated whenever the window moves to a monitor with another DPI.
pub struct Fonts {
    pub normal: HFONT,
    pub value: HFONT,
}

impl Fonts {
    pub fn new(dpi: u32) -> Fonts {
        Fonts {
            normal: create_font(9, FW_NORMAL, dpi),
            value: create_font(11, FW_NORMAL, dpi),
        }
    }

    pub fn delete(&self) {
        unsafe {
            DeleteObject(self.normal);
            DeleteObject(self.value);
        }
    }
}

fn font_height(point_size: i32, dpi: u32) -> i32 {
    let pixels = ((point_size as f32 * dpi as f32) / 72.0).round() as i32;
    -pixels.max(1)
}

/// Grayscale AA. ClearType assumes a light background and looks fringed on
/// this dark UI.
fn preferred_quality() -> BYTE {
    ANTIALIASED_QUALITY
}

/// Uses the system icon/message font as a base so the app gets the same face,
/// charset and fallback behavior as the rest of the desktop.
fn system_logfont() -> Option<LOGFONTW> {
    let mut logfont: LOGFONTW = unsafe { std::mem::zeroed() };
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETICONTITLELOGFONT,
            std::mem::size_of::<LOGFONTW>() as UINT,
            &mut logfont as *mut _ as *mut std::ffi::c_void,
            0,
        ) != 0
    };
    (ok && logfont.lfFaceName[0] != 0).then_some(logfont)
}

fn create_font(point_size: i32, weight: i32, dpi: u32) -> HFONT {
    let quality = preferred_quality();
    let mut logfont = system_logfont().unwrap_or_else(|| {
        let mut fallback: LOGFONTW = unsafe { std::mem::zeroed() };
        let face = to_wide("Segoe UI");
        let count = face.len().min(fallback.lfFaceName.len());
        fallback.lfCharSet = DEFAULT_CHARSET;
        fallback.lfFaceName[..count].copy_from_slice(&face[..count]);
        fallback
    });

    logfont.lfHeight = font_height(point_size, dpi);
    logfont.lfWidth = 0;
    logfont.lfEscapement = 0;
    logfont.lfOrientation = 0;
    logfont.lfWeight = weight;
    logfont.lfItalic = 0;
    logfont.lfUnderline = 0;
    logfont.lfStrikeOut = 0;
    logfont.lfOutPrecision = OUT_TT_PRECIS;
    logfont.lfClipPrecision = CLIP_DEFAULT_PRECIS;
    logfont.lfQuality = quality;
    logfont.lfPitchAndFamily = DEFAULT_PITCH;

    let font = unsafe { CreateFontIndirectW(&logfont) };
    if !font.is_null() {
        return font;
    }

    let face = to_wide("Segoe UI");
    unsafe {
        CreateFontW(
            logfont.lfHeight,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_TT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            quality as u32,
            DEFAULT_PITCH as u32,
            face.as_ptr(),
        )
    }
}

// ------------------------------------------------------------------- brushes

pub struct Brushes {
    pub background: HBRUSH,
    pub grey: HBRUSH,
    pub border: HBRUSH,
    pub green: HBRUSH,
    pub yellow: HBRUSH,
    pub red: HBRUSH,
    pub blue: HBRUSH,
    pub white: HBRUSH,
}

// GDI handles are process-wide objects; sharing them across threads is fine.
unsafe impl Send for Brushes {}
unsafe impl Sync for Brushes {}

fn brush(color: Rgb) -> HBRUSH {
    unsafe { CreateSolidBrush(colors::colorref(color)) }
}

/// Process-wide brushes; the GUI runs until the process exits.
pub fn brushes() -> &'static Brushes {
    static BRUSHES: OnceLock<Brushes> = OnceLock::new();
    BRUSHES.get_or_init(|| Brushes {
        background: brush(colors::DARK_GREY),
        grey: brush(colors::GREY),
        border: brush(colors::BORDER),
        green: brush(colors::GREEN),
        yellow: brush(colors::YELLOW),
        red: brush(colors::RED),
        blue: brush(colors::BLUE),
        white: brush(colors::WHITE),
    })
}

pub fn brush_for(color: Rgb) -> HBRUSH {
    let brushes = brushes();
    match color {
        colors::GREEN => brushes.green,
        colors::YELLOW => brushes.yellow,
        colors::RED => brushes.red,
        colors::BLUE => brushes.blue,
        colors::WHITE => brushes.white,
        colors::GREY => brushes.grey,
        _ => brushes.background,
    }
}

// -------------------------------------------------------------- control setup

pub fn create_control(
    parent: HWND,
    class: &str,
    text: &str,
    style: u32,
    id: i32,
    instance: HINSTANCE,
) -> HWND {
    let class = to_wide(class);
    let text = to_wide(text);
    unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            text.as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            0,
            0,
            10,
            10,
            parent,
            id as HMENU,
            instance,
            null_mut(),
        )
    }
}

pub fn set_font(hwnd: HWND, font: HFONT) {
    unsafe { SendMessageW(hwnd, WM_SETFONT, font as WPARAM, 1) };
}

pub fn move_control(hwnd: HWND, x: i32, y: i32, width: i32, height: i32) {
    unsafe {
        SetWindowPos(
            hwnd,
            null_mut(),
            x,
            y,
            width,
            height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        )
    };
}

pub const SWP_NOZORDER: UINT = 0x0004;
pub const SWP_NOMOVE: UINT = 0x0002;
pub const SWP_NOSIZE: UINT = 0x0001;
pub const SWP_NOACTIVATE: UINT = 0x0010;

/// Turns off visual styles for a control so its colours can be themed by hand.
/// Without this, themed combo boxes and track bars ignore WM_CTLCOLOR* and stay
/// light grey.
pub fn untheme(hwnd: HWND) {
    let empty = to_wide("");
    unsafe { SetWindowTheme(hwnd, empty.as_ptr(), empty.as_ptr()) };
}

pub fn text(hwnd: HWND) -> String {
    let mut buffer = [0u16; 512];
    let length = unsafe { GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
    String::from_utf16_lossy(&buffer[..length.max(0) as usize])
}

pub fn combo_add_items(hwnd: HWND, items: &[String]) {
    for item in items {
        let wide = to_wide(item);
        unsafe { SendMessageW(hwnd, CB_ADDSTRING, 0, wide.as_ptr() as LPARAM) };
    }
}

pub fn combo_selection(hwnd: HWND) -> Option<i32> {
    let index = unsafe { SendMessageW(hwnd, CB_GETCURSEL, 0, 0) };
    (index >= 0).then_some(index as i32)
}

pub fn combo_select_text(hwnd: HWND, value: &str) {
    let count = unsafe { SendMessageW(hwnd, CB_GETCOUNT, 0, 0) };
    for index in 0..count {
        let mut buffer = [0u16; 64];
        let length = unsafe {
            SendMessageW(
                hwnd,
                CB_GETLBTEXT,
                index as WPARAM,
                buffer.as_mut_ptr() as LPARAM,
            )
        };
        if length < 0 {
            continue;
        }
        let item = String::from_utf16_lossy(&buffer[..length as usize]);
        if item == value {
            unsafe { SendMessageW(hwnd, CB_SETCURSEL, index as WPARAM, 0) };
            return;
        }
    }
}

pub fn combo_value(hwnd: HWND, fallback: i32) -> i32 {
    combo_selection(hwnd)
        .map(|index| {
            let mut buffer = [0u16; 64];
            let length = unsafe {
                SendMessageW(
                    hwnd,
                    CB_GETLBTEXT,
                    index as WPARAM,
                    buffer.as_mut_ptr() as LPARAM,
                )
            };
            if length <= 0 {
                return fallback;
            }
            String::from_utf16_lossy(&buffer[..length as usize])
                .parse::<i32>()
                .unwrap_or(fallback)
        })
        .unwrap_or(fallback)
}

pub fn checkbox_checked(hwnd: HWND) -> bool {
    unsafe { SendMessageW(hwnd, BM_GETCHECK, 0, 0) == BST_CHECKED as LRESULT }
}

pub fn set_checked(hwnd: HWND, checked: bool) {
    unsafe {
        SendMessageW(
            hwnd,
            BM_SETCHECK,
            if checked { BST_CHECKED } else { BST_UNCHECKED },
            0,
        )
    };
}

pub fn slider_range(hwnd: HWND, min: i32, max: i32) {
    unsafe { SendMessageW(hwnd, TBM_SETRANGE, 1, pack(min, max)) };
}

pub fn slider_set(hwnd: HWND, value: i32) {
    unsafe { SendMessageW(hwnd, TBM_SETPOS, 1, value as LPARAM) };
}

pub fn slider_get(hwnd: HWND) -> i32 {
    unsafe { SendMessageW(hwnd, TBM_GETPOS, 0, 0) as i32 }
}

pub fn slider_set_tick_freq(hwnd: HWND, frequency: i32) {
    unsafe { SendMessageW(hwnd, TBM_SETTICFREQ, frequency as WPARAM, 0) };
}

fn pack(low: i32, high: i32) -> LPARAM {
    (low as u32 as u64 | ((high as u32 as u64) << 16)) as LPARAM
}

// ------------------------------------------------------------------ tooltips

pub fn create_tooltip(parent: HWND, instance: HINSTANCE) -> HWND {
    let class = to_wide("tooltips_class32");
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            null(),
            WS_POPUP | TTS_ALWAYSTIP | TTS_NOPREFIX,
            0,
            0,
            0,
            0,
            parent,
            null_mut(),
            instance,
            null_mut(),
        )
    };
    if !hwnd.is_null() {
        unsafe { SendMessageW(hwnd, TTM_SETMAXTIPWIDTH, 0, 400) };
    }
    hwnd
}

/// Attaches `tip` to `control`. The text buffer is owned by the tooltip
/// control, so the caller must keep `text` alive for as long as the tooltip
/// shows - we store the strings in `TooltipStore`.
pub fn tooltip_add(tooltip: HWND, control: HWND, text: &mut [u16]) {
    let mut info = TOOLINFOW {
        cbSize: std::mem::size_of::<TOOLINFOW>() as u32,
        uFlags: TTF_IDISHWND | TTF_SUBCLASS,
        hwnd: unsafe { GetParent(control) },
        uId: control as usize,
        rect: RECT::new(0, 0, 0, 0),
        hinst: null_mut(),
        lpszText: text.as_mut_ptr(),
        lParam: 0,
        lpReserved: null_mut(),
    };
    unsafe {
        SendMessageW(
            tooltip,
            TTM_ADDTOOLW,
            0,
            &mut info as *mut TOOLINFOW as LPARAM,
        )
    };
}

pub fn tooltip_update(tooltip: HWND, control: HWND, text: &mut [u16]) {
    let mut info = TOOLINFOW {
        cbSize: std::mem::size_of::<TOOLINFOW>() as u32,
        uFlags: TTF_IDISHWND,
        hwnd: unsafe { GetParent(control) },
        uId: control as usize,
        rect: RECT::new(0, 0, 0, 0),
        hinst: null_mut(),
        lpszText: text.as_mut_ptr(),
        lParam: 0,
        lpReserved: null_mut(),
    };
    unsafe {
        SendMessageW(
            tooltip,
            TTM_UPDATETIPTEXTW,
            0,
            &mut info as *mut TOOLINFOW as LPARAM,
        )
    };
}

// --------------------------------------------------------------------- gauge

pub const GAUGE_CLASS_NAME: &str = "TCC_G15_Gauge";

/// Payload of a gauge window, reachable through `GWLP_USERDATA`.
struct GaugeState {
    min: i32,
    max: i32,
    value: i32,
    plain_color: Rgb,
    heat: Option<(u32, u32)>,
}

/// Registers the gauge window class. Returns `false` if registration failed.
pub fn register_gauge_class(instance: HINSTANCE) -> bool {
    let name = to_wide(GAUGE_CLASS_NAME);
    let class = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: 0,
        lpfnWndProc: Some(gauge_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance,
        hIcon: null_mut(),
        hCursor: unsafe { LoadCursorW(null_mut(), IDC_ARROW) },
        hbrBackground: null_mut(),
        lpszMenuName: null(),
        lpszClassName: name.as_ptr(),
        hIconSm: null_mut(),
    };
    unsafe { RegisterClassExW(&class) != 0 }
}

pub fn create_gauge(
    parent: HWND,
    id: i32,
    min: i32,
    max: i32,
    plain_color: Rgb,
    heat: Option<(u32, u32)>,
    instance: HINSTANCE,
) -> HWND {
    let class = to_wide(GAUGE_CLASS_NAME);
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            null(),
            WS_CHILD | WS_VISIBLE,
            0,
            0,
            10,
            10,
            parent,
            id as HMENU,
            instance,
            null_mut(),
        )
    };
    if hwnd.is_null() {
        return hwnd;
    }
    let state = Box::new(GaugeState {
        min,
        max,
        value: min,
        plain_color,
        heat,
    });
    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize) };
    hwnd
}

pub fn gauge_set_value(hwnd: HWND, value: i32) {
    unsafe { SendMessageW(hwnd, WM_GAUGE_SET_VALUE, value as WPARAM, 0) };
}

unsafe fn gauge_state<'a>(hwnd: HWND) -> Option<&'a mut GaugeState> {
    let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut GaugeState;
    pointer.as_mut()
}

unsafe extern "system" fn gauge_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut paint: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut paint);
            paint_gauge(hwnd, hdc);
            EndPaint(hwnd, &paint);
            0
        }
        // Also render into printer/capture DCs so screenshots are faithful.
        WM_PRINTCLIENT => {
            paint_gauge(hwnd, wparam as HDC);
            0
        }
        WM_ERASEBKGND => 1,
        m if m == WM_GAUGE_SET_VALUE => {
            if let Some(state) = gauge_state(hwnd) {
                state.value = wparam as i32;
            }
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_DESTROY => {
            let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut GaugeState;
            if !pointer.is_null() {
                drop(Box::from_raw(pointer));
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn paint_gauge(hwnd: HWND, hdc: HDC) {
    let mut rect = RECT::new(0, 0, 0, 0);
    unsafe { GetClientRect(hwnd, &mut rect) };
    let Some(state) = (unsafe { gauge_state(hwnd) }) else {
        return;
    };
    let brushes = brushes();
    unsafe { FillRect(hdc, &rect, brushes.grey) };
    let span = (state.max - state.min).max(1);
    let value = state.value.clamp(state.min, state.max);
    let width = ((value - state.min) as f32 / span as f32 * rect.width() as f32).round() as i32;
    if width > 0 {
        let chunk = RECT::new(rect.left, rect.top, rect.left + width, rect.bottom);
        let color = match state.heat {
            Some(limits) => colors::heat_color(value as u32, limits),
            None => state.plain_color,
        };
        unsafe { FillRect(hdc, &chunk, brush_for(color)) };
    }
    unsafe { FrameRect(hdc, &rect, brushes.border) };
}

// ------------------------------------------------------------------- dialogs

/// Pointer to `TaskDialogIndirect`, when the shell provides it.
fn task_dialog_indirect() -> Option<
    unsafe extern "system" fn(*const TASKDIALOGCONFIG, *mut i32, *mut i32, *mut BOOL) -> HRESULT,
> {
    static PROC: OnceLock<usize> = OnceLock::new();
    let address = *PROC.get_or_init(|| unsafe {
        let module = LoadLibraryW(to_wide("comctl32.dll").as_ptr());
        if module.is_null() {
            return 0;
        }
        GetProcAddress(module, c"TaskDialogIndirect".as_ptr() as *const u8) as usize
    });
    if address == 0 {
        return None;
    }
    type TaskDialogIndirect = unsafe extern "system" fn(
        *const TASKDIALOGCONFIG,
        *mut i32,
        *mut i32,
        *mut BOOL,
    ) -> HRESULT;
    let procedure: TaskDialogIndirect = unsafe { std::mem::transmute(address) };
    Some(procedure)
}

pub struct Dialog {
    pub title: String,
    pub instruction: String,
    pub content: Option<String>,
    pub icon: DialogIcon,
    pub buttons: Vec<(i32, String)>,
    pub verification: Option<String>,
    pub verification_checked: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DialogIcon {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy)]
pub struct DialogResult {
    pub button: i32,
    pub verified: bool,
}

impl Dialog {
    pub fn info(title: &str, instruction: &str) -> Dialog {
        Dialog {
            title: title.to_string(),
            instruction: instruction.to_string(),
            content: None,
            icon: DialogIcon::Info,
            buttons: vec![(ID_OK, "OK".to_string())],
            verification: None,
            verification_checked: false,
        }
    }

    pub fn error(title: &str, instruction: &str) -> Dialog {
        Dialog {
            icon: DialogIcon::Error,
            ..Dialog::info(title, instruction)
        }
    }

    pub fn warning(title: &str, instruction: &str) -> Dialog {
        Dialog {
            icon: DialogIcon::Warning,
            ..Dialog::info(title, instruction)
        }
    }

    pub fn with_content(mut self, content: &str) -> Dialog {
        self.content = Some(content.to_string());
        self
    }

    pub fn with_buttons(mut self, buttons: Vec<(i32, String)>) -> Dialog {
        self.buttons = buttons;
        self
    }

    pub fn with_verification(mut self, text: &str) -> Dialog {
        self.verification = Some(text.to_string());
        self
    }

    pub fn with_verification_checked(mut self, checked: bool) -> Dialog {
        self.verification_checked = checked;
        self
    }

    pub fn show(&self, parent: HWND) -> DialogResult {
        let title = to_wide(&self.title);
        let instruction = to_wide(&self.instruction);
        let content = self.content.as_ref().map(|text| to_wide(text));
        let verification = self.verification.as_ref().map(|text| to_wide(text));
        let button_texts: Vec<Vec<u16>> =
            self.buttons.iter().map(|(_, text)| to_wide(text)).collect();
        let buttons: Vec<TASKDIALOG_BUTTON> = self
            .buttons
            .iter()
            .zip(button_texts.iter())
            .map(|((id, _), text)| TASKDIALOG_BUTTON {
                nButtonID: *id,
                pszButtonText: text.as_ptr(),
            })
            .collect();

        if let Some(task_dialog) = task_dialog_indirect() {
            let icon = match self.icon {
                DialogIcon::Info => TD_INFORMATION_ICON,
                DialogIcon::Warning => TD_WARNING_ICON,
                DialogIcon::Error => TD_ERROR_ICON,
            };
            let config = TASKDIALOGCONFIG {
                cbSize: std::mem::size_of::<TASKDIALOGCONFIG>() as u32,
                hwndParent: parent,
                dwFlags: TDF_ALLOW_DIALOG_CANCELLATION
                    | TDF_POSITION_RELATIVE_TO_WINDOW
                    | if self.verification_checked {
                        TDF_VERIFICATION_FLAG_CHECKED
                    } else {
                        0
                    },
                pszWindowTitle: title.as_ptr(),
                hMainIcon: icon as *mut std::ffi::c_void,
                pszMainInstruction: instruction.as_ptr(),
                pszContent: content.as_ref().map(|text| text.as_ptr()).unwrap_or(null()),
                cButtons: buttons.len() as u32,
                pButtons: buttons.as_ptr(),
                nDefaultButton: self.buttons.first().map(|(id, _)| *id).unwrap_or(ID_OK),
                pszVerificationText: verification
                    .as_ref()
                    .map(|text| text.as_ptr())
                    .unwrap_or(null()),
                ..Default::default()
            };

            let mut button = ID_CANCEL;
            let mut verified: BOOL = 0;
            let mut radio = 0i32;
            let hr = unsafe { task_dialog(&config, &mut button, &mut radio, &mut verified) };
            if hr >= 0 {
                return DialogResult {
                    button,
                    verified: verified != 0,
                };
            }
        }

        // Fallback for shells without the v6 common controls.
        let mut text = self.instruction.clone();
        if let Some(content) = &self.content {
            text.push_str("\r\n\r\n");
            text.push_str(content);
        }
        let flags = MB_SETFOREGROUND
            | match self.icon {
                DialogIcon::Info => MB_ICONINFORMATION,
                DialogIcon::Warning => MB_ICONWARNING,
                DialogIcon::Error => MB_ICONERROR,
            }
            | if self.buttons.len() > 1 {
                MB_YESNO
            } else {
                MB_OK
            };
        let result = unsafe { MessageBoxW(parent, to_wide(&text).as_ptr(), title.as_ptr(), flags) };
        let button = if self.buttons.len() > 1 && result != IDYES {
            self.buttons.get(1).map(|(id, _)| *id).unwrap_or(ID_CANCEL)
        } else {
            self.buttons.first().map(|(id, _)| *id).unwrap_or(ID_OK)
        };
        DialogResult {
            button,
            verified: self.verification_checked,
        }
    }
}

pub const ID_OK: i32 = 100;
pub const ID_YES: i32 = 101;
pub const ID_NO: i32 = 102;
pub const ID_CANCEL: i32 = 2;

/// Embedded resource first (so Task Scheduler does not need the icons folder),
/// then `icons/gaugeIcon.ico` next to the exe, then the generic app icon.
pub fn load_app_icon(size: i32) -> HICON {
    let instance = unsafe { GetModuleHandleW(null()) };
    let from_resource = unsafe { LoadImageW(instance, 1 as PCWSTR, IMAGE_ICON, size, size, 0) };
    if !from_resource.is_null() {
        return from_resource as HICON;
    }

    let path = crate::icon_path();
    let wide = to_wide(&path.to_string_lossy());
    let from_file = unsafe {
        LoadImageW(
            null_mut(),
            wide.as_ptr(),
            IMAGE_ICON,
            size,
            size,
            LR_LOADFROMFILE,
        )
    };
    if !from_file.is_null() {
        return from_file as HICON;
    }
    unsafe { LoadIconW(null_mut(), IDI_APPLICATION) }
}

/// Owns tooltip strings: the tooltip control keeps the pointer we hand it, so
/// the buffers must never move. Fixed-size boxes never reallocate.
#[derive(Default)]
#[allow(clippy::vec_box)] // The boxes pin the buffers: the tooltip control keeps our pointers.
pub struct TooltipStrings {
    entries: Vec<Box<[u16; TOOLTIP_CAPACITY]>>,
}

const TOOLTIP_CAPACITY: usize = 256;

impl TooltipStrings {
    pub fn add(&mut self, tooltip: HWND, control: HWND, text: &str) -> usize {
        let mut buffer = Box::new([0u16; TOOLTIP_CAPACITY]);
        copy_wide(&mut buffer[..], text);
        tooltip_add(tooltip, control, &mut buffer[..]);
        self.entries.push(buffer);
        self.entries.len() - 1
    }

    pub fn update(&mut self, index: usize, tooltip: HWND, control: HWND, text: &str) {
        if let Some(buffer) = self.entries.get_mut(index) {
            copy_wide(&mut buffer[..], text);
            tooltip_update(tooltip, control, &mut buffer[..]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_pixel_height_rounds_to_the_nearest_pixel() {
        assert_eq!(font_height(9, 96), -12);
        assert_eq!(font_height(9, 120), -15);
        assert_eq!(font_height(11, 96), -15);
        assert_eq!(font_height(11, 144), -22);
    }

    #[test]
    fn system_fonts_can_be_created_and_destroyed() {
        let fonts = Fonts::new(96);
        assert!(!fonts.normal.is_null());
        assert!(!fonts.value.is_null());
        fonts.delete();
    }
}
