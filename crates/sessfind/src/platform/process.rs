use anyhow::Result;

/// Resume a session by executing the given command.
///
/// On Unix this replaces the current process via `exec()`.
/// On Windows it spawns the child and exits with its exit code.
pub fn exec_resume(args: &[String], cwd: Option<&str>) -> Result<()> {
    let mut command = std::process::Command::new(&args[0]);
    command.args(&args[1..]);

    if let Some(dir) = cwd {
        let path = std::path::Path::new(dir);
        if path.is_dir() {
            command.current_dir(path);
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = command.exec();
        Err(anyhow::anyhow!("Failed to exec {}: {err}", args[0]))
    }

    #[cfg(windows)]
    {
        let status = command
            .status()
            .map_err(|e| anyhow::anyhow!("Failed to run {}: {e}", args[0]))?;
        std::process::exit(status.code().unwrap_or(1));
    }
}
