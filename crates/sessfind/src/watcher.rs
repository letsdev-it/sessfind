use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use notify_debouncer_mini::{DebouncedEventKind, new_debouncer};

use crate::config;
use crate::indexer::engine::IndexEngine;
use crate::semantic;

const DEBOUNCE_SECS: u64 = 5;

/// Run the file watcher in the foreground, re-indexing on session changes.
pub fn run() -> Result<()> {
    let dirs = watch_dirs();
    if dirs.is_empty() {
        eprintln!("No session directories found to watch.");
        return Ok(());
    }

    let (tx, rx) = mpsc::channel();
    let mut debouncer = new_debouncer(Duration::from_secs(DEBOUNCE_SECS), tx)?;

    for (label, path, recursive) in &dirs {
        let mode = if *recursive {
            notify::RecursiveMode::Recursive
        } else {
            notify::RecursiveMode::NonRecursive
        };
        match debouncer.watcher().watch(path, mode) {
            Ok(()) => eprintln!("Watching {label}: {}", path.display()),
            Err(e) => eprintln!("Warning: cannot watch {label} ({}): {e}", path.display()),
        }
    }

    eprintln!("Watcher running. Press Ctrl+C to stop.");

    // Initial index on startup
    if let Err(e) = run_index() {
        eprintln!("Initial index error: {e}");
    }

    loop {
        match rx.recv() {
            Ok(Ok(events)) => {
                let dominated_by_access_only = events
                    .iter()
                    .all(|e| e.kind == DebouncedEventKind::AnyContinuous);
                if dominated_by_access_only {
                    continue;
                }
                eprintln!("Change detected, re-indexing...");
                match run_index() {
                    Ok(indexed) => {
                        if indexed > 0 {
                            eprintln!("Applied {indexed} catalog change(s).");
                        } else {
                            eprintln!("Already up to date.");
                        }
                    }
                    Err(e) => eprintln!("Index error: {e}"),
                }
            }
            Ok(Err(errs)) => {
                eprintln!("Watch error: {errs}");
            }
            Err(_) => {
                // Channel closed — watcher dropped
                break;
            }
        }
    }

    Ok(())
}

/// Run reconciliation for all sources. Returns the number of catalog changes.
fn run_index() -> Result<usize> {
    let data_dir = config::data_dir();
    let engine = IndexEngine::open(&data_dir)?;

    let sources = crate::sources::all_sources();

    let mut total_changes = 0usize;
    for src in &sources {
        match engine.index_source(src.as_ref(), false) {
            Ok(stats) => {
                total_changes +=
                    stats.new_sessions + stats.updated_sessions + stats.removed_sessions
            }
            Err(e) => eprintln!("Warning: failed to index {}: {e}", src.name()),
        }
    }

    if semantic::is_available()
        && total_changes > 0
        && let Err(error) = semantic::trigger_index()
    {
        eprintln!("Warning: semantic indexing failed: {error}");
    }

    Ok(total_changes)
}

/// Collect directories to watch (only those that exist on disk).
fn watch_dirs() -> Vec<(&'static str, PathBuf, bool)> {
    let candidates: Vec<(&str, PathBuf, bool)> = vec![
        ("claude", config::claude_projects_dir(), true),
        ("copilot", config::copilot_session_dir(), true),
        (
            "opencode",
            config::opencode_db_path()
                .parent()
                .unwrap_or(&config::opencode_db_path())
                .to_path_buf(),
            false,
        ),
        ("cursor", config::cursor_projects_dir(), true),
        ("codex", config::codex_sessions_dir(), true),
    ];

    candidates
        .into_iter()
        .filter(|(_, path, _)| path.exists())
        .collect()
}
