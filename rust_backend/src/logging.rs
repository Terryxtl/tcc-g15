//! Tiny append-only log file.
//!
//! The GUI is a windowed application, so its `stdout` goes nowhere unless it is
//! started from a console. Everything worth reporting therefore also lands in
//! `%LOCALAPPDATA%\tcc-g15\log.txt`, which users can attach to bug reports.

use std::fmt::Display;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

use crate::win32::SYSTEMTIME;

const MAX_LOG_BYTES: u64 = 256 * 1024;

static PATH: OnceLock<PathBuf> = OnceLock::new();

fn path() -> &'static PathBuf {
    PATH.get_or_init(|| {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let directory = base.join("tcc-g15");
        let _ = std::fs::create_dir_all(&directory);
        let file = directory.join("log.txt");
        if std::fs::metadata(&file)
            .map(|meta| meta.len() > MAX_LOG_BYTES)
            .unwrap_or(false)
        {
            let _ = std::fs::remove_file(&file);
        }
        file
    })
}

/// Where the log is written.
pub fn log_path() -> &'static PathBuf {
    path()
}

pub fn line(message: impl Display) {
    let mut timestamp = SYSTEMTIME::default();
    unsafe { crate::win32::GetLocalTime(&mut timestamp) };
    let text = format!("[{}] {message}\r\n", timestamp.as_string());

    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path())
    {
        let _ = file.write_all(text.as_bytes());
    }
}

/// Logs to the file and to `stdout` (useful when started from a console).
#[macro_export]
macro_rules! logln {
    ($($arg:tt)*) => {{
        let message = format!($($arg)*);
        println!("{message}");
        $crate::logging::line(&message);
    }};
}
