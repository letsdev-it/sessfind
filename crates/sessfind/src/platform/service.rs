#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};

pub fn install() -> Result<()> {
    platform::install()
}

pub fn uninstall() -> Result<()> {
    platform::uninstall()
}

pub fn status() -> Result<()> {
    platform::status()
}

fn bin_path() -> Result<PathBuf> {
    std::env::current_exe().context("cannot determine sessfind binary path")
}

// ── macOS launchd ──────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod platform {
    use super::*;

    const LABEL: &str = "dev.lets.sessfind.watch";

    fn plist_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Library/LaunchAgents")
            .join(format!("{LABEL}.plist"))
    }

    fn plist_contents(bin: &Path) -> String {
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

    pub fn install() -> Result<()> {
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

    pub fn uninstall() -> Result<()> {
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

    pub fn status() -> Result<()> {
        let uid = unsafe { libc::getuid() };
        let target = format!("gui/{uid}/{LABEL}");

        let out = Command::new("launchctl")
            .args(["print", &target])
            .output()
            .context("failed to run launchctl")?;

        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
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
}

// ── Linux systemd ──────────────────────────────────────────────

#[cfg(all(unix, not(target_os = "macos")))]
mod platform {
    use super::*;

    const UNIT_NAME: &str = "sessfind-watch.service";

    fn unit_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config/systemd/user")
            .join(UNIT_NAME)
    }

    fn unit_contents(bin: &Path) -> String {
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

    pub fn install() -> Result<()> {
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

    pub fn uninstall() -> Result<()> {
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

    pub fn status() -> Result<()> {
        let out = Command::new("systemctl")
            .args(["--user", "is-active", UNIT_NAME])
            .output()
            .context("failed to run systemctl")?;

        let state = String::from_utf8_lossy(&out.stdout).trim().to_string();
        match state.as_str() {
            "active" => eprintln!("Service is running."),
            "inactive" => eprintln!("Service is installed but not running."),
            "failed" => {
                eprintln!("Service has failed. Check: journalctl --user -u {UNIT_NAME}")
            }
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
}

// ── Windows Task Scheduler ─────────────────────────────────────

#[cfg(windows)]
mod platform {
    use super::*;

    const TASK_NAME: &str = "sessfind-watch";

    pub fn install() -> Result<()> {
        let bin = bin_path()?;
        let bin_str = bin.to_string_lossy();

        // Remove any existing task first (ignore errors)
        let _ = Command::new("schtasks")
            .args(["/Delete", "/TN", TASK_NAME, "/F"])
            .output();

        let out = Command::new("schtasks")
            .args([
                "/Create",
                "/TN",
                TASK_NAME,
                "/TR",
                &format!("\"{}\" watch", bin_str),
                "/SC",
                "ONLOGON",
                "/RL",
                "LIMITED",
                "/F",
            ])
            .output()
            .context("failed to run schtasks /Create")?;

        if out.status.success() {
            eprintln!("Scheduled task '{TASK_NAME}' created (runs at logon).");
        } else {
            let stderr = String::from_utf8_lossy(&out.stderr);
            eprintln!("schtasks /Create failed: {stderr}");
            return Ok(());
        }

        // Start the task immediately
        let out = Command::new("schtasks")
            .args(["/Run", "/TN", TASK_NAME])
            .output()
            .context("failed to run schtasks /Run")?;

        if out.status.success() {
            eprintln!("Service started.");
        } else {
            let stderr = String::from_utf8_lossy(&out.stderr);
            eprintln!("Warning: could not start task immediately: {stderr}");
            eprintln!("The task will start at next logon.");
        }

        Ok(())
    }

    pub fn uninstall() -> Result<()> {
        let out = Command::new("schtasks")
            .args(["/Delete", "/TN", TASK_NAME, "/F"])
            .output()
            .context("failed to run schtasks /Delete")?;

        if out.status.success() {
            eprintln!("Scheduled task '{TASK_NAME}' removed.");
        } else {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if stderr.contains("does not exist") || stderr.contains("nie istnieje") {
                eprintln!("Service is not installed.");
            } else {
                eprintln!("schtasks /Delete failed: {stderr}");
            }
        }
        Ok(())
    }

    pub fn status() -> Result<()> {
        let out = Command::new("schtasks")
            .args(["/Query", "/TN", TASK_NAME, "/FO", "LIST"])
            .output()
            .context("failed to run schtasks /Query")?;

        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let status_line = stdout
                .lines()
                .find(|l| l.trim_start().starts_with("Status:"))
                .map(|l| l.trim().trim_start_matches("Status:").trim())
                .unwrap_or("unknown");

            match status_line {
                "Running" => eprintln!("Service is running."),
                "Ready" => eprintln!("Service is installed (will start at next logon)."),
                "Disabled" => eprintln!("Service is installed but disabled."),
                other => eprintln!("Service state: {other}"),
            }
        } else {
            eprintln!("Service is not installed.");
        }
        Ok(())
    }
}
