use std::path::PathBuf;

use anyhow::Result;
use serde::Deserialize;
use sessfind_common::{SearchParams, SearchResult};

const PLUGIN_NAME: &str = "sessfind-semantic";

/// Find the sessfind-semantic binary: first PATH, then next to our own binary.
fn find_plugin() -> Option<PathBuf> {
    if let Ok(path) = which::which(PLUGIN_NAME) {
        return Some(path);
    }
    if let Ok(self_path) = std::env::current_exe()
        && let Some(dir) = self_path.parent()
    {
        let name = format!("{PLUGIN_NAME}{}", std::env::consts::EXE_SUFFIX);
        let sibling = dir.join(name);
        if sibling.exists() {
            return Some(sibling);
        }
    }
    None
}

/// Check if sessfind-semantic plugin is installed.
pub fn is_available() -> bool {
    find_plugin().is_some()
}

/// Run semantic search via the plugin subprocess.
pub fn search(params: &SearchParams) -> Result<Vec<SearchResult>> {
    let bin = find_plugin().ok_or_else(|| anyhow::anyhow!("sessfind-semantic not found"))?;

    let mut cmd = std::process::Command::new(bin);
    cmd.arg("search")
        .arg(&params.query)
        .arg("--limit")
        .arg(params.limit.to_string());

    if let Some(ref source) = params.source {
        cmd.arg("--source").arg(source);
    }
    if let Some(ref project) = params.project {
        cmd.arg("--project").arg(project);
    }
    if let Some(ref after) = params.after {
        cmd.arg("--after").arg(after.format("%Y-%m-%d").to_string());
    }
    if let Some(ref before) = params.before {
        cmd.arg("--before")
            .arg(before.format("%Y-%m-%d").to_string());
    }

    let output = cmd.output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("sessfind-semantic search failed: {stderr}");
    }

    let stdout = String::from_utf8(output.stdout)?;
    let results: Vec<SearchResult> = serde_json::from_str(&stdout)?;
    Ok(results)
}

/// Trigger semantic indexing via the plugin.
pub fn trigger_index() -> Result<()> {
    let bin = find_plugin().ok_or_else(|| anyhow::anyhow!("sessfind-semantic not found"))?;

    let status = std::process::Command::new(bin).arg("index").status()?;
    if !status.success() {
        anyhow::bail!("sessfind-semantic index failed");
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct SemanticStatus {
    pub installed: bool,
    pub indexed_chunks: usize,
    pub model: String,
}

/// Get plugin status.
pub fn status() -> Result<SemanticStatus> {
    let bin = find_plugin().ok_or_else(|| anyhow::anyhow!("sessfind-semantic not found"))?;

    let output = std::process::Command::new(bin).arg("status").output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("sessfind-semantic status failed: {stderr}");
    }
    let stdout = String::from_utf8(output.stdout)?;
    let status: SemanticStatus = serde_json::from_str(&stdout)?;
    Ok(status)
}
