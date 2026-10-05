//! Compiles the resource script (application icon + manifest) when the Windows
//! SDK resource compiler is available. The build still succeeds without it:
//! the app then runs unthemed and without an embedded icon.

use std::path::{Path, PathBuf};
use std::process::Command;

const RC_FILE: &str = "rust_backend/resources/tcc-g15.rc";

fn main() {
    for file in [
        RC_FILE,
        "rust_backend/resources/tcc-g15.manifest",
        "icons/gaugeIcon.ico",
        "tcc_g15_task.xml",
    ] {
        println!("cargo:rerun-if-changed={file}");
    }

    let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") else {
        return;
    };
    let manifest_dir = PathBuf::from(manifest_dir);
    let Ok(out_dir) = std::env::var("OUT_DIR") else {
        return;
    };
    let resource = Path::new(&out_dir).join("tcc-g15.res");

    let Some(compiler) = find_resource_compiler() else {
        println!(
            "cargo:warning=rc.exe not found - building without the application manifest and icon"
        );
        return;
    };
    if !manifest_dir.join(RC_FILE).exists() {
        return;
    }

    let result = Command::new(&compiler)
        .current_dir(&manifest_dir)
        .arg("/nologo")
        .arg("/fo")
        .arg(&resource)
        .arg("/I")
        .arg(&manifest_dir)
        .arg("/I")
        .arg(manifest_dir.join("rust_backend/resources"))
        .arg(RC_FILE)
        .output();

    match result {
        Ok(output) if output.status.success() => {
            println!("cargo:rustc-link-arg-bin=tcc-g15={}", resource.display());
        }
        Ok(output) => {
            let detail = String::from_utf8_lossy(&output.stdout);
            let detail = detail.trim();
            println!(
                "cargo:warning=rc.exe failed ({}): {detail}; continuing without resources",
                output.status
            );
        }
        Err(error) => {
            println!("cargo:warning=could not run rc.exe ({error}); continuing without resources");
        }
    }
}

/// Looks for `rc.exe` on `PATH`, then in the newest Windows SDK under the
/// location recorded in the registry.
fn find_resource_compiler() -> Option<PathBuf> {
    if Command::new("rc.exe").arg("/?").output().is_ok() {
        return Some(PathBuf::from("rc.exe"));
    }

    let output = Command::new("reg")
        .args([
            "query",
            r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Microsoft SDKs\Windows\v10.0",
            "/v",
            "InstallationFolder",
        ])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let root = text
        .lines()
        .find_map(|line| line.split("REG_SZ").nth(1))
        .map(str::trim)
        .map(PathBuf::from)?;

    let mut versions: Vec<PathBuf> = std::fs::read_dir(root.join("bin"))
        .ok()?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir())
        .collect();
    // Newest SDK version last.
    versions.sort();
    for version in versions.into_iter().rev() {
        let candidate = version.join("x64").join("rc.exe");
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}
