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
use chrono::{Local, NaiveDate, TimeZone, Utc};
use clap::{Parser, Subcommand};

use crate::indexer::engine::{IndexEngine, SearchParams};
use crate::models::Source;
use crate::search::results::{SortOrder, dedup_by_session};
use crate::sources::SessionSource;

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
        /// Source owning the session (required only if native ids collide)
        #[arg(long, short = 's')]
        source: Option<String>,
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
        /// Source owning the session (required only if native ids collide)
        #[arg(long, short = 's')]
        source: Option<String>,
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
        /// Source owning the session (required only if native ids collide)
        #[arg(long, short = 's')]
        source: Option<String>,
        #[arg(required = true)]
        tags: Vec<String>,
    },
    /// Remove one or more tags from a session
    Rm {
        session_id: String,
        /// Source owning the session (required only if native ids collide)
        #[arg(long, short = 's')]
        source: Option<String>,
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

fn get_sources(filter: &str) -> Result<Vec<Box<dyn SessionSource>>> {
    let mut sources: Vec<Box<dyn SessionSource>> = Vec::new();
    match filter {
        "all" => {
            sources = crate::sources::all_sources();
        }
        "claude" | "opencode" | "copilot" | "cursor" | "codex" => {
            sources.push(crate::sources::source_for(
                Source::parse_source(filter).expect("validated source"),
            ));
        }
        other => {
            anyhow::bail!(
                "Unknown source: {other}. Available: claude, opencode, copilot, cursor, codex, all"
            )
        }
    }
    Ok(sources)
}

fn parse_date(s: &str, end_of_day: bool) -> Result<chrono::DateTime<Utc>> {
    let date = NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| anyhow::anyhow!("Invalid date format: {s}. Expected YYYY-MM-DD"))?;
    let time = if end_of_day {
        date.and_hms_nano_opt(23, 59, 59, 999_999_999).unwrap()
    } else {
        date.and_hms_opt(0, 0, 0).unwrap()
    };
    let local = Local
        .from_local_datetime(&time)
        .single()
        .ok_or_else(|| anyhow::anyhow!("Date is not unique in the local timezone: {s}"))?;
    Ok(local.with_timezone(&Utc))
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
            // Index before launching TUI if requested or if opening the
            // catalog recovered from a stale index/state mismatch.
            if cli.index || engine.requires_reindex() {
                if engine.requires_reindex() && !cli.index {
                    eprintln!("Rebuilding session catalog after index recovery…");
                }
                let sources = get_sources("all")?;
                for src in &sources {
                    if let Err(error) = engine.index_source(src.as_ref(), false) {
                        eprintln!("Warning: failed to index {}: {error}", src.name());
                    }
                }
                if semantic::is_available()
                    && let Err(error) = semantic::trigger_index()
                {
                    eprintln!("Warning: semantic indexing failed: {error}");
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
            let sources = get_sources(&source)?;
            let mut failures = Vec::new();
            for src in &sources {
                eprint!("Indexing {}... ", src.name());
                match engine.index_source(src.as_ref(), force) {
                    Ok(stats) => eprintln!(
                        "done ({} added, {} updated, {} removed, {} unchanged, {} skipped, {} chunks)",
                        stats.new_sessions,
                        stats.updated_sessions,
                        stats.removed_sessions,
                        stats.unchanged_sessions,
                        stats.skipped_sessions,
                        stats.total_chunks
                    ),
                    Err(error) => {
                        eprintln!("failed: {error}");
                        failures.push(format!("{}: {error}", src.name()));
                    }
                }
            }

            // Trigger semantic indexing if plugin is available
            if semantic::is_available() {
                eprintln!();
                eprint!("Updating semantic index... ");
                if let Err(error) = semantic::trigger_index() {
                    eprintln!("warning: semantic indexing failed: {error}");
                }
            }
            if !failures.is_empty() {
                anyhow::bail!(
                    "Indexing failed for {} source(s): {}",
                    failures.len(),
                    failures.join("; ")
                );
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
            if let Some(source) = source.as_deref()
                && Source::parse_source(source).is_none()
            {
                anyhow::bail!("Unknown source: {source}");
            }
            if !["fts", "fuzzy", "semantic", "llm"].contains(&method.as_str()) {
                anyhow::bail!("Unknown search method: {method}");
            }
            let after_dt = after
                .as_deref()
                .map(|value| parse_date(value, false))
                .transpose()?;
            let before_dt = before
                .as_deref()
                .map(|value| parse_date(value, true))
                .transpose()?;

            let params = SearchParams {
                query: query.clone(),
                limit,
                source: source.clone(),
                project: project.clone(),
                after: after_dt,
                before: before_dt,
            };

            let mut results = if method == "semantic" {
                if !semantic::is_available() {
                    anyhow::bail!(
                        "Semantic search unavailable. Install with: cargo install sessfind-semantic"
                    );
                }
                semantic::search(&params)?
            } else if method == "llm" {
                let backends = llm::detect_backends()?;
                if backends.is_empty() {
                    anyhow::bail!(
                        "LLM search unavailable: no supported CLI detected (claude, opencode, copilot)"
                    );
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

                expanded.results
            } else if method == "fuzzy" {
                engine.search_fuzzy(&params)?
            } else {
                engine.search(&params)?
            };

            let store = open_metadata()?;
            results.extend(commands::metadata_search_matches(&engine, &store, &params)?);
            commands::apply_custom_names(&store, &mut results)?;
            let results = dedup_by_session(&results, SortOrder::ScoreDesc);
            commands::print_search_results(&results, limit, json)?;
        }
        Commands::Show {
            session_id,
            source,
            json,
        } => {
            let engine = open_engine()?;
            let store = open_metadata()?;
            commands::show(&engine, &store, &session_id, source.as_deref(), json)?;
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
                if let Some(source) = source.as_deref()
                    && Source::parse_source(source).is_none()
                {
                    anyhow::bail!("Unknown source: {source}");
                }
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
                source,
                name,
                clear,
            } => {
                if clear == name.is_some() {
                    anyhow::bail!("sessions rename requires exactly one of NAME or --clear");
                }
                let engine = open_engine()?;
                let store = open_metadata()?;
                let name = if clear { None } else { name };
                commands::session_rename(
                    &engine,
                    &store,
                    &session_id,
                    source.as_deref(),
                    name.as_deref(),
                )?;
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
                TagAction::Add {
                    session_id,
                    source,
                    tags,
                } => {
                    let engine = open_engine()?;
                    commands::tag_add(&engine, &store, &session_id, source.as_deref(), &tags)?;
                }
                TagAction::Rm {
                    session_id,
                    source,
                    tags,
                } => {
                    let engine = open_engine()?;
                    commands::tag_rm(&engine, &store, &session_id, source.as_deref(), &tags)?;
                }
                TagAction::List { json } => {
                    let engine = open_engine()?;
                    commands::tag_list(&engine, &store, json)?;
                }
                TagAction::AddProject { dir, tags } => {
                    let engine = open_engine()?;
                    commands::tag_add_project(&engine, &store, &dir, &tags)?;
                }
                TagAction::RmProject { dir, tags } => {
                    let engine = open_engine()?;
                    commands::tag_rm_project(&engine, &store, &dir, &tags)?;
                }
            }
        }
        Commands::LlmModelSet { provider, model } => {
            let valid = ["claude", "opencode", "copilot"];
            if !valid.contains(&provider.as_str()) {
                anyhow::bail!(
                    "Unknown provider: {provider}. Available: {}",
                    valid.join(", ")
                );
            }
            if model.trim().is_empty() {
                anyhow::bail!("Model identifier cannot be empty");
            }
            let mut cfg = config::Config::load()?;
            cfg.llm_models.insert(provider.clone(), model.clone());
            cfg.save()?;
            println!("Set {provider} model to: {model}");
        }
        Commands::LlmModelUnset { provider } => {
            let valid = ["claude", "opencode", "copilot"];
            if !valid.contains(&provider.as_str()) {
                anyhow::bail!(
                    "Unknown provider: {provider}. Available: {}",
                    valid.join(", ")
                );
            }
            let mut cfg = config::Config::load()?;
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
        if !path.exists() {
            std::fs::create_dir_all(path).map_err(|error| {
                anyhow::anyhow!("Cannot create resume directory {cwd}: {error}")
            })?;
        }
        if !path.is_dir() {
            anyhow::bail!("Resume working directory is not a directory: {cwd}");
        }
        command.current_dir(path);
    }
    // Replace current process with the resume command
    let err = command.exec();
    Err(anyhow::anyhow!("Failed to exec {}: {err}", resume.args[0]))
}
