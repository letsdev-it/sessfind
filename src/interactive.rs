use skim::prelude::*;
use std::io::Cursor;

use crate::indexer::engine::IndexEngine;
use crate::models::SearchResult;

fn truncate_end(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s.to_string()
    } else {
        let truncated: String = chars[..max - 3].iter().collect();
        format!("{truncated}...")
    }
}

fn truncate_start(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s.to_string()
    } else {
        let skip = chars.len() - (max - 3);
        let truncated: String = chars[skip..].iter().collect();
        format!("...{truncated}")
    }
}

/// Format a SearchResult as a single line for skim
fn format_item(r: &SearchResult) -> String {
    let date = r.timestamp.format("%Y-%m-%d");
    let source = r.source.as_str();
    let project = truncate_start(&r.project, 30);
    let preview = r.snippet.replace('\n', " ");
    let preview = truncate_end(&preview, 80);
    format!(
        "{:<10} {:<30} {:<12} {}",
        source, project, date, preview
    )
}

pub fn run_interactive(engine: &IndexEngine) -> anyhow::Result<()> {
    let chunks = engine.list_all_chunks()?;
    if chunks.is_empty() {
        eprintln!("No indexed sessions. Run `session-seek index` first.");
        return Ok(());
    }

    // Deduplicate by session_id, keeping most recent chunk per session
    let mut seen = std::collections::HashSet::new();
    let unique_chunks: Vec<&SearchResult> = chunks
        .iter()
        .filter(|c| seen.insert(c.session_id.clone()))
        .collect();

    let lines: Vec<String> = unique_chunks.iter().map(|c| format_item(c)).collect();
    let input = lines.join("\n");

    let options = SkimOptionsBuilder::default()
        .height("100%".to_string())
        .multi(false)
        .prompt("search> ".to_string())
        .header(format!(
            "{:<10} {:<30} {:<12} {}",
            "Source", "Project", "Date", "Preview"
        ))
        .reverse(true)
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build skim options: {e}"))?;

    let item_reader = SkimItemReader::default();
    let items = item_reader.of_bufread(Box::new(Cursor::new(input)));

    let output = Skim::run_with(options, Some(items));

    match output {
        Ok(out) if !out.is_abort => {
            if let Some(selected) = out.selected_items.first() {
                let text = selected.output().to_string();
                // Find which chunk this corresponds to by matching the formatted line
                if let Some(idx) = lines.iter().position(|l| *l == text) {
                    if let Some(chunk) = unique_chunks.get(idx) {
                        show_session(engine, &chunk.session_id)?;
                    }
                }
            }
        }
        _ => {}
    }

    Ok(())
}

fn show_session(engine: &IndexEngine, session_id: &str) -> anyhow::Result<()> {
    let chunks = engine.get_session_chunks(session_id)?;
    if chunks.is_empty() {
        return Ok(());
    }

    let first = &chunks[0];
    println!(
        "\x1b[1mSession:\x1b[0m {} \x1b[90m({})\x1b[0m",
        first.session_id,
        first.source.as_str()
    );
    println!("\x1b[1mProject:\x1b[0m {}", first.project);
    println!(
        "\x1b[1mDate:\x1b[0m    {}",
        first.timestamp.format("%Y-%m-%d %H:%M")
    );
    if let Some(title) = &first.title {
        println!("\x1b[1mTitle:\x1b[0m   {title}");
    }
    println!("{}", "-".repeat(80));
    println!();

    for chunk in &chunks {
        for line in chunk.snippet.lines() {
            if line.starts_with("USER:") {
                println!("\x1b[32m{}\x1b[0m", line);
            } else if line.starts_with("ASSISTANT:") {
                println!("\x1b[34m{}\x1b[0m", line);
            } else if line.starts_with("[tools:") {
                println!("\x1b[90m{}\x1b[0m", line);
            } else {
                println!("{}", line);
            }
        }
        println!();
    }

    Ok(())
}
