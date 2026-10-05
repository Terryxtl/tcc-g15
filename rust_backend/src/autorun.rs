//! "Run at sign-in" support, implemented with a Windows Task Scheduler task.
//!
//! Port of `autorunTask()` from `GUI/AppGUI.py`, but the task XML lives inside
//! the executable instead of next to it, and the patched copy goes to `%TEMP%`
//! so the installation directory never has to be writable.

use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Task definition shipped with the app (`tcc_g15_task.xml`).
const TASK_XML: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tcc_g15_task.xml"));
const EXE_PLACEHOLDER: &str = "<!--EXE_FILE_PATH-->";
const TASK_NAME: &str = "TCC_G15";

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutorunAction {
    Enable,
    Disable,
}

#[derive(Debug)]
pub enum AutorunError {
    Io(std::io::Error),
    /// The executable is not a `.exe` file (e.g. running from `cargo run`),
    /// which is what the Python app reported as `-100`.
    NotAnExecutable,
}

impl std::fmt::Display for AutorunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AutorunError::Io(error) => write!(f, "{error}"),
            AutorunError::NotAnExecutable => write!(f, "not an executable (-100)"),
        }
    }
}

impl From<std::io::Error> for AutorunError {
    fn from(error: std::io::Error) -> Self {
        AutorunError::Io(error)
    }
}

/// Enables or disables the scheduled task. Returns the `schtasks` exit code.
pub fn apply(action: AutorunAction) -> Result<i32, AutorunError> {
    match action {
        AutorunAction::Enable => enable(),
        AutorunAction::Disable => disable(),
    }
}

fn enable() -> Result<i32, AutorunError> {
    let exe = std::env::current_exe()?;
    if exe.extension().map(|ext| ext != "exe").unwrap_or(true) {
        return Err(AutorunError::NotAnExecutable);
    }

    let xml = TASK_XML.replace(EXE_PLACEHOLDER, &exe.to_string_lossy());
    let task_file = task_file_path();
    // schtasks /create /xml requires a Unicode (UTF-16 LE) file.
    std::fs::write(&task_file, utf16_le_bytes(&xml))?;

    // Drop a previously registered task first, then register the fresh one.
    let _ = schtasks(&["/delete", "/tn", TASK_NAME, "/f"]);
    let code = schtasks(&[
        "/create",
        "/xml",
        &task_file.to_string_lossy(),
        "/tn",
        TASK_NAME,
    ]);
    let _ = std::fs::remove_file(&task_file);
    Ok(code)
}

fn disable() -> Result<i32, AutorunError> {
    Ok(schtasks(&["/delete", "/tn", TASK_NAME, "/f"]))
}

fn task_file_path() -> PathBuf {
    std::env::temp_dir().join(format!("tcc-g15-task-{}.xml", std::process::id()))
}

fn schtasks(args: &[&str]) -> i32 {
    Command::new("schtasks")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map(|status| status.code().unwrap_or(-1))
        .unwrap_or(-1)
}

/// Renders the task XML with `exe` substituted, for tests and diagnostics.
pub fn render_task_xml(exe: &Path) -> String {
    TASK_XML.replace(EXE_PLACEHOLDER, &exe.to_string_lossy())
}

fn utf16_le_bytes(text: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(2 + text.len() * 2);
    bytes.extend_from_slice(&[0xFF, 0xFE]);
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_is_substituted_once() {
        let xml = render_task_xml(Path::new(
            r"C:\Program Files\Thermal Control Center\tcc-g15.exe",
        ));
        assert!(!xml.contains(EXE_PLACEHOLDER));
        assert!(xml.contains(r#""C:\Program Files\Thermal Control Center\tcc-g15.exe""#));
        assert!(xml.contains("<Arguments>--minimized</Arguments>"));
        assert!(xml.contains("<RunLevel>HighestAvailable</RunLevel>"));
    }

    #[test]
    fn rendered_xml_is_well_formed_enough_to_parse() {
        let xml = render_task_xml(Path::new(r"C:\tcc-g15.exe"));
        assert!(xml.starts_with("<?xml"));
        assert!(xml.contains("encoding=\"UTF-16\""));
        assert_eq!(xml.matches("<Task ").count(), 1);
        assert_eq!(xml.matches("</Task>").count(), 1);
    }

    #[test]
    fn task_file_bytes_are_utf16_le_with_bom() {
        let bytes = utf16_le_bytes("<?xml encoding=\"UTF-16\"?>");
        assert_eq!(&bytes[..2], &[0xFF, 0xFE]);
        assert_eq!(bytes[2], b'<');
        assert_eq!(bytes[3], 0);
    }
}
