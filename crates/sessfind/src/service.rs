use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};

const LABEL: &str = "dev.lets.sessfind.watch";

pub fn install() -> Result<()> {
    if cfg!(target_os = "macos") {
        install_launchd()
    } else {
        install_systemd()
    }
}

pub fn uninstall() -> Result<()> {
    if cfg!(target_os = "macos") {
        uninstall_launchd()
    } else {
        uninstall_systemd()
    }
}

pub fn status() -> Result<()> {
    if cfg!(target_os = "macos") {
        status_launchd()
    } else {
        status_systemd()
    }
}

fn bin_path() -> Result<PathBuf> {
    std::env::current_exe().context("cannot determine sessfind binary path")
}

// ── macOS launchd ──────────────────────────────────────────────

fn plist_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist"))
}

fn plist_contents(bin: &PathBuf) -> String {
    let log_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Library/Logs/sessfind-watch.log");

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{bin}</string>
        <string>watch</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{log}</string>
    <key>StandardErrorPath</key>
    <string>{log}</string>
</dict>
</plist>
"#,
        bin = bin.display(),
        log = log_path.display(),
    )
}

fn install_launchd() -> Result<()> {
    let bin = bin_path()?;
    let plist = plist_path();

    if let Some(parent) = plist.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&plist, plist_contents(&bin))?;
    eprintln!("Created {}", plist.display());

    let uid = unsafe { libc::getuid() };
    let target = format!("gui/{uid}");

    // bootout first (ignore errors if not loaded)
    let _ = Command::new("launchctl")
        .args(["bootout", &format!("{target}/{LABEL}")])
        .output();

    let out = Command::new("launchctl")
        .args(["bootstrap", &target, &plist.to_string_lossy()])
        .output()
        .context("failed to run launchctl bootstrap")?;

    if out.status.success() {
        eprintln!("Service installed and started.");
        eprintln!("Logs: ~/Library/Logs/sessfind-watch.log");
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        eprintln!("launchctl bootstrap failed: {stderr}");
    }
    Ok(())
}

fn uninstall_launchd() -> Result<()> {
    let plist = plist_path();
    let uid = unsafe { libc::getuid() };
    let target = format!("gui/{uid}/{LABEL}");

    let _ = Command::new("launchctl")
        .args(["bootout", &target])
        .output();

    if plist.exists() {
        std::fs::remove_file(&plist)?;
        eprintln!("Removed {}", plist.display());
    }
    eprintln!("Service uninstalled.");
    Ok(())
}

fn status_launchd() -> Result<()> {
    let uid = unsafe { libc::getuid() };
    let target = format!("gui/{uid}/{LABEL}");

    let out = Command::new("launchctl")
        .args(["print", &target])
        .output()
        .context("failed to run launchctl")?;

    if out.status.success() {
        let stdout = String::from_utf8_lossy(&out.stdout);
        // Extract state and PID from launchctl print output
        let mut pid = None;
        let mut state = None;
        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("pid = ") {
                pid = trimmed.strip_prefix("pid = ").map(|s| s.to_string());
            }
            if trimmed.starts_with("state = ") {
                state = trimmed.strip_prefix("state = ").map(|s| s.to_string());
            }
        }
        let state_str = state.as_deref().unwrap_or("unknown");
        match pid {
            Some(p) => eprintln!("Service is running (pid {p}, state: {state_str})."),
            None => eprintln!("Service is loaded but not running (state: {state_str})."),
        }
    } else {
        eprintln!("Service is not installed.");
    }
    Ok(())
}

// ── Linux systemd ──────────────────────────────────────────────

const UNIT_NAME: &str = "sessfind-watch.service";

fn unit_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config/systemd/user")
        .join(UNIT_NAME)
}

fn unit_contents(bin: &PathBuf) -> String {
    format!(
        r#"[Unit]
Description=sessfind watch — automatic session indexing

[Service]
Type=simple
ExecStart={bin} watch
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
"#,
        bin = bin.display(),
    )
}

fn install_systemd() -> Result<()> {
    let bin = bin_path()?;
    let unit = unit_path();

    if let Some(parent) = unit.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&unit, unit_contents(&bin))?;
    eprintln!("Created {}", unit.display());

    let out = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .output()
        .context("failed to run systemctl")?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        eprintln!("Warning: daemon-reload failed: {stderr}");
    }

    let out = Command::new("systemctl")
        .args(["--user", "enable", "--now", UNIT_NAME])
        .output()
        .context("failed to run systemctl enable")?;
    if out.status.success() {
        eprintln!("Service installed and started.");
        eprintln!("View logs: journalctl --user -u {UNIT_NAME} -f");
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        eprintln!("systemctl enable failed: {stderr}");
    }
    Ok(())
}

fn uninstall_systemd() -> Result<()> {
    let unit = unit_path();

    let _ = Command::new("systemctl")
        .args(["--user", "disable", "--now", UNIT_NAME])
        .output();

    if unit.exists() {
        std::fs::remove_file(&unit)?;
        eprintln!("Removed {}", unit.display());
    }

    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .output();

    eprintln!("Service uninstalled.");
    Ok(())
}

fn status_systemd() -> Result<()> {
    let out = Command::new("systemctl")
        .args(["--user", "is-active", UNIT_NAME])
        .output()
        .context("failed to run systemctl")?;

    let state = String::from_utf8_lossy(&out.stdout).trim().to_string();
    match state.as_str() {
        "active" => eprintln!("Service is running."),
        "inactive" => eprintln!("Service is installed but not running."),
        "failed" => eprintln!("Service has failed. Check: journalctl --user -u {UNIT_NAME}"),
        _ => {
            if unit_path().exists() {
                eprintln!("Service state: {state}");
            } else {
                eprintln!("Service is not installed.");
            }
        }
    }
    Ok(())
}
