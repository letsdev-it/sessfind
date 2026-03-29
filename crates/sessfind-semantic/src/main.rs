mod embedder;
mod store;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "sessfind-semantic",
    about = "Semantic search plugin for sessfind"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build semantic index from sessfind chunks
    Index {
        /// Force re-index all chunks
        #[arg(long)]
        force: bool,
    },
    /// Search using semantic similarity
    Search {
        /// Search query
        query: String,
        /// Filter by source (claude, opencode, copilot)
        #[arg(long, short = 's')]
        source: Option<String>,
        /// Filter by project name (substring match)
        #[arg(long, short = 'p')]
        project: Option<String>,
        /// Only results after this date (YYYY-MM-DD)
        #[arg(long)]
        after: Option<String>,
        /// Only results before this date (YYYY-MM-DD)
        #[arg(long)]
        before: Option<String>,
        /// Max results
        #[arg(long, short = 'n', default_value = "10")]
        limit: usize,
    },
    /// Show plugin status
    Status,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Index { force } => cmd_index(force),
        Commands::Search {
            query,
            source,
            project,
            after,
            before,
            limit,
        } => cmd_search(query, source, project, after, before, limit),
        Commands::Status => cmd_status(),
    }
}

/// Find the `sessfind` binary: first check PATH, then look next to our own binary.
fn find_sessfind() -> Result<std::path::PathBuf> {
    if let Ok(path) = which::which("sessfind") {
        return Ok(path);
    }
    // Look next to our own binary (common in dev builds: target/debug/)
    if let Ok(self_path) = std::env::current_exe()
        && let Some(dir) = self_path.parent()
    {
        let sibling = dir.join("sessfind");
        if sibling.exists() {
            return Ok(sibling);
        }
    }
    anyhow::bail!(
        "Cannot find `sessfind` binary. Make sure it's in your PATH or in the same directory as sessfind-semantic."
    )
}

fn cmd_index(force: bool) -> Result<()> {
    use std::io::BufRead;
    use std::process::Command;

    let data_dir = sessfind_common::data_dir();
    let db_path = data_dir.join("semantic.db");

    eprintln!("Loading embedding model...");
    let embedder = embedder::Embedder::new()?;
    let mut store = store::SemanticStore::open(&db_path)?;

    // Get chunks from sessfind dump-chunks
    let sessfind_bin = find_sessfind()?;
    eprintln!("Reading chunks from sessfind...");
    let output = Command::new(&sessfind_bin)
        .arg("dump-chunks")
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    let stdout = output
        .stdout
        .ok_or_else(|| anyhow::anyhow!("Failed to capture stdout"))?;
    let reader = std::io::BufReader::new(stdout);

    let mut chunks: Vec<sessfind_common::DumpChunk> = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.is_empty() {
            continue;
        }
        let chunk: sessfind_common::DumpChunk = serde_json::from_str(&line)?;
        if !force && store.is_chunk_indexed(&chunk.chunk_id) {
            continue;
        }
        // Skip low-content chunks (mostly XML tags, slash commands, etc.)
        if meaningful_text_len(&chunk.text) < 100 {
            continue;
        }
        chunks.push(chunk);
    }

    if chunks.is_empty() {
        eprintln!("Semantic index is up to date.");
        return Ok(());
    }

    eprintln!("Embedding {} new chunks...", chunks.len());
    let batch_size = 64;
    let total = chunks.len();
    for (i, batch) in chunks.chunks(batch_size).enumerate() {
        let texts: Vec<String> = batch
            .iter()
            .map(|c| {
                let mut enriched = String::new();
                // Add project name for context
                if let Some(name) = c.project.rsplit('/').next()
                    && !name.is_empty()
                {
                    enriched.push_str("Project: ");
                    enriched.push_str(name);
                    enriched.push('\n');
                }
                // Add title for context
                if let Some(ref title) = c.title {
                    enriched.push_str("Title: ");
                    enriched.push_str(title);
                    enriched.push('\n');
                }
                enriched.push_str(&c.text);
                enriched
            })
            .collect();
        let embeddings = embedder.embed_passages(&texts)?;

        for (chunk, embedding) in batch.iter().zip(embeddings.iter()) {
            store.insert(chunk, embedding)?;
        }

        let done = ((i + 1) * batch_size).min(total);
        eprint!("\r  Embedded {done}/{total} chunks...");
    }
    eprintln!("\nDone. Semantic index: {} total chunks.", store.count()?);
    Ok(())
}

fn cmd_search(
    query: String,
    source: Option<String>,
    project: Option<String>,
    after: Option<String>,
    before: Option<String>,
    limit: usize,
) -> Result<()> {
    use chrono::{NaiveDate, TimeZone, Utc};

    let data_dir = sessfind_common::data_dir();
    let db_path = data_dir.join("semantic.db");

    if !db_path.exists() {
        anyhow::bail!("Semantic index not found. Run `sessfind-semantic index` first.");
    }

    let embedder = embedder::Embedder::new()?;
    let store = store::SemanticStore::open(&db_path)?;

    let query_embedding = embedder.embed_query(&query)?;

    let after_dt = after
        .as_deref()
        .map(|s| {
            NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map(|d| Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap()))
                .map_err(|_| anyhow::anyhow!("Invalid date: {s}"))
        })
        .transpose()?;
    let before_dt = before
        .as_deref()
        .map(|s| {
            NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map(|d| Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap()))
                .map_err(|_| anyhow::anyhow!("Invalid date: {s}"))
        })
        .transpose()?;

    let results = store.search(
        &query_embedding,
        limit,
        source.as_deref(),
        project.as_deref(),
        after_dt,
        before_dt,
    )?;

    let json = serde_json::to_string(&results)?;
    println!("{json}");
    Ok(())
}

fn cmd_status() -> Result<()> {
    let data_dir = sessfind_common::data_dir();
    let db_path = data_dir.join("semantic.db");

    let (indexed_chunks, model) = if db_path.exists() {
        let store = store::SemanticStore::open(&db_path)?;
        (store.count()?, "intfloat/multilingual-e5-small")
    } else {
        (0, "intfloat/multilingual-e5-small")
    };

    let status = serde_json::json!({
        "installed": true,
        "indexed_chunks": indexed_chunks,
        "model": model,
    });

    println!("{}", serde_json::to_string(&status)?);
    Ok(())
}

/// Count chars remaining after stripping XML/HTML tags and common meta prefixes.
fn meaningful_text_len(text: &str) -> usize {
    let mut clean = String::with_capacity(text.len());
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                continue;
            }
            _ if in_tag => continue,
            _ => clean.push(ch),
        }
    }
    // Strip "USER: " / "ASSISTANT: " prefixes and whitespace
    clean
        .replace("USER:", "")
        .replace("ASSISTANT:", "")
        .split_whitespace()
        .map(|w| w.len())
        .sum()
}
