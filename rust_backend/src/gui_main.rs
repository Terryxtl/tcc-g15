//! `tcc-g15` - the GUI application.
#![windows_subsystem = "windows"]

fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(tcc_backend::gui::run() as u8)
}
