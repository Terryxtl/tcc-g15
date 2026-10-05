//! Hand-written Win32 bindings.
//!
//! The project builds with zero external crates, so every function, struct and
//! constant we use is declared here. Struct layouts are verified against the
//! Windows SDK by the `abi_layout_matches_sdk` test at the bottom of this file.

#![allow(non_snake_case, non_camel_case_types, dead_code)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;

pub type BOOL = i32;
pub type BYTE = u8;
pub type WORD = u16;
pub type DWORD = u32;
pub type UINT = u32;
pub type INT = i32;
pub type LONG = i32;
pub type LRESULT = isize;
pub type WPARAM = usize;
pub type LPARAM = isize;
pub type HRESULT = i32;
pub type HANDLE = *mut std::ffi::c_void;
pub type HINSTANCE = *mut std::ffi::c_void;
pub type HMODULE = *mut std::ffi::c_void;
pub type HICON = *mut std::ffi::c_void;
pub type HCURSOR = *mut std::ffi::c_void;
pub type HBRUSH = *mut std::ffi::c_void;
pub type HFONT = *mut std::ffi::c_void;
pub type HBITMAP = *mut std::ffi::c_void;
pub type HDC = *mut std::ffi::c_void;
pub type HGDIOBJ = *mut std::ffi::c_void;
pub type HMENU = *mut std::ffi::c_void;
pub type HWND = *mut std::ffi::c_void;
pub type COLORREF = u32;
pub type PCWSTR = *const u16;
pub type PWSTR = *mut u16;
pub type BSTR = *const u16;

pub const TRUE: BOOL = 1;
pub const FALSE: BOOL = 0;

// ---------------------------------------------------------------- structures

#[repr(C)]
#[derive(Clone, Copy)]
pub struct POINT {
    pub x: LONG,
    pub y: LONG,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RECT {
    pub left: LONG,
    pub top: LONG,
    pub right: LONG,
    pub bottom: LONG,
}

impl RECT {
    pub fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        RECT {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn width(&self) -> i32 {
        self.right - self.left
    }

    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
}

#[repr(C)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: UINT,
    pub wParam: WPARAM,
    pub lParam: LPARAM,
    pub time: DWORD,
    pub pt: POINT,
}

pub type WNDPROC = Option<
    unsafe extern "system" fn(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT,
>;

#[repr(C)]
pub struct WNDCLASSEXW {
    pub cbSize: UINT,
    pub style: UINT,
    pub lpfnWndProc: WNDPROC,
    pub cbClsExtra: INT,
    pub cbWndExtra: INT,
    pub hInstance: HINSTANCE,
    pub hIcon: HICON,
    pub hCursor: HCURSOR,
    pub hbrBackground: HBRUSH,
    pub lpszMenuName: PCWSTR,
    pub lpszClassName: PCWSTR,
    pub hIconSm: HICON,
}

#[repr(C)]
pub struct PAINTSTRUCT {
    pub hdc: HDC,
    pub fErase: BOOL,
    pub rcPaint: RECT,
    pub fRestore: BOOL,
    pub fIncUpdate: BOOL,
    pub rgbReserved: [BYTE; 32],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ICONINFO {
    pub fIcon: BOOL,
    pub xHotspot: DWORD,
    pub yHotspot: DWORD,
    pub hbmMask: HBITMAP,
    pub hbmColor: HBITMAP,
}

#[repr(C)]
pub struct CREATESTRUCTW {
    pub lpCreateParams: *mut std::ffi::c_void,
    pub hInstance: HINSTANCE,
    pub hMenu: HMENU,
    pub hwndParent: HWND,
    pub cy: INT,
    pub cx: INT,
    pub y: INT,
    pub x: INT,
    pub style: LONG,
    pub lpszName: PCWSTR,
    pub lpszClass: PCWSTR,
    pub dwExStyle: DWORD,
}

#[repr(C)]
pub struct MINMAXINFO {
    pub ptReserved: POINT,
    pub ptMaxSize: POINT,
    pub ptMaxPosition: POINT,
    pub ptMinTrackSize: POINT,
    pub ptMaxTrackSize: POINT,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SYSTEMTIME {
    pub wYear: WORD,
    pub wMonth: WORD,
    pub wDayOfWeek: WORD,
    pub wDay: WORD,
    pub wHour: WORD,
    pub wMinute: WORD,
    pub wSecond: WORD,
    pub wMilliseconds: WORD,
}

impl SYSTEMTIME {
    pub fn as_string(&self) -> String {
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.wYear, self.wMonth, self.wDay, self.wHour, self.wMinute, self.wSecond
        )
    }
}

#[repr(C)]
pub struct DRAWITEMSTRUCT {
    pub CtlType: UINT,
    pub CtlID: UINT,
    pub itemID: UINT,
    pub itemAction: UINT,
    pub itemState: UINT,
    pub hwndItem: HWND,
    pub hDC: HDC,
    pub rcItem: RECT,
    pub itemData: usize,
}

#[repr(C)]
pub struct INITCOMMONCONTROLSEX {
    pub dwSize: DWORD,
    pub dwICC: DWORD,
}

#[repr(C)]
pub struct TOOLINFOW {
    pub cbSize: UINT,
    pub uFlags: UINT,
    pub hwnd: HWND,
    pub uId: usize,
    pub rect: RECT,
    pub hinst: HINSTANCE,
    pub lpszText: PWSTR,
    pub lParam: LPARAM,
    /// Present since Windows XP (`NTDDI_VERSION >= NTDDI_WINXP`).
    pub lpReserved: *mut std::ffi::c_void,
}

// ------------------------------------------------------------------- GDI

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BITMAPINFOHEADER {
    pub biSize: DWORD,
    pub biWidth: LONG,
    pub biHeight: LONG,
    pub biPlanes: WORD,
    pub biBitCount: WORD,
    pub biCompression: DWORD,
    pub biSizeImage: DWORD,
    pub biXPelsPerMeter: LONG,
    pub biYPelsPerMeter: LONG,
    pub biClrUsed: DWORD,
    pub biClrImportant: DWORD,
}

impl BITMAPINFOHEADER {
    pub fn new(width: i32, height: i32, bit_count: u16) -> Self {
        BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: height,
            biPlanes: 1,
            biBitCount: bit_count,
            biCompression: BI_RGB,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        }
    }
}

#[repr(C)]
pub struct BITMAPINFO {
    pub bmiHeader: BITMAPINFOHEADER,
    pub bmiColors: [u32; 1],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LOGFONTW {
    pub lfHeight: LONG,
    pub lfWidth: LONG,
    pub lfEscapement: LONG,
    pub lfOrientation: LONG,
    pub lfWeight: LONG,
    pub lfItalic: BYTE,
    pub lfUnderline: BYTE,
    pub lfStrikeOut: BYTE,
    pub lfCharSet: BYTE,
    pub lfOutPrecision: BYTE,
    pub lfClipPrecision: BYTE,
    pub lfQuality: BYTE,
    pub lfPitchAndFamily: BYTE,
    pub lfFaceName: [u16; 32],
}

// ------------------------------------------------------- shell / task dialogs

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NOTIFYICONDATAW {
    pub cbSize: DWORD,
    pub hWnd: HWND,
    pub uID: UINT,
    pub uFlags: UINT,
    pub uCallbackMessage: UINT,
    pub hIcon: HICON,
    pub szTip: [u16; 128],
    pub dwState: DWORD,
    pub dwStateMask: DWORD,
    pub szInfo: [u16; 256],
    pub uVersion: UINT,
    pub szInfoTitle: [u16; 64],
    pub dwInfoFlags: DWORD,
    pub guidItem: GUID,
    pub hBalloonIcon: HICON,
}

impl Default for NOTIFYICONDATAW {
    fn default() -> Self {
        NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: null_mut(),
            uID: 0,
            uFlags: 0,
            uCallbackMessage: 0,
            hIcon: null_mut(),
            szTip: [0; 128],
            dwState: 0,
            dwStateMask: 0,
            szInfo: [0; 256],
            uVersion: 0,
            szInfoTitle: [0; 64],
            dwInfoFlags: 0,
            guidItem: GUID::default(),
            hBalloonIcon: null_mut(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GUID {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

pub type PFTASKDIALOGCALLBACK = Option<
    unsafe extern "system" fn(
        hwnd: HWND,
        msg: UINT,
        wparam: WPARAM,
        lparam: LPARAM,
        refdata: isize,
    ) -> HRESULT,
>;

/// commctrl.h declares this struct inside `#include <pshpack1.h>`, so it is
/// byte-packed. Getting this wrong corrupts the dialog configuration.
#[repr(C, packed(1))]
pub struct TASKDIALOGCONFIG {
    pub cbSize: UINT,
    pub hwndParent: HWND,
    pub hInstance: HINSTANCE,
    pub dwFlags: u32,
    pub dwCommonButtons: u32,
    pub pszWindowTitle: PCWSTR,
    /// Union of `HICON hMainIcon` / `PCWSTR pszMainIcon`.
    pub hMainIcon: *mut std::ffi::c_void,
    pub pszMainInstruction: PCWSTR,
    pub pszContent: PCWSTR,
    pub cButtons: UINT,
    pub pButtons: *const TASKDIALOG_BUTTON,
    pub nDefaultButton: INT,
    pub cRadioButtons: UINT,
    pub pRadioButtons: *const TASKDIALOG_BUTTON,
    pub nDefaultRadioButton: INT,
    pub pszVerificationText: PCWSTR,
    pub pszExpandedInformation: PCWSTR,
    pub pszExpandedControlText: PCWSTR,
    pub pszCollapsedControlText: PCWSTR,
    pub hFooterIcon: HICON,
    pub pszFooter: PCWSTR,
    pub pfCallback: PFTASKDIALOGCALLBACK,
    pub lpCallbackData: isize,
    pub cxWidth: UINT,
}

impl Default for TASKDIALOGCONFIG {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

#[repr(C, packed(1))]
pub struct TASKDIALOG_BUTTON {
    pub nButtonID: INT,
    pub pszButtonText: PCWSTR,
}

// ------------------------------------------------------------------ VARIANT

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VARIANT {
    pub vt: WORD,
    pub wReserved1: WORD,
    pub wReserved2: WORD,
    pub wReserved3: WORD,
    pub data: [u8; 16],
}

impl VARIANT {
    pub fn empty() -> Self {
        VARIANT {
            vt: VT_EMPTY,
            wReserved1: 0,
            wReserved2: 0,
            wReserved3: 0,
            data: [0; 16],
        }
    }

    pub fn from_i4(value: i32) -> Self {
        let mut v = VARIANT::empty();
        v.vt = VT_I4;
        v.data[0..4].copy_from_slice(&value.to_le_bytes());
        v
    }

    pub fn as_u32(&self) -> u32 {
        u32::from_le_bytes([self.data[0], self.data[1], self.data[2], self.data[3]])
    }

    pub fn as_i64(&self) -> i64 {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&self.data[0..8]);
        i64::from_le_bytes(buf)
    }

    pub fn bstr(&self) -> *const u16 {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&self.data[0..8]);
        usize::from_le_bytes(buf) as *const u16
    }
}

// -------------------------------------------------------------- style values

pub const WS_OVERLAPPED: u32 = 0x0000_0000;
pub const WS_CAPTION: u32 = 0x00C0_0000;
pub const WS_SYSMENU: u32 = 0x0008_0000;
pub const WS_MINIMIZEBOX: u32 = 0x0002_0000;
pub const WS_THICKFRAME: u32 = 0x0004_0000;
pub const WS_MAXIMIZEBOX: u32 = 0x0001_0000;
pub const WS_CHILD: u32 = 0x4000_0000;
pub const WS_VISIBLE: u32 = 0x1000_0000;
pub const WS_DISABLED: u32 = 0x0800_0000;
pub const WS_TABSTOP: u32 = 0x0001_0000;
pub const WS_GROUP: u32 = 0x0002_0000;
pub const WS_BORDER: u32 = 0x0080_0000;
pub const WS_CLIPSIBLINGS: u32 = 0x0400_0000;
pub const WS_VSCROLL: u32 = 0x0020_0000;
pub const WS_POPUP: u32 = 0x8000_0000;

pub const SS_LEFT: u32 = 0x0000_0000;
pub const SS_CENTER: u32 = 0x0000_0001;
pub const SS_NOPREFIX: u32 = 0x0000_0080;
pub const SS_CENTERIMAGE: u32 = 0x0000_0200;

pub const BS_AUTORADIOBUTTON: u32 = 0x0000_0009;
pub const BS_AUTOCHECKBOX: u32 = 0x0000_0003;

pub const CBS_DROPDOWNLIST: u32 = 0x0000_0003;

pub const TBS_HORZ: u32 = 0x0000_0000;
pub const TBS_AUTOTICKS: u32 = 0x0000_0001;
pub const TBS_BOTTOM: u32 = 0x0000_0000;

pub const WS_OVERLAPPED_WINDOW: u32 = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;

// ------------------------------------------------------------------- classes

pub const WC_STATIC: &[u16] = &[0x53, 0x74, 0x61, 0x74, 0x69, 0x63, 0];
pub const WC_BUTTON: &[u16] = &[0x42, 0x75, 0x74, 0x74, 0x6F, 0x6E, 0];
pub const WC_COMBOBOX: &[u16] = &[0x43, 0x6F, 0x6D, 0x62, 0x6F, 0x42, 0x6F, 0x78, 0];
pub const WC_TRACKBAR: &[u16] = &[
    0x6D, 0x73, 0x63, 0x74, 0x6C, 0x73, 0x5F, 0x74, 0x72, 0x61, 0x63, 0x6B, 0x62, 0x61, 0x72, 0x33,
    0x32, 0,
];
pub const WC_TOOLTIPS: &[u16] = &[
    0x74, 0x6F, 0x6F, 0x6C, 0x74, 0x69, 0x70, 0x73, 0x5F, 0x63, 0x6C, 0x61, 0x73, 0x73, 0x33, 0x32,
    0,
];

// ------------------------------------------------------------- window messages

pub const WM_CREATE: UINT = 0x0001;
pub const WM_DESTROY: UINT = 0x0002;
pub const WM_SIZE: UINT = 0x0005;
pub const WM_PAINT: UINT = 0x000F;
pub const WM_QUERYENDSESSION: UINT = 0x0011;
pub const WM_CLOSE: UINT = 0x0010;
pub const WM_ENDSESSION: UINT = 0x0016;
pub const WM_ERASEBKGND: UINT = 0x0014;
pub const WM_SETTINGCHANGE: UINT = 0x001A;
pub const WM_GETMINMAXINFO: UINT = 0x0024;
pub const WM_DRAWITEM: UINT = 0x002B;
pub const WM_MEASUREITEM: UINT = 0x002C;
pub const WM_SETFONT: UINT = 0x0030;
pub const WM_NOTIFY: UINT = 0x004E;
pub const WM_CONTEXTMENU: UINT = 0x007B;
pub const WM_SETICON: UINT = 0x0080;
pub const WM_NCHITTEST: UINT = 0x0084;
pub const WM_KEYDOWN: UINT = 0x0100;
pub const WM_KEYUP: UINT = 0x0101;
pub const WM_COMMAND: UINT = 0x0111;
pub const WM_SYSCOMMAND: UINT = 0x0112;
pub const SIZE_MINIMIZED: WPARAM = 1;
pub const SC_MINIMIZE: WPARAM = 0xF020;
pub const WM_TIMER: UINT = 0x0113;
pub const WM_HSCROLL: UINT = 0x0114;
pub const WM_INITMENUPOPUP: UINT = 0x0117;
pub const WM_CTLCOLORMSGBOX: UINT = 0x0132;
pub const WM_CTLCOLOREDIT: UINT = 0x0133;
pub const WM_CTLCOLORLISTBOX: UINT = 0x0134;
pub const WM_CTLCOLORBTN: UINT = 0x0135;
pub const WM_CTLCOLORDLG: UINT = 0x0136;
pub const WM_CTLCOLORSCROLLBAR: UINT = 0x0137;
pub const WM_CTLCOLORSTATIC: UINT = 0x0138;
pub const WM_LBUTTONUP: UINT = 0x0202;
pub const WM_LBUTTONDBLCLK: UINT = 0x0203;
pub const WM_RBUTTONUP: UINT = 0x0205;
pub const WM_APP: UINT = 0x8000;
pub const WM_HOTKEY: UINT = 0x0312;
pub const WM_PRINT: UINT = 0x0317;
pub const WM_PRINTCLIENT: UINT = 0x0318;
pub const WM_DPICHANGED: UINT = 0x02E0;

pub const WM_TRAYICON: UINT = WM_APP + 1;
pub const WM_HOTKEY_TRIGGERED: UINT = WM_APP + 2;

pub const BM_GETCHECK: UINT = 0x00F0;
pub const BM_SETCHECK: UINT = 0x00F1;
pub const BST_UNCHECKED: WPARAM = 0;
pub const BST_CHECKED: WPARAM = 1;

pub const CB_ADDSTRING: UINT = 0x0143;
pub const CB_GETCOUNT: UINT = 0x0146;
pub const CB_GETCURSEL: UINT = 0x0147;
pub const CB_GETLBTEXT: UINT = 0x0148;
pub const CB_SETCURSEL: UINT = 0x014E;

pub const TBM_GETPOS: UINT = 0x0400;
pub const TBM_SETPOS: UINT = 0x0405;
pub const TBM_SETRANGE: UINT = 0x0406;
pub const TBM_SETTICFREQ: UINT = 0x0414;
pub const TBM_SETPAGESIZE: UINT = 0x0415;

pub const TTM_ADDTOOLW: UINT = 0x0400 + 50;
pub const TTM_SETMAXTIPWIDTH: UINT = 0x0400 + 24;
pub const TTM_UPDATETIPTEXTW: UINT = 0x0400 + 57;
pub const TTF_IDISHWND: UINT = 0x0001;
pub const TTF_SUBCLASS: UINT = 0x0010;
pub const TTS_ALWAYSTIP: UINT = 0x01;
pub const TTS_NOPREFIX: UINT = 0x02;

pub const GWLP_WNDPROC: INT = -4;
pub const GWLP_USERDATA: INT = -21;
pub const GWL_ID: INT = -12;
pub const GWL_STYLE: INT = -16;
pub const GWL_EXSTYLE: INT = -20;

pub const SW_HIDE: INT = 0;
pub const SW_SHOWNORMAL: INT = 1;
pub const SW_SHOWMINIMIZED: INT = 2;
pub const SW_SHOW: INT = 5;
pub const SW_RESTORE: INT = 9;

pub const TPM_LEFTALIGN: UINT = 0x0000;
pub const TPM_RIGHTALIGN: UINT = 0x0008;
pub const TPM_RIGHTBUTTON: UINT = 0x0002;
pub const TPM_NONOTIFY: UINT = 0x0080;
pub const TPM_RETURNCMD: UINT = 0x0100;

pub const MF_STRING: UINT = 0x0000_0000;
pub const MF_BYCOMMAND: UINT = 0x0000_0000;
pub const MF_DISABLED: UINT = 0x0000_0002;
pub const MF_GRAYED: UINT = 0x0000_0001;

pub const COLOR_WINDOW: UINT = 5;
pub const IDC_ARROW: PCWSTR = 32512 as PCWSTR;
pub const IDI_APPLICATION: PCWSTR = 32512 as PCWSTR;
pub const IMAGE_ICON: UINT = 1;
pub const LR_LOADFROMFILE: UINT = 0x0000_0010;
pub const LR_DEFAULTSIZE: UINT = 0x0000_0040;
pub const LR_SHARED: UINT = 0x0000_8000;

pub const SM_CXSMICON: INT = 49;
pub const SM_CYSMICON: INT = 50;
pub const SM_CXSCREEN: INT = 0;
pub const SM_CYSCREEN: INT = 1;

pub const MB_OK: UINT = 0x0000_0000;
pub const MB_YESNO: UINT = 0x0000_0004;
pub const MB_ICONERROR: UINT = 0x0000_0010;
pub const MB_ICONQUESTION: UINT = 0x0000_0020;
pub const MB_ICONWARNING: UINT = 0x0000_0030;
pub const MB_ICONINFORMATION: UINT = 0x0000_0040;
pub const MB_SETFOREGROUND: UINT = 0x0001_0000;
pub const MB_TOPMOST: UINT = 0x0004_0000;
pub const IDYES: INT = 6;

pub const TRANSPARENT: INT = 1;
pub const OPAQUE: INT = 2;
pub const DT_LEFT: UINT = 0x0000;
pub const DT_CENTER: UINT = 0x0001;
pub const DT_RIGHT: UINT = 0x0002;
pub const DT_VCENTER: UINT = 0x0004;
pub const DT_SINGLELINE: UINT = 0x0020;
pub const DT_NOPREFIX: UINT = 0x0800;
pub const DT_TOP: UINT = 0x0000;
pub const DT_END_ELLIPSIS: UINT = 0x0000_8000;
pub const TA_LEFT: UINT = 0;
pub const TA_BASELINE: UINT = 24;

pub const FW_NORMAL: INT = 400;
pub const FW_SEMIBOLD: INT = 600;
pub const FW_BOLD: INT = 700;
pub const DEFAULT_CHARSET: BYTE = 1;
pub const OUT_DEFAULT_PRECIS: BYTE = 0;
pub const OUT_TT_PRECIS: BYTE = 4;
pub const CLIP_DEFAULT_PRECIS: BYTE = 0;
pub const DEFAULT_QUALITY: BYTE = 0;
pub const CLEARTYPE_QUALITY: BYTE = 5;
pub const ANTIALIASED_QUALITY: BYTE = 4;
pub const DEFAULT_PITCH: BYTE = 0;
pub const SPI_GETICONTITLELOGFONT: UINT = 0x001F;
pub const SPI_GETFONTSMOOTHING: UINT = 0x004A;
pub const SPI_GETFONTSMOOTHINGTYPE: UINT = 0x200B;
pub const FE_FONTSMOOTHINGCLEARTYPE: UINT = 0x0002;

pub const DIB_RGB_COLORS: UINT = 0;
pub const BI_RGB: DWORD = 0;

pub const NIM_ADD: DWORD = 0x0000_0000;
pub const NIM_MODIFY: DWORD = 0x0000_0001;
pub const NIM_DELETE: DWORD = 0x0000_0002;
pub const NIM_SETVERSION: DWORD = 0x0000_0004;
pub const NOTIFYICON_VERSION_4: UINT = 4;
pub const NIF_MESSAGE: UINT = 0x0000_0001;
pub const NIF_ICON: UINT = 0x0000_0002;
pub const NIF_TIP: UINT = 0x0000_0004;
pub const NIF_INFO: UINT = 0x0000_0010;
pub const NIF_SHOWTIP: UINT = 0x0000_0080;
pub const NIIF_INFO: DWORD = 0x0000_0001;
pub const NIIF_WARNING: DWORD = 0x0000_0002;
pub const NIIF_ERROR: DWORD = 0x0000_0003;

pub const MSGFLT_ALLOW: DWORD = 1;
pub const TDF_USE_HICON_MAIN: u32 = 0x0000_0002;
pub const TDF_ALLOW_DIALOG_CANCELLATION: u32 = 0x0000_0008;
pub const TDF_VERIFICATION_FLAG_CHECKED: u32 = 0x0000_0100;
pub const TDF_POSITION_RELATIVE_TO_WINDOW: u32 = 0x0000_1000;
pub const TDF_SIZE_TO_CONTENT: u32 = 0x0100_0000;
pub const TD_WARNING_ICON: PCWSTR = 0xFFFF as PCWSTR;
pub const TD_ERROR_ICON: PCWSTR = 0xFFFE as PCWSTR;
pub const TD_INFORMATION_ICON: PCWSTR = 0xFFFD as PCWSTR;

pub const ICC_STANDARD_CLASSES: DWORD = 0x0000_4000;
pub const ICC_BAR_CLASSES: DWORD = 0x0000_0008;
pub const ICC_PROGRESS_CLASS: DWORD = 0x0000_0020;
pub const ICC_WIN95_CLASSES: DWORD = 0x0000_00FF;

pub const ERROR_ALREADY_EXISTS: DWORD = 183;
pub const TOKEN_QUERY: DWORD = 0x0008;
pub const TOKEN_ELEVATION: INT = 20;

pub const VT_EMPTY: WORD = 0;
pub const VT_NULL: WORD = 1;
pub const VT_I2: WORD = 2;
pub const VT_I4: WORD = 3;
pub const VT_BSTR: WORD = 8;
pub const VT_UI2: WORD = 18;
pub const VT_UI4: WORD = 19;
pub const VT_I8: WORD = 20;
pub const VT_UI8: WORD = 21;
pub const VT_BOOL: WORD = 11;

pub const CLSCTX_INPROC_SERVER: DWORD = 0x1;
pub const COINIT_APARTMENTTHREADED: DWORD = 0x2;
pub const COINIT_MULTITHREADED: DWORD = 0x0;
pub const RPC_C_AUTHN_LEVEL_DEFAULT: DWORD = 0;
pub const RPC_C_AUTHN_LEVEL_CALL: DWORD = 3;
pub const RPC_C_IMP_LEVEL_IMPERSONATE: DWORD = 3;
pub const RPC_C_AUTHN_WINNT: DWORD = 10;
pub const RPC_C_AUTHZ_NONE: DWORD = 0;
pub const EOAC_NONE: DWORD = 0;
pub const RPC_E_TOO_LATE: HRESULT = 0x8001_0119u32 as HRESULT;

// ------------------------------------------------------------------ kernel32

#[link(name = "kernel32")]
extern "system" {
    pub fn GetModuleHandleW(lpModuleName: PCWSTR) -> HMODULE;
    pub fn GetLastError() -> DWORD;
    pub fn CloseHandle(hObject: HANDLE) -> BOOL;
    pub fn GetCurrentProcess() -> HANDLE;
    pub fn GetCurrentThreadId() -> DWORD;
    pub fn CreateMutexW(
        lpMutexAttributes: *const std::ffi::c_void,
        bInitialOwner: BOOL,
        lpName: PCWSTR,
    ) -> HANDLE;
    pub fn PostThreadMessageW(idThread: DWORD, msg: UINT, wParam: WPARAM, lParam: LPARAM) -> BOOL;
    pub fn GetLocalTime(lpSystemTime: *mut SYSTEMTIME);
    pub fn LoadLibraryW(lpLibFileName: PCWSTR) -> HMODULE;
    pub fn GetProcAddress(hModule: HMODULE, lpProcName: *const u8) -> *mut std::ffi::c_void;
}

// -------------------------------------------------------------------- user32

#[link(name = "user32")]
extern "system" {
    pub fn RegisterClassExW(lpWndClass: *const WNDCLASSEXW) -> u16;
    pub fn RegisterWindowMessageW(lpString: PCWSTR) -> UINT;
    pub fn UnregisterClassW(lpClassName: PCWSTR, hInstance: HINSTANCE) -> BOOL;
    pub fn CreateWindowExW(
        dwExStyle: DWORD,
        lpClassName: PCWSTR,
        lpWindowName: PCWSTR,
        dwStyle: DWORD,
        x: INT,
        y: INT,
        nWidth: INT,
        nHeight: INT,
        hWndParent: HWND,
        hMenu: HMENU,
        hInstance: HINSTANCE,
        lpParam: *mut std::ffi::c_void,
    ) -> HWND;
    pub fn DefWindowProcW(hwnd: HWND, msg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    pub fn DestroyWindow(hwnd: HWND) -> BOOL;
    pub fn ShowWindow(hwnd: HWND, nCmdShow: INT) -> BOOL;
    pub fn UpdateWindow(hwnd: HWND) -> BOOL;
    pub fn InvalidateRect(hwnd: HWND, lpRect: *const RECT, bErase: BOOL) -> BOOL;
    pub fn BeginPaint(hwnd: HWND, lpPaint: *mut PAINTSTRUCT) -> HDC;
    pub fn EndPaint(hwnd: HWND, lpPaint: *const PAINTSTRUCT) -> BOOL;
    pub fn FillRect(hdc: HDC, lprc: *const RECT, hbr: HBRUSH) -> INT;
    pub fn DrawTextW(
        hdc: HDC,
        lpchText: PCWSTR,
        cchText: INT,
        lprc: *mut RECT,
        format: UINT,
    ) -> INT;
    pub fn SetWindowTextW(hwnd: HWND, lpString: PCWSTR) -> BOOL;
    pub fn GetClientRect(hwnd: HWND, lpRect: *mut RECT) -> BOOL;
    pub fn GetWindowRect(hwnd: HWND, lpRect: *mut RECT) -> BOOL;
    pub fn GetWindowLongPtrW(hwnd: HWND, nIndex: INT) -> isize;
    pub fn SetWindowLongPtrW(hwnd: HWND, nIndex: INT, dwNewLong: isize) -> isize;
    pub fn AdjustWindowRectEx(
        lpRect: *mut RECT,
        dwStyle: DWORD,
        bMenu: BOOL,
        dwExStyle: DWORD,
    ) -> BOOL;
    pub fn SetWindowPos(
        hwnd: HWND,
        hWndInsertAfter: HWND,
        x: INT,
        y: INT,
        cx: INT,
        cy: INT,
        uFlags: UINT,
    ) -> BOOL;
    pub fn EnableWindow(hwnd: HWND, bEnable: BOOL) -> BOOL;
    pub fn SetForegroundWindow(hwnd: HWND) -> BOOL;
    pub fn GetMessageW(
        lpMsg: *mut MSG,
        hwnd: HWND,
        wMsgFilterMin: UINT,
        wMsgFilterMax: UINT,
    ) -> BOOL;
    pub fn TranslateMessage(lpMsg: *const MSG) -> BOOL;
    pub fn DispatchMessageW(lpMsg: *const MSG) -> LRESULT;
    pub fn PostQuitMessage(nExitCode: INT);
    pub fn PostMessageW(hwnd: HWND, msg: UINT, wParam: WPARAM, lParam: LPARAM) -> BOOL;
    pub fn SendMessageW(hwnd: HWND, msg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    pub fn IsDialogMessageW(hwnd: HWND, lpMsg: *mut MSG) -> BOOL;
    pub fn RegisterHotKey(hwnd: HWND, id: INT, fsModifiers: UINT, vk: UINT) -> BOOL;
    pub fn UnregisterHotKey(hwnd: HWND, id: INT) -> BOOL;
    pub fn SetTimer(
        hwnd: HWND,
        nIDEvent: usize,
        uElapse: UINT,
        lpTimerFunc: *const std::ffi::c_void,
    ) -> usize;
    pub fn KillTimer(hwnd: HWND, uIDEvent: usize) -> BOOL;
    pub fn CreatePopupMenu() -> HMENU;
    pub fn DestroyMenu(hMenu: HMENU) -> BOOL;
    pub fn AppendMenuW(hMenu: HMENU, uFlags: UINT, uIDNewItem: usize, lpNewItem: PCWSTR) -> BOOL;
    pub fn ModifyMenuW(
        hMnu: HMENU,
        uPosition: UINT,
        uFlags: UINT,
        uIDNewItem: usize,
        lpNewItem: PCWSTR,
    ) -> BOOL;
    pub fn TrackPopupMenu(
        hMenu: HMENU,
        uFlags: UINT,
        x: INT,
        y: INT,
        nReserved: INT,
        hwnd: HWND,
        prcRect: *const RECT,
    ) -> BOOL;
    pub fn GetCursorPos(lpPoint: *mut POINT) -> BOOL;
    pub fn MessageBoxW(hwnd: HWND, lpText: PCWSTR, lpCaption: PCWSTR, uType: UINT) -> INT;
    pub fn LoadImageW(
        hinst: HINSTANCE,
        name: PCWSTR,
        type_: UINT,
        cx: INT,
        cy: INT,
        fuLoad: UINT,
    ) -> HANDLE;
    pub fn LoadIconW(hInstance: HINSTANCE, lpIconName: PCWSTR) -> HICON;
    pub fn LoadCursorW(hInstance: HINSTANCE, lpCursorName: PCWSTR) -> HCURSOR;
    pub fn DestroyIcon(hIcon: HICON) -> BOOL;
    pub fn GetDC(hwnd: HWND) -> HDC;
    pub fn ReleaseDC(hwnd: HWND, hdc: HDC) -> INT;
    pub fn GetSystemMetrics(nIndex: INT) -> INT;
    pub fn GetDpiForWindow(hwnd: HWND) -> UINT;
    pub fn GetDpiForSystem() -> UINT;
    pub fn SetProcessDPIAware() -> BOOL;
    pub fn GetParent(hwnd: HWND) -> HWND;
    pub fn GetWindowTextW(hwnd: HWND, lpString: PWSTR, nMaxCount: INT) -> INT;
    pub fn FindWindowW(lpClassName: PCWSTR, lpWindowName: PCWSTR) -> HWND;
    pub fn IsWindowVisible(hwnd: HWND) -> BOOL;
    pub fn ChangeWindowMessageFilterEx(
        hwnd: HWND,
        message: UINT,
        action: DWORD,
        pChangeFilterStruct: *mut std::ffi::c_void,
    ) -> BOOL;
    pub fn SystemParametersInfoW(
        uiAction: UINT,
        uiParam: UINT,
        pvParam: *mut std::ffi::c_void,
        fWinIni: UINT,
    ) -> BOOL;
    pub fn FrameRect(hdc: HDC, lprc: *const RECT, hbr: HBRUSH) -> INT;
    pub fn SetBkColor(hdc: HDC, color: COLORREF) -> COLORREF;
}

#[link(name = "uxtheme")]
extern "system" {
    pub fn SetWindowTheme(hwnd: HWND, pszSubAppName: PCWSTR, pszSubIdList: PCWSTR) -> HRESULT;
}

// --------------------------------------------------------------------- gdi32

#[link(name = "gdi32")]
extern "system" {
    pub fn CreateSolidBrush(color: COLORREF) -> HBRUSH;
    pub fn DeleteObject(ho: HGDIOBJ) -> BOOL;
    pub fn CreateCompatibleDC(hdc: HDC) -> HDC;
    pub fn DeleteDC(hdc: HDC) -> BOOL;
    pub fn CreateDIBSection(
        hdc: HDC,
        pbmi: *const BITMAPINFO,
        usage: UINT,
        ppvBits: *mut *mut std::ffi::c_void,
        hSection: HANDLE,
        offset: DWORD,
    ) -> HBITMAP;
    pub fn CreateBitmap(
        nWidth: INT,
        nHeight: INT,
        nPlanes: UINT,
        nBitCount: UINT,
        lpBits: *const std::ffi::c_void,
    ) -> HBITMAP;
    pub fn SelectObject(hdc: HDC, h: HGDIOBJ) -> HGDIOBJ;
    pub fn SetBkMode(hdc: HDC, mode: INT) -> INT;
    pub fn SetTextColor(hdc: HDC, color: COLORREF) -> COLORREF;
    pub fn SetTextAlign(hdc: HDC, align: UINT) -> UINT;
    pub fn TextOutW(hdc: HDC, x: INT, y: INT, lpString: PCWSTR, c: INT) -> BOOL;
    pub fn CreatePen(style: INT, width: INT, color: COLORREF) -> HGDIOBJ;
    pub fn Ellipse(hdc: HDC, left: INT, top: INT, right: INT, bottom: INT) -> BOOL;
    pub fn CreateFontW(
        cHeight: INT,
        cWidth: INT,
        cEscapement: INT,
        cOrientation: INT,
        cWeight: INT,
        bItalic: DWORD,
        bUnderline: DWORD,
        bStrikeOut: DWORD,
        iCharSet: DWORD,
        iOutPrecision: DWORD,
        iClipPrecision: DWORD,
        iQuality: DWORD,
        iPitchAndFamily: DWORD,
        pszFaceName: PCWSTR,
    ) -> HFONT;
    pub fn CreateFontIndirectW(lplf: *const LOGFONTW) -> HFONT;
    pub fn CreateIconIndirect(piconinfo: *const ICONINFO) -> HICON;
}

// ------------------------------------------------------------------- shell32

#[link(name = "shell32")]
extern "system" {
    pub fn Shell_NotifyIconW(dwMessage: DWORD, lpData: *const NOTIFYICONDATAW) -> BOOL;
    pub fn ShellExecuteW(
        hwnd: HWND,
        lpOperation: PCWSTR,
        lpFile: PCWSTR,
        lpParameters: PCWSTR,
        lpDirectory: PCWSTR,
        nShowCmd: INT,
    ) -> HINSTANCE;
}

// ------------------------------------------------------------------ ole32

#[link(name = "ole32")]
extern "system" {
    pub fn CoInitializeEx(pvReserved: *mut std::ffi::c_void, dwCoInit: DWORD) -> HRESULT;
    pub fn CoUninitialize();
    pub fn CoCreateInstance(
        rclsid: *const GUID,
        pUnkOuter: *mut std::ffi::c_void,
        dwClsContext: DWORD,
        riid: *const GUID,
        ppv: *mut *mut std::ffi::c_void,
    ) -> HRESULT;
    pub fn CoInitializeSecurity(
        pSecDesc: *mut std::ffi::c_void,
        cAuthSvc: LONG,
        asAuthSvc: *mut std::ffi::c_void,
        pReserved1: *mut std::ffi::c_void,
        dwAuthnLevel: DWORD,
        dwImpLevel: DWORD,
        pAuthList: *mut std::ffi::c_void,
        dwCapabilities: DWORD,
        pReserved3: *mut std::ffi::c_void,
    ) -> HRESULT;
    pub fn CoSetProxyBlanket(
        pProxy: *mut std::ffi::c_void,
        dwAuthnSvc: DWORD,
        dwAuthzSvc: DWORD,
        pServerPrincName: PCWSTR,
        dwAuthnLevel: DWORD,
        dwImpLevel: DWORD,
        pAuthInfo: *mut std::ffi::c_void,
        dwCapabilities: DWORD,
    ) -> HRESULT;
    pub fn CoTaskMemFree(pv: *mut std::ffi::c_void);
}

// --------------------------------------------------------------- oleaut32

#[link(name = "oleaut32")]
extern "system" {
    pub fn SysAllocStringLen(str_: PCWSTR, ui: UINT) -> BSTR;
    pub fn SysFreeString(bstrString: BSTR);
    pub fn VariantInit(pvarg: *mut VARIANT);
    pub fn VariantClear(pvarg: *mut VARIANT);
    pub fn SafeArrayGetLBound(
        psa: *mut std::ffi::c_void,
        nDim: UINT,
        plLbound: *mut i32,
    ) -> HRESULT;
    pub fn SafeArrayGetUBound(
        psa: *mut std::ffi::c_void,
        nDim: UINT,
        plUbound: *mut i32,
    ) -> HRESULT;
    pub fn SafeArrayGetElement(
        psa: *mut std::ffi::c_void,
        rgIndices: *const i32,
        pv: *mut std::ffi::c_void,
    ) -> HRESULT;
    pub fn SafeArrayDestroy(psa: *mut std::ffi::c_void) -> HRESULT;
}

// --------------------------------------------------------------- comctl32

#[link(name = "comctl32")]
extern "system" {
    pub fn InitCommonControlsEx(picce: *const INITCOMMONCONTROLSEX) -> BOOL;
    pub fn _TrackMouseEvent(lpEventTrack: *mut std::ffi::c_void) -> BOOL;
}

// ---------------------------------------------------------------- advapi32

#[link(name = "advapi32")]
extern "system" {
    pub fn OpenProcessToken(
        ProcessHandle: HANDLE,
        DesiredAccess: DWORD,
        TokenHandle: *mut HANDLE,
    ) -> BOOL;
    pub fn GetTokenInformation(
        TokenHandle: HANDLE,
        TokenInformationClass: INT,
        TokenInformation: *mut std::ffi::c_void,
        TokenInformationLength: DWORD,
        ReturnLength: *mut DWORD,
    ) -> BOOL;
}

// ------------------------------------------------------------------- helpers

pub fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

pub fn copy_wide(dst: &mut [u16], src: &str) {
    let w = to_wide(src);
    let n = w.len().min(dst.len());
    dst[..n].copy_from_slice(&w[..n]);
    if n == dst.len() {
        dst[n - 1] = 0;
    }
}

pub fn wide_from_buf(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

pub fn succeeded(hr: HRESULT) -> bool {
    hr >= 0
}

pub fn hresult_message(hr: HRESULT) -> String {
    format!("0x{:08X}", hr as u32)
}

pub fn last_error() -> u32 {
    unsafe { GetLastError() }
}

/// `true` when the current process token is elevated.
pub fn is_process_elevated() -> bool {
    unsafe {
        let mut token: HANDLE = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation: DWORD = 0;
        let mut returned: DWORD = 0;
        let ok = GetTokenInformation(
            token,
            TOKEN_ELEVATION,
            &mut elevation as *mut DWORD as *mut std::ffi::c_void,
            std::mem::size_of::<DWORD>() as u32,
            &mut returned,
        );
        CloseHandle(token);
        ok != 0 && elevation != 0
    }
}

pub const RGB: fn(u8, u8, u8) -> COLORREF =
    |r, g, b| (r as u32) | ((g as u32) << 8) | ((b as u32) << 16);

#[cfg(test)]
mod abi_layout_matches_sdk {
    use super::*;
    use std::mem::{align_of, offset_of, size_of};

    /// Expected values come from the Windows SDK 10.0.26100 headers compiled
    /// with MSVC x64 (`cl /W3` with `sizeof`/`offsetof` dumps).
    #[test]
    fn struct_sizes_and_offsets() {
        assert_eq!(size_of::<WNDCLASSEXW>(), 80);
        assert_eq!(size_of::<MSG>(), 48);
        assert_eq!(size_of::<POINT>(), 8);
        assert_eq!(size_of::<RECT>(), 16);
        assert_eq!(size_of::<PAINTSTRUCT>(), 72);
        assert_eq!(size_of::<ICONINFO>(), 32);
        assert_eq!(size_of::<NOTIFYICONDATAW>(), 976);
        assert_eq!(size_of::<VARIANT>(), 24);
        assert_eq!(size_of::<TOOLINFOW>(), 72);
        assert_eq!(size_of::<LOGFONTW>(), 92);
        assert_eq!(size_of::<BITMAPINFOHEADER>(), 40);
        assert_eq!(size_of::<BITMAPINFO>(), 44);
        assert_eq!(size_of::<MINMAXINFO>(), 40);
        assert_eq!(size_of::<CREATESTRUCTW>(), 80);

        // TASKDIALOGCONFIG/TASKDIALOG_BUTTON are byte-packed in commctrl.h.
        assert_eq!(size_of::<TASKDIALOGCONFIG>(), 160);
        assert_eq!(align_of::<TASKDIALOGCONFIG>(), 1);
        assert_eq!(size_of::<TASKDIALOG_BUTTON>(), 12);
        assert_eq!(offset_of!(TASKDIALOGCONFIG, hwndParent), 4);
        assert_eq!(offset_of!(TASKDIALOGCONFIG, hMainIcon), 36);
        assert_eq!(offset_of!(TASKDIALOGCONFIG, cButtons), 60);
        assert_eq!(offset_of!(TASKDIALOGCONFIG, pszVerificationText), 92);
        assert_eq!(offset_of!(TASKDIALOGCONFIG, pfCallback), 140);
        assert_eq!(offset_of!(TASKDIALOGCONFIG, cxWidth), 156);
        assert_eq!(offset_of!(TASKDIALOG_BUTTON, pszButtonText), 4);

        assert_eq!(offset_of!(NOTIFYICONDATAW, hWnd), 8);
        assert_eq!(offset_of!(NOTIFYICONDATAW, hIcon), 32);
        assert_eq!(offset_of!(NOTIFYICONDATAW, szTip), 40);
        assert_eq!(offset_of!(NOTIFYICONDATAW, szInfo), 304);
        assert_eq!(offset_of!(NOTIFYICONDATAW, uVersion), 816);
        assert_eq!(offset_of!(NOTIFYICONDATAW, szInfoTitle), 820);
        assert_eq!(offset_of!(NOTIFYICONDATAW, dwInfoFlags), 948);
        assert_eq!(offset_of!(NOTIFYICONDATAW, hBalloonIcon), 968);

        assert_eq!(offset_of!(WNDCLASSEXW, lpfnWndProc), 8);
        assert_eq!(offset_of!(WNDCLASSEXW, hbrBackground), 48);
        assert_eq!(offset_of!(WNDCLASSEXW, hIconSm), 72);
        assert_eq!(offset_of!(MSG, wParam), 16);
        assert_eq!(offset_of!(MSG, pt), 36);
        assert_eq!(offset_of!(PAINTSTRUCT, rcPaint), 12);
        assert_eq!(offset_of!(PAINTSTRUCT, rgbReserved), 36);
        assert_eq!(offset_of!(ICONINFO, hbmMask), 16);
        assert_eq!(offset_of!(ICONINFO, hbmColor), 24);
        assert_eq!(offset_of!(VARIANT, data), 8);
        assert_eq!(offset_of!(TOOLINFOW, rect), 24);
        assert_eq!(offset_of!(TOOLINFOW, lpszText), 48);
        assert_eq!(offset_of!(TOOLINFOW, lParam), 56);
        assert_eq!(offset_of!(TOOLINFOW, lpReserved), 64);
        assert_eq!(offset_of!(LOGFONTW, lfWeight), 16);
        assert_eq!(offset_of!(LOGFONTW, lfFaceName), 28);
        assert_eq!(offset_of!(BITMAPINFO, bmiColors), 40);
        assert_eq!(offset_of!(CREATESTRUCTW, lpszClass), 64);
    }
}
