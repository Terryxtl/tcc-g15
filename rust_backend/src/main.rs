//! `tcc-backend` - troubleshooting CLI for the AWCC thermal interface.

use std::process::ExitCode;

use tcc_backend::hardware::{DetectHardware, CPU_FAN_INDEX, GPU_FAN_INDEX};
use tcc_backend::thermal::{AwccWmi, Thermal, ThermalMode};
use tcc_backend::trayicon;
use tcc_backend::wmi::Com;
use tcc_backend::{APP_NAME, APP_VERSION};

const USAGE: &str = "\
Thermal Control Center for Dell G15 - backend CLI

USAGE:
    tcc-backend info                     Show the thermal interface and a live sample
    tcc-backend detect                   Show the CPU and GPU model names
    tcc-backend probe                    Try documented thermal modes on this machine
    tcc-backend measure                  Sample temps/RPM of working modes under load
    tcc-backend icon-preview <file.bmp>  Render the tray icon to a BMP for inspection
                                         [gpu=<temp>] [cpu=<temp>] [g-mode]
                                         [size=<px>] [mode=max|gpu|cpu|both]
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str);

    match command {
        Some("info") => info(),
        Some("detect") => detect(),
        Some("probe") => probe(),
        Some("measure") => measure(),
        Some("icon-preview") => icon_preview(&args[1..]),
        _ => {
            println!("{APP_NAME} v{APP_VERSION}\n");
            print!("{USAGE}");
            ExitCode::from(if command.is_none() { 0 } else { 2 })
        }
    }
}

fn info() -> ExitCode {
    let _com = match Com::init(false) {
        Ok(com) => com,
        Err(error) => return fail(&format!("COM initialization failed: {error}")),
    };

    println!("{APP_NAME} v{APP_VERSION}");

    let awcc = match AwccWmi::connect() {
        Ok(awcc) => awcc,
        Err(error) => return fail(&format!("{error}")),
    };
    let thermal = Thermal::from_awcc(awcc);

    let fans = thermal.fans();
    println!("Fans reported by the driver: {}", fans.len());
    if fans.is_empty() {
        // Almost always a permissions problem (the app needs administrator
        // rights to reach the driver), so show what actually went wrong.
        println!("No fans found. Raw diagnostics:");
        for line in thermal.awcc().diagnose() {
            println!("  {line}");
        }
        println!("  (run this from an administrator console to rule permissions out)");
    }
    for (index, (fan_id, sensors)) in fans.iter().enumerate() {
        let label = match index {
            CPU_FAN_INDEX => "CPU",
            GPU_FAN_INDEX => "GPU",
            _ => "--",
        };
        let sensor_ids: Vec<String> = sensors.iter().map(|id| format!("0x{id:02X}")).collect();
        println!(
            "  fan {index} ({label}): id 0x{fan_id:02X}  temp {} C  rpm {}  sensors [{}]",
            reading(thermal.fan_related_temp(index)),
            reading(thermal.fan_rpm(index)),
            sensor_ids.join(", ")
        );
    }

    println!("All sensor temperatures: {:?}", thermal.all_temps());
    println!("All fan speeds (RPM):    {:?}", thermal.all_fan_rpm());
    println!(
        "Headline readings: GPU {} | CPU {}",
        reading(thermal.fan_related_temp(GPU_FAN_INDEX)),
        reading(thermal.fan_related_temp(CPU_FAN_INDEX))
    );
    ExitCode::SUCCESS
}

fn reading(value: Option<u32>) -> String {
    match value {
        Some(value) => value.to_string(),
        None => "n/a".to_string(),
    }
}

fn detect() -> ExitCode {
    let _com = match Com::init(false) {
        Ok(com) => com,
        Err(error) => return fail(&format!("COM initialization failed: {error}")),
    };
    let Some(detect) = DetectHardware::connect() else {
        return fail("Could not connect to the ROOT\\CIMV2 WMI namespace");
    };

    println!(
        "CPU: {}",
        detect.cpu_name().unwrap_or_else(|| "unknown".to_string())
    );
    match detect.gpu_name() {
        Some(gpu) => println!("GPU: {gpu}"),
        None => {
            // Fall back to whichever adapter WMI reports first.
            println!("GPU: unknown");
            return ExitCode::SUCCESS;
        }
    }
    ExitCode::SUCCESS
}

fn icon_preview(args: &[String]) -> ExitCode {
    let Some(path) = args.first() else {
        print!("{USAGE}");
        return ExitCode::from(2);
    };
    let number = |prefix: &str| {
        args.iter()
            .find_map(|arg| arg.strip_prefix(prefix))
            .and_then(|value| value.parse::<i32>().ok())
    };
    let temp = |prefix: &str| number(prefix).map(|value| value.max(0) as u32);
    let size = number("size=")
        .filter(|size| *size >= 8)
        .unwrap_or_else(trayicon::best_size);
    let mode = args
        .iter()
        .find_map(|arg| arg.strip_prefix("mode="))
        .and_then(trayicon::TrayIconMode::from_key)
        .unwrap_or(trayicon::TrayIconMode::Both);
    let spec = trayicon::spec_for(
        size,
        temp("gpu="),
        temp("cpu="),
        (72, 85),
        (85, 95),
        args.iter().any(|arg| arg == "g-mode"),
        mode,
    );

    let Some(icon) = trayicon::draw(&spec) else {
        return fail("Could not render the tray icon");
    };
    if let Err(error) = icon.write_bmp(std::path::Path::new(path), (0x40, 0x40, 0x40)) {
        return fail(&format!("Could not write {path}: {error}"));
    }
    println!("Wrote {}x{} tray icon to {path}", icon.width, icon.height);
    ExitCode::SUCCESS
}

fn probe() -> ExitCode {
    let _com = match Com::init(false) {
        Ok(com) => com,
        Err(error) => return fail(&format!("COM initialization failed: {error}")),
    };
    let awcc = match AwccWmi::connect() {
        Ok(awcc) => awcc,
        Err(error) => return fail(&format!("{error}")),
    };
    let thermal = Thermal::from_awcc(awcc);
    let awcc = thermal.awcc();

    let mut lines = Vec::new();
    let mut log = |line: String| {
        println!("{line}");
        lines.push(line);
    };

    log(format!(
        "probe on {}  fans={}",
        awcc.target_path(),
        thermal.fan_count()
    ));

    const MODES: &[(&str, u32)] = &[
        ("Custom", 0),
        ("Quiet", 150),
        ("Balanced", 151),
        ("Performance", 152),
        ("FullSpeed", 153),
        ("USTT_Balanced", 160),
        ("USTT_Performance", 161),
        ("USTT_Cool", 162),
        ("USTT_Quiet", 163),
        ("USTT_FullSpeed", 164),
        ("USTT_BatterySaver", 165),
        ("G_Mode", 171),
    ];

    log("\nThermal modes (Thermal_Control ApplyThermalMode):".into());
    let mut ok_modes = Vec::new();
    for (name, value) in MODES {
        let result = match awcc.apply_mode_value(*value) {
            Ok(0) => {
                ok_modes.push(*name);
                "OK".to_string()
            }
            Ok(code) => format!("returned {code}"),
            Err(error) => format!("FAIL {error}"),
        };
        log(format!("  {name:<22} {value:>3}  {result}"));
        std::thread::sleep(std::time::Duration::from_millis(400));
    }

    let restored = if awcc.apply_mode_value(151).ok() == Some(0) {
        Some("Balanced")
    } else if awcc.apply_mode_value(160).ok() == Some(0) {
        Some("USTT_Balanced")
    } else {
        None
    };
    log(format!(
        "\nRestored {}.",
        restored.unwrap_or("nothing (check the current mode)")
    ));

    log("\nFan RPM percent (Thermal_Information op 6):".into());
    for (index, (fan_id, _)) in thermal.fans().iter().enumerate() {
        let result = match awcc.fan_rpm_percent(*fan_id) {
            Some(value) => {
                format!("OK {value}")
            }
            None => "FAIL".into(),
        };
        log(format!("  fan {index} id 0x{fan_id:02X}  {result}"));
    }

    const READS: &[(&str, u32)] = &[
        ("Return_OverclockingReport", 0),
        ("SystemInformation", 0),
        ("PowerInformation", 0),
        ("ReadTotalofGPIOs", 0),
        ("ReadChassisColor", 0),
        ("ReadPlatformProperties", 0),
        ("GameShiftStatus", 0),
    ];
    log("\nOther read-only-ish methods (arg=0):".into());
    let mut ok_reads = Vec::new();
    for (name, arg) in READS {
        let result = match awcc.call_named(name, *arg) {
            Ok(value) => {
                ok_reads.push(*name);
                format!("OK ({value})")
            }
            Err(error) => format!("FAIL {error}"),
        };
        log(format!("  {name:<28} {result}"));
    }

    log(format!(
        "\nOK modes: {}\nOK reads: {}",
        if ok_modes.is_empty() {
            "(none)".into()
        } else {
            ok_modes.join(", ")
        },
        if ok_reads.is_empty() {
            "(none)".into()
        } else {
            ok_reads.join(", ")
        }
    ));

    let path = tcc_backend::logging::log_path()
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("probe.txt");
    if let Err(error) = std::fs::write(&path, lines.join("\r\n")) {
        eprintln!("could not write {}: {error}", path.display());
    } else {
        println!("Wrote {}", path.display());
    }
    ExitCode::SUCCESS
}

fn measure() -> ExitCode {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    let _com = match Com::init(false) {
        Ok(com) => com,
        Err(error) => return fail(&format!("COM initialization failed: {error}")),
    };
    let awcc = match AwccWmi::connect() {
        Ok(awcc) => awcc,
        Err(error) => return fail(&format!("{error}")),
    };
    let mut thermal = Thermal::from_awcc(awcc);
    if thermal.fan_count() < 2 {
        return fail("need CPU and GPU fans");
    }

    let burn = Arc::new(AtomicBool::new(true));
    let workers: Vec<_> = (0..std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4))
        .map(|_| {
            let burn = burn.clone();
            std::thread::spawn(move || {
                let mut x = 1u64;
                while burn.load(Ordering::Relaxed) {
                    x = x.wrapping_mul(6364136223846793005).wrapping_add(1);
                    std::hint::black_box(x);
                }
            })
        })
        .collect();

    let mut lines = Vec::new();
    let mut log = |line: String| {
        println!("{line}");
        lines.push(line);
    };
    log("CPU all-core load. Each mode: wait 10s, then 5 samples.".into());
    log(format!(
        "{:<22} {:>6} {:>6} {:>8} {:>8}",
        "mode", "cpu°C", "gpu°C", "cpu RPM", "gpu RPM"
    ));

    const MODES: &[(&str, u32)] = &[
        ("USTT_BatterySaver", 165),
        ("USTT_Quiet", 163),
        ("USTT_Balanced", 160),
        ("USTT_Performance", 161),
        ("G_Mode", 171),
    ];

    for (name, value) in MODES {
        if thermal.awcc().apply_mode_value(*value).ok() != Some(0) {
            log(format!("{name:<22} apply failed"));
            continue;
        }
        std::thread::sleep(Duration::from_secs(10));
        let mut cpu_t = Vec::new();
        let mut gpu_t = Vec::new();
        let mut cpu_r = Vec::new();
        let mut gpu_r = Vec::new();
        for _ in 0..5 {
            if let Some(v) = thermal.fan_related_temp(CPU_FAN_INDEX) {
                cpu_t.push(v);
            }
            if let Some(v) = thermal.fan_related_temp(GPU_FAN_INDEX) {
                gpu_t.push(v);
            }
            if let Some(v) = thermal.fan_rpm(CPU_FAN_INDEX) {
                cpu_r.push(v);
            }
            if let Some(v) = thermal.fan_rpm(GPU_FAN_INDEX) {
                gpu_r.push(v);
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        log(format!(
            "{:<22} {:>6} {:>6} {:>8} {:>8}",
            name,
            avg(&cpu_t),
            avg(&gpu_t),
            avg(&cpu_r),
            avg(&gpu_r)
        ));
    }

    burn.store(false, Ordering::Relaxed);
    for worker in workers {
        let _ = worker.join();
    }
    let restored = thermal.set_mode(ThermalMode::Balanced);
    if restored {
        log("Restored Balanced, load stopped.".into());
    } else {
        log("Could not restore Balanced; load stopped. Check the current mode.".into());
    }

    let path = tcc_backend::logging::log_path()
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("measure.txt");
    if let Err(error) = std::fs::write(&path, lines.join("\r\n")) {
        eprintln!("could not write {}: {error}", path.display());
    } else {
        println!("Wrote {}", path.display());
    }
    if restored {
        ExitCode::SUCCESS
    } else {
        fail("Could not restore Balanced mode after measurement; CPU load stopped")
    }
}

fn avg(values: &[u32]) -> String {
    if values.is_empty() {
        return "-".into();
    }
    let sum: u32 = values.iter().sum();
    (sum / values.len() as u32).to_string()
}

fn fail(message: &str) -> ExitCode {
    eprintln!("error: {message}");
    ExitCode::from(1)
}
