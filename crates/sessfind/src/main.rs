mod commands;
mod config;
mod indexer;
mod llm;
mod metadata;
mod models;
mod search;
pub mod semantic;
mod service;
mod sources;
mod tui;
mod version_check;
mod watcher;

use anyhow::Result;
use chrono::{NaiveDate, TimeZone, Utc};
use clap::{Parser, Subcommand};

use crate::indexer::engine::{IndexEngine, SearchParams};
use crate::search::results::{SortOrder, dedup_by_session};
use crate::sources::SessionSource;
use crate::sources::claude_code::ClaudeCodeSource;
use crate::sources::codex::CodexSource;
use crate::sources::copilot::CopilotSource;
use crate::sources::cursor::CursorSource;
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
    /// Initial search mode for TUI (fts, fuzzy, semantic, llm)
    #[arg(long, short = 'm')]
    mode: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Index sessions from all sources
    Index {
        /// Source to index (claude, opencode, copilot, cursor, codex, all)
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
        /// Filter by source (claude, opencode, copilot, cursor, codex)
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
        /// Output results as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show full session content
    Show {
        /// Session ID (from search results)
        session_id: String,
        /// Output session as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show index statistics
    Stats {
        /// Output statistics as JSON
        #[arg(long)]
        json: bool,
    },
    /// Print machine-readable capabilities of this binary (always JSON)
    Capabilities,
    /// List indexed sessions
    Sessions {
        #[command(subcommand)]
        action: SessionsAction,
    },
    /// List projects derived from indexed sessions
    Projects {
        #[command(subcommand)]
        action: ProjectsAction,
    },
    /// List installed AI CLI tools
    Tools {
        #[command(subcommand)]
        action: ToolsAction,
    },
    /// Manage tags on sessions
    Tag {
        #[command(subcommand)]
        action: TagAction,
    },
    /// Dump all indexed chunks as JSONL (for plugins)
    DumpChunks,
    /// Set LLM model override for a provider
    #[command(name = "llm-model-set")]
    LlmModelSet {
        /// Provider name (claude, opencode, copilot, cursor, codex)
        provider: String,
        /// Model identifier (e.g. sonnet, anthropic/claude-sonnet-4-6)
        model: String,
    },
    /// Remove LLM model override (use provider's default)
    #[command(name = "llm-model-unset")]
    LlmModelUnset {
        /// Provider name (claude, opencode, copilot, cursor, codex)
        provider: String,
    },
    /// Watch session directories and re-index on changes
    Watch {
        #[command(subcommand)]
        action: Option<WatchAction>,
    },
}

#[derive(Subcommand)]
enum SessionsAction {
    /// List all indexed sessions (newest first)
    List {
        /// Filter by source (claude, opencode, copilot, cursor, codex)
        #[arg(long, short = 's')]
        source: Option<String>,
        /// Filter by project name (substring match)
        #[arg(long, short = 'p')]
        project: Option<String>,
        /// Filter by tag
        #[arg(long, short = 't')]
        tag: Option<String>,
        /// Max sessions (default: all)
        #[arg(long, short = 'n')]
        limit: Option<usize>,
        /// Sort order (time, score)
        #[arg(long, default_value = "time")]
        sort: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Set or clear a custom display name for a session
    Rename {
        session_id: String,
        /// The new name (omit together with --clear to remove the override)
        name: Option<String>,
        /// Remove the custom name
        #[arg(long)]
        clear: bool,
    },
}

#[derive(Subcommand)]
enum ProjectsAction {
    /// List auto-grouped projects (grouped by session directory)
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Generate an LLM summary of a project directory
    Summarize {
        dir: String,
        /// LLM backend to use (claude, opencode, copilot); default: first detected
        #[arg(long)]
        tool: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Build the command that opens a chat about a project (context pre-loaded)
    Chat {
        dir: String,
        /// Tool to chat with (claude, opencode, codex); default: first capable
        #[arg(long)]
        tool: Option<String>,
        /// Output the command as JSON (CommandSpec)
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ToolsAction {
    /// List installed AI CLI tools with new-session commands
    List {
        /// Directory the new-session commands should run in (default: cwd)
        #[arg(long)]
        dir: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum TagAction {
    /// Add one or more tags to a session
    Add {
        session_id: String,
        #[arg(required = true)]
        tags: Vec<String>,
    },
    /// Remove one or more tags from a session
    Rm {
        session_id: String,
        #[arg(required = true)]
        tags: Vec<String>,
    },
    /// List all tags with session counts
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Tag a whole project directory (sessions in it inherit the tag)
    AddProject {
        dir: String,
        #[arg(required = true)]
        tags: Vec<String>,
    },
    /// Remove tags from a project directory
    RmProject {
        dir: String,
        #[arg(required = true)]
        tags: Vec<String>,
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
            sources.push(Box::new(CursorSource::new()));
            sources.push(Box::new(CodexSource::new()));
        }
        "claude" => sources.push(Box::new(ClaudeCodeSource::new())),
        "opencode" => sources.push(Box::new(OpenCodeSource::new())),
        "copilot" => sources.push(Box::new(CopilotSource::new())),
        "cursor" => sources.push(Box::new(CursorSource::new())),
        "codex" => sources.push(Box::new(CodexSource::new())),
        other => {
            eprintln!(
                "Unknown source: {other}. Available: claude, opencode, copilot, cursor, codex, all"
            );
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

    // The tantivy index is opened lazily: metadata-only commands (tag, project,
    // capabilities, llm-model-*) never touch it, avoiding its lock and load cost.
    let open_engine = || IndexEngine::open(&data_dir);
    let open_metadata = || metadata::MetadataStore::open(&config::metadata_db_path());

    let command = match cli.command {
        Some(cmd) => cmd,
        None => {
            let engine = open_engine()?;
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
            if let Some(resume) = tui::run(&engine, cli.mode.as_deref())? {
                exec_resume(&resume)?;
            }
            return Ok(());
        }
    };

    match command {
        Commands::Index { source, force } => {
            let engine = open_engine()?;
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
            json,
        } => {
            let engine = open_engine()?;
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
                    if json {
                        anyhow::bail!("semantic search unavailable");
                    }
                    return Ok(());
                }
                semantic::search(&params)?
            } else if method == "llm" {
                let backends = llm::detect_backends();
                if backends.is_empty() {
                    eprintln!("No LLM CLI tools detected (claude, opencode, copilot).");
                    if json {
                        anyhow::bail!("llm search unavailable");
                    }
                    return Ok(());
                }
                let backend = &backends[0];
                eprintln!("Searching with LLM ({})...", backend.display());

                let base = SearchParams {
                    query: String::new(),
                    limit: 30,
                    source: source.clone(),
                    project: project.clone(),
                    after: after_dt,
                    before: before_dt,
                };
                let expanded = llm::expanded_search(&engine, backend, &query, &base)?;
                eprintln!("LLM generated {} queries", expanded.queries.len());

                dedup_by_session(&expanded.results, SortOrder::ScoreDesc)
            } else if method == "fuzzy" {
                engine.search_fuzzy(&params)?
            } else {
                engine.search(&params)?
            };

            commands::print_search_results(&results, limit, json)?;
        }
        Commands::Show { session_id, json } => {
            let engine = open_engine()?;
            let store = open_metadata()?;
            commands::show(&engine, &store, &session_id, json)?;
        }
        Commands::DumpChunks => {
            let engine = open_engine()?;
            let chunks = engine.dump_all_chunks()?;
            for chunk in &chunks {
                let json = serde_json::to_string(chunk)?;
                println!("{json}");
            }
        }
        Commands::Stats { json } => {
            let engine = open_engine()?;
            commands::stats(&engine, json)?;
        }
        Commands::Capabilities => {
            commands::capabilities()?;
        }
        Commands::Sessions { action } => match action {
            SessionsAction::List {
                source,
                project,
                tag,
                limit,
                sort,
                json,
            } => {
                let sort = match sort.as_str() {
                    "time" => SortOrder::TimeDesc,
                    "score" => SortOrder::ScoreDesc,
                    other => anyhow::bail!("Invalid sort order: {other}. Expected time or score"),
                };
                let engine = open_engine()?;
                let store = open_metadata()?;
                commands::sessions_list(
                    &engine,
                    &store,
                    &commands::SessionListOpts {
                        source,
                        project,
                        tag,
                        limit,
                        sort,
                        json,
                    },
                )?;
            }
            SessionsAction::Rename {
                session_id,
                name,
                clear,
            } => {
                let engine = open_engine()?;
                let store = open_metadata()?;
                let name = if clear { None } else { name };
                commands::session_rename(&engine, &store, &session_id, name.as_deref())?;
            }
        },
        Commands::Projects { action } => match action {
            ProjectsAction::List { json } => {
                let engine = open_engine()?;
                let store = open_metadata()?;
                commands::projects_list(&engine, &store, json)?;
            }
            ProjectsAction::Summarize { dir, tool, json } => {
                let engine = open_engine()?;
                let store = open_metadata()?;
                commands::projects_summarize(&engine, &store, &dir, tool.as_deref(), json)?;
            }
            ProjectsAction::Chat { dir, tool, json } => {
                let engine = open_engine()?;
                let store = open_metadata()?;
                commands::projects_chat(&engine, &store, &dir, tool.as_deref(), json)?;
            }
        },
        Commands::Tools {
            action: ToolsAction::List { dir, json },
        } => {
            commands::tools_list(dir.as_deref(), json)?;
        }
        Commands::Tag { action } => {
            let store = open_metadata()?;
            match action {
                TagAction::Add { session_id, tags } => {
                    let engine = open_engine()?;
                    commands::tag_add(&engine, &store, &session_id, &tags)?;
                }
                TagAction::Rm { session_id, tags } => {
                    commands::tag_rm(&store, &session_id, &tags)?;
                }
                TagAction::List { json } => {
                    commands::tag_list(&store, json)?;
                }
                TagAction::AddProject { dir, tags } => {
                    commands::tag_add_project(&store, &dir, &tags)?;
                }
                TagAction::RmProject { dir, tags } => {
                    commands::tag_rm_project(&store, &dir, &tags)?;
                }
            }
        }
        Commands::LlmModelSet { provider, model } => {
            let valid = ["claude", "opencode", "copilot", "cursor", "codex"];
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
