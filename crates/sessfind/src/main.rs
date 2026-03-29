mod config;
mod indexer;
mod llm;
mod models;
mod search;
pub mod semantic;
mod service;
mod sources;
mod tui;
mod watcher;

use anyhow::Result;
use chrono::{NaiveDate, TimeZone, Utc};
use clap::{Parser, Subcommand};

use crate::indexer::engine::{IndexEngine, SearchParams};
use crate::sources::SessionSource;
use crate::sources::claude_code::ClaudeCodeSource;
use crate::sources::copilot::CopilotSource;
use crate::sources::opencode::OpenCodeSource;

#[derive(Parser)]
#[command(
    name = "sessfind",
    about = "Search past AI coding assistant sessions",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    /// Index all sources before launching TUI
    #[arg(long)]
    index: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Index sessions from all sources
    Index {
        /// Source to index (claude, opencode, copilot, all)
        #[arg(long, default_value = "all")]
        source: String,
        /// Force re-index all sessions
        #[arg(long)]
        force: bool,
    },
    /// Search indexed sessions (CLI mode)
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
        /// Search method (fts, fuzzy, semantic, llm)
        #[arg(long, short = 'm', default_value = "fts")]
        method: String,
    },
    /// Show full session content
    Show {
        /// Session ID (from search results)
        session_id: String,
    },
    /// Show index statistics
    Stats,
    /// Dump all indexed chunks as JSONL (for plugins)
    DumpChunks,
    /// Set LLM model override for a provider
    #[command(name = "llm-model-set")]
    LlmModelSet {
        /// Provider name (claude, opencode, copilot)
        provider: String,
        /// Model identifier (e.g. sonnet, anthropic/claude-sonnet-4-6)
        model: String,
    },
    /// Remove LLM model override (use provider's default)
    #[command(name = "llm-model-unset")]
    LlmModelUnset {
        /// Provider name (claude, opencode, copilot)
        provider: String,
    },
    /// Watch session directories and re-index on changes
    Watch {
        #[command(subcommand)]
        action: Option<WatchAction>,
    },
}

#[derive(Subcommand)]
enum WatchAction {
    /// Install watcher as a system service (launchd on macOS, systemd on Linux)
    Install,
    /// Remove the watcher system service
    Uninstall,
    /// Show whether the watcher service is running
    Status,
}

fn get_sources(filter: &str) -> Vec<Box<dyn SessionSource>> {
    let mut sources: Vec<Box<dyn SessionSource>> = Vec::new();
    match filter {
        "all" => {
            sources.push(Box::new(ClaudeCodeSource::new()));
            sources.push(Box::new(OpenCodeSource::new()));
            sources.push(Box::new(CopilotSource::new()));
        }
        "claude" => sources.push(Box::new(ClaudeCodeSource::new())),
        "opencode" => sources.push(Box::new(OpenCodeSource::new())),
        "copilot" => sources.push(Box::new(CopilotSource::new())),
        other => {
            eprintln!("Unknown source: {other}. Available: claude, opencode, copilot, all");
        }
    }
    sources
}

fn parse_date(s: &str) -> Result<chrono::DateTime<Utc>> {
    let date = NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| anyhow::anyhow!("Invalid date format: {s}. Expected YYYY-MM-DD"))?;
    Ok(Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap()))
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let data_dir = config::data_dir();
    let engine = IndexEngine::open(&data_dir)?;

    let command = match cli.command {
        Some(cmd) => cmd,
        None => {
            // Index before launching TUI if requested
            if cli.index {
                let sources = get_sources("all");
                for src in &sources {
                    let _ = engine.index_source(src.as_ref(), false);
                }
                if semantic::is_available() {
                    let _ = semantic::trigger_index();
                }
            }
            // Launch TUI
            if let Some(resume) = tui::run(&engine)? {
                exec_resume(&resume)?;
            }
            return Ok(());
        }
    };

    match command {
        Commands::Index { source, force } => {
            let sources = get_sources(&source);
            for src in &sources {
                eprint!("Indexing {}... ", src.name());
                let stats = engine.index_source(src.as_ref(), force)?;
                if stats.new_sessions == 0 {
                    eprintln!("up to date ({} sessions)", stats.total_sessions);
                } else {
                    eprintln!(
                        "done ({} new sessions, {} chunks)",
                        stats.new_sessions, stats.total_chunks
                    );
                }
            }

            // Trigger semantic indexing if plugin is available
            if semantic::is_available() {
                eprintln!();
                eprint!("Updating semantic index... ");
                match semantic::trigger_index() {
                    Ok(()) => {}
                    Err(e) => eprintln!("warning: semantic indexing failed: {e}"),
                }
            }
        }
        Commands::Search {
            query,
            source,
            project,
            after,
            before,
            limit,
            method,
        } => {
            let after_dt = after.as_deref().map(parse_date).transpose()?;
            let before_dt = before.as_deref().map(parse_date).transpose()?;

            let params = SearchParams {
                query: query.clone(),
                limit,
                source: source.clone(),
                project: project.clone(),
                after: after_dt,
                before: before_dt,
            };

            let results = if method == "semantic" {
                if !semantic::is_available() {
                    eprintln!("Semantic search plugin not installed.");
                    eprintln!("Install with: cargo install sessfind-semantic");
                    return Ok(());
                }
                semantic::search(&params)?
            } else if method == "llm" {
                let backends = llm::detect_backends();
                if backends.is_empty() {
                    eprintln!("No LLM CLI tools detected (claude, opencode, copilot).");
                    return Ok(());
                }
                let backend = &backends[0];
                eprintln!("Searching with LLM ({})...", backend.display());

                // Ask LLM to generate FTS queries from user intent
                let prompt = llm::build_query_gen_prompt(&query);
                let response = llm::invoke(backend, &prompt)?;
                let queries = llm::parse_query_gen_response(&response);

                let queries = if queries.is_empty() {
                    eprintln!("LLM returned no queries, falling back to original query.");
                    vec![query.clone()]
                } else {
                    eprintln!("LLM generated {} queries", queries.len());
                    queries
                };

                // Run each generated query and merge results
                let mut all_results = Vec::new();
                for q in &queries {
                    let qparams = SearchParams {
                        query: q.clone(),
                        limit: 30,
                        source: source.clone(),
                        project: project.clone(),
                        after: after_dt,
                        before: before_dt,
                    };
                    if let Ok(results) = engine.search(&qparams) {
                        all_results.extend(results);
                    }
                }

                // Dedup by session, keep highest score, sort descending
                let mut best: std::collections::HashMap<String, models::SearchResult> =
                    std::collections::HashMap::new();
                for r in all_results {
                    best.entry(r.session_id.clone())
                        .and_modify(|existing| {
                            if r.score > existing.score {
                                *existing = r.clone();
                            }
                        })
                        .or_insert(r);
                }
                let mut merged: Vec<_> = best.into_values().collect();
                merged.sort_by(|a, b| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| b.timestamp.cmp(&a.timestamp))
                });
                merged
            } else {
                engine.search(&params)?
            };

            if results.is_empty() {
                println!("No results found.");
                return Ok(());
            }

            println!(
                "\x1b[1m{:<6} {:<10} {:<30} {:<12} Preview\x1b[0m",
                "Score", "Source", "Project", "Date"
            );
            println!("\x1b[90m{}\x1b[0m", "-".repeat(100));

            for r in results.iter().take(limit) {
                let date = r.timestamp.format("%Y-%m-%d");
                let project = truncate_project(&r.project, 28);
                let snippet = r.snippet.replace('\n', " ");
                let snippet = truncate_str(&snippet, 60);

                let source_colored = match r.source {
                    models::Source::ClaudeCode => format!("\x1b[35m{:<10}\x1b[0m", "claude"),
                    models::Source::OpenCode => format!("\x1b[36m{:<10}\x1b[0m", "opencode"),
                    models::Source::Copilot => format!("\x1b[33m{:<10}\x1b[0m", "copilot"),
                };

                println!(
                    "\x1b[32m{:<6.2}\x1b[0m {} {:<30} \x1b[90m{:<12}\x1b[0m {}",
                    r.score, source_colored, project, date, snippet
                );
            }

            println!();
            println!("\x1b[90mUse `sessfind show <SESSION_ID>` to view full session.\x1b[0m");
            let unique_sessions: Vec<_> = {
                let mut seen = std::collections::HashSet::new();
                results
                    .iter()
                    .filter(|r| seen.insert(&r.session_id))
                    .take(3)
                    .collect()
            };
            for r in &unique_sessions {
                println!("\x1b[90m  {} ({})\x1b[0m", r.session_id, r.source.as_str());
            }
        }
        Commands::Show { session_id } => {
            let chunks = engine.get_session_chunks(&session_id)?;
            if chunks.is_empty() {
                eprintln!("No session found with ID: {session_id}");
                eprintln!("Tip: Use the full session ID from search results.");
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
        }
        Commands::DumpChunks => {
            let chunks = engine.dump_all_chunks()?;
            for chunk in &chunks {
                let json = serde_json::to_string(chunk)?;
                println!("{json}");
            }
        }
        Commands::Stats => {
            let claude = engine.session_count(Some("claude"))?;
            let opencode = engine.session_count(Some("opencode"))?;
            let copilot = engine.session_count(Some("copilot"))?;
            let total = engine.session_count(None)?;

            println!("\x1b[1mIndexed sessions:\x1b[0m");
            println!("  \x1b[35mClaude Code:\x1b[0m {claude}");
            println!("  \x1b[36mOpenCode:\x1b[0m    {opencode}");
            println!("  \x1b[33mCopilot:\x1b[0m     {copilot}");
            println!("  Total:       {total}");
            println!();

            // Semantic plugin status
            if semantic::is_available() {
                match semantic::status() {
                    Ok(st) => {
                        println!("\x1b[1mSemantic search:\x1b[0m");
                        println!("  Model:       {}", st.model);
                        println!("  Chunks:      {}", st.indexed_chunks);
                        println!();
                    }
                    Err(e) => {
                        println!("\x1b[1mSemantic search:\x1b[0m \x1b[33merror: {e}\x1b[0m");
                        println!();
                    }
                }
            } else {
                println!(
                    "\x1b[90mSemantic search: not installed (cargo install sessfind-semantic)\x1b[0m"
                );
                println!();
            }

            // LLM backends
            let backends = llm::detect_backends();
            if backends.is_empty() {
                println!(
                    "\x1b[90mLLM search: no CLI tools detected (install claude, opencode, or copilot)\x1b[0m"
                );
            } else {
                println!("\x1b[1mLLM search backends:\x1b[0m");
                for b in &backends {
                    let model_info = match &b.model {
                        Some(m) => format!("model: {m}"),
                        None => "model: (tool default)".into(),
                    };
                    println!("  \x1b[33m{:<10}\x1b[0m {model_info}", b.name);
                }
                println!(
                    "\x1b[90m  Config: {}\x1b[0m",
                    crate::config::config_path().display()
                );
            }
            println!();

            println!(
                "\x1b[90mIndex location: {}\x1b[0m",
                config::data_dir().display()
            );
        }
        Commands::LlmModelSet { provider, model } => {
            let valid = ["claude", "opencode", "copilot"];
            if !valid.contains(&provider.as_str()) {
                eprintln!(
                    "Unknown provider: {provider}. Available: {}",
                    valid.join(", ")
                );
                return Ok(());
            }
            let mut cfg = config::Config::load();
            cfg.llm_models.insert(provider.clone(), model.clone());
            cfg.save()?;
            println!("Set {provider} model to: {model}");
        }
        Commands::LlmModelUnset { provider } => {
            let mut cfg = config::Config::load();
            if cfg.llm_models.remove(&provider).is_some() {
                cfg.save()?;
                println!("Removed model override for {provider} (will use tool default)");
            } else {
                println!("No model override set for {provider}");
            }
        }
        Commands::Watch { action } => match action {
            None => watcher::run()?,
            Some(WatchAction::Install) => service::install()?,
            Some(WatchAction::Uninstall) => service::uninstall()?,
            Some(WatchAction::Status) => service::status()?,
        },
    }

    Ok(())
}

fn exec_resume(resume: &tui::ResumeCommand) -> Result<()> {
    use std::os::unix::process::CommandExt;
    let mut command = std::process::Command::new(&resume.args[0]);
    command.args(&resume.args[1..]);
    // Claude Code requires being in the project directory to find the session
    if let Some(ref cwd) = resume.cwd {
        let path = std::path::Path::new(cwd);
        if path.is_dir() {
            command.current_dir(path);
        }
    }
    // Replace current process with the resume command
    let err = command.exec();
    Err(anyhow::anyhow!("Failed to exec {}: {err}", resume.args[0]))
}

fn truncate_project(project: &str, max: usize) -> String {
    let chars: Vec<char> = project.chars().collect();
    if chars.len() <= max {
        project.to_string()
    } else {
        let skip = chars.len() - (max - 3);
        let truncated: String = chars[skip..].iter().collect();
        format!("...{truncated}")
    }
}

fn truncate_str(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s.to_string()
    } else {
        let truncated: String = chars[..max - 3].iter().collect();
        format!("{truncated}...")
    }
}
