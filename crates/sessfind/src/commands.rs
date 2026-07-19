//! Implementations of the CLI subcommands (human and `--json` output).
//! JSON goes to stdout only on success; errors go to stderr with exit != 0.

use std::collections::HashMap;

use anyhow::Result;
use sessfind_common::{
    Capabilities, ProjectGroup, SearchMethods, SessionSummary, ToolInfo, chat_command,
    new_session_command, resume_command,
};

use crate::indexer::engine::IndexEngine;
use crate::metadata::MetadataStore;
use crate::models::{SearchResult, Source};
use crate::search::results::{SortOrder, apply_sort};
use crate::{config, llm, semantic};

/// Bump only on breaking changes to the JSON output shapes in sessfind-common.
pub const JSON_API_VERSION: u32 = 1;

/// Features advertised via `sessfind capabilities`; clients gate UI on these.
const FEATURES: &[&str] = &[
    "search-json",
    "sessions-list",
    "projects-auto",
    "resume-spec",
    "tags",
    "tools-list",
    "session-rename",
    "project-tags",
    "project-summaries",
    "project-chat",
];

pub fn session_summary(r: &SearchResult) -> SessionSummary {
    SessionSummary {
        session_id: r.session_id.clone(),
        source: r.source,
        project: r.project.clone(),
        title: r.title.clone(),
        custom_name: None,
        timestamp: r.timestamp,
        snippet: r.snippet.clone(),
        tags: Vec::new(),
        resume: resume_command(r.source, &r.session_id, &r.project),
        new_session: new_session_command(r.source, &r.project),
    }
}

pub fn capabilities() -> Result<()> {
    let caps = Capabilities {
        version: env!("CARGO_PKG_VERSION").to_string(),
        json_api_version: JSON_API_VERSION,
        features: FEATURES.iter().map(|s| s.to_string()).collect(),
        search_methods: SearchMethods {
            fts: true,
            fuzzy: true,
            semantic: semantic::is_available(),
            llm: llm::detect_backends().into_iter().map(|b| b.name).collect(),
        },
        data_dir: config::data_dir().display().to_string(),
    };
    println!("{}", serde_json::to_string_pretty(&caps)?);
    Ok(())
}

pub struct SessionListOpts {
    pub source: Option<String>,
    pub project: Option<String>,
    pub tag: Option<String>,
    pub limit: Option<usize>,
    pub sort: SortOrder,
    pub json: bool,
}

pub fn sessions_list(
    engine: &IndexEngine,
    store: &MetadataStore,
    opts: &SessionListOpts,
) -> Result<()> {
    let mut sessions = engine.list_sessions()?;
    if let Some(src) = &opts.source {
        sessions.retain(|r| r.source.as_str() == src);
    }
    if let Some(proj) = &opts.project {
        let needle = proj.to_lowercase();
        sessions.retain(|r| r.project.to_lowercase().contains(&needle));
    }
    if let Some(tag) = &opts.tag {
        // Effective tagging: direct session tags plus tags on the whole dir.
        let ids: std::collections::HashSet<String> =
            store.sessions_with_tag(tag)?.into_iter().collect();
        let project_tags = store.project_tags_map()?;
        sessions.retain(|r| {
            ids.contains(&r.session_id)
                || project_tags
                    .get(&r.project)
                    .is_some_and(|tags| tags.contains(tag))
        });
    }
    apply_sort(&mut sessions, opts.sort);
    if let Some(limit) = opts.limit {
        sessions.truncate(limit);
    }

    // Decorate with tags (direct + inherited from dir) and custom names,
    // each in one batch query.
    let ids: Vec<String> = sessions.iter().map(|r| r.session_id.clone()).collect();
    let mut tags_by_session = store.tags_for_sessions(&ids)?;
    let mut names = store.names_for_sessions(&ids)?;
    let project_tags = store.project_tags_map()?;
    let summaries: Vec<SessionSummary> = sessions
        .iter()
        .map(|r| {
            let mut summary = session_summary(r);
            let mut tags = tags_by_session.remove(&r.session_id).unwrap_or_default();
            if let Some(inherited) = project_tags.get(&r.project) {
                for tag in inherited {
                    if !tags.contains(tag) {
                        tags.push(tag.clone());
                    }
                }
                tags.sort();
            }
            summary.tags = tags;
            if let Some(name) = names.remove(&r.session_id) {
                summary.title = Some(name.clone());
                summary.custom_name = Some(name);
            }
            summary
        })
        .collect();
    if opts.json {
        println!("{}", serde_json::to_string(&summaries)?);
        return Ok(());
    }

    println!(
        "\x1b[1m{:<10} {:<12} {:<30} {:<38} Title / Preview\x1b[0m",
        "Source", "Date", "Project", "Session ID"
    );
    println!("\x1b[90m{}\x1b[0m", "-".repeat(120));
    for s in &summaries {
        let mut text = s.title.clone().unwrap_or_else(|| s.snippet.clone());
        if !s.tags.is_empty() {
            text = format!("[{}] {text}", s.tags.join(", "));
        }
        println!(
            "{} \x1b[90m{:<12}\x1b[0m {:<30} \x1b[90m{:<38}\x1b[0m {}",
            colored_source(s.source),
            s.timestamp.format("%Y-%m-%d"),
            truncate_project(&s.project, 28),
            truncate_str(&s.session_id, 36),
            truncate_str(&text.replace('\n', " "), 60)
        );
    }
    Ok(())
}

pub fn projects_list(engine: &IndexEngine, store: &MetadataStore, json: bool) -> Result<()> {
    let sessions = engine.list_sessions()?;
    let project_tags = store.project_tags_map()?;
    let descriptions = store.project_descriptions_map()?;

    let mut groups: HashMap<String, ProjectGroup> = HashMap::new();
    for s in &sessions {
        let group = groups
            .entry(s.project.clone())
            .or_insert_with(|| ProjectGroup {
                path: s.project.clone(),
                name: project_display_name(&s.project),
                session_count: 0,
                last_activity: s.timestamp,
                sources: Vec::new(),
                tags: project_tags.get(&s.project).cloned().unwrap_or_default(),
                description: descriptions.get(&s.project).cloned(),
            });
        group.session_count += 1;
        if s.timestamp > group.last_activity {
            group.last_activity = s.timestamp;
        }
        if !group.sources.contains(&s.source) {
            group.sources.push(s.source);
        }
    }

    let mut projects: Vec<ProjectGroup> = groups.into_values().collect();
    projects.sort_by_key(|g| std::cmp::Reverse(g.last_activity));

    if json {
        println!("{}", serde_json::to_string(&projects)?);
        return Ok(());
    }

    println!(
        "\x1b[1m{:<24} {:>8} {:<12} {:<24} Path\x1b[0m",
        "Project", "Sessions", "Last", "Sources"
    );
    println!("\x1b[90m{}\x1b[0m", "-".repeat(110));
    for p in &projects {
        let sources: Vec<&str> = p.sources.iter().map(|s| s.as_str()).collect();
        println!(
            "{:<24} {:>8} \x1b[90m{:<12}\x1b[0m {:<24} \x1b[90m{}\x1b[0m",
            truncate_str(&p.name, 22),
            p.session_count,
            p.last_activity.format("%Y-%m-%d"),
            truncate_str(&sources.join(","), 22),
            p.path
        );
    }
    Ok(())
}

pub fn print_search_results(results: &[SearchResult], limit: usize, json: bool) -> Result<()> {
    if json {
        let limited: Vec<&SearchResult> = results.iter().take(limit).collect();
        println!("{}", serde_json::to_string(&limited)?);
        return Ok(());
    }

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

        println!(
            "\x1b[32m{:<6.2}\x1b[0m {} {:<30} \x1b[90m{:<12}\x1b[0m {}",
            r.score,
            colored_source(r.source),
            project,
            date,
            snippet
        );
    }

    println!();
    println!("\x1b[90mUse `sessfind show <SESSION_ID>` to view full session.\x1b[0m");
    let unique_sessions: Vec<&SearchResult> = {
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
    Ok(())
}

pub fn show(
    engine: &IndexEngine,
    store: &MetadataStore,
    session_id: &str,
    json: bool,
) -> Result<()> {
    let chunks = engine.get_session_chunks(session_id)?;
    if chunks.is_empty() {
        if json {
            anyhow::bail!("No session found with ID: {session_id}");
        }
        eprintln!("No session found with ID: {session_id}");
        eprintln!("Tip: Use the full session ID from search results.");
        return Ok(());
    }

    if json {
        let mut summary = session_summary(&chunks[0]);
        let mut tags = store.tags_for_session(session_id)?;
        if let Some(inherited) = store.project_tags_map()?.get(&summary.project) {
            for tag in inherited {
                if !tags.contains(tag) {
                    tags.push(tag.clone());
                }
            }
            tags.sort();
        }
        summary.tags = tags;
        if let Some(name) = store
            .names_for_sessions(&[session_id.to_string()])?
            .remove(session_id)
        {
            summary.title = Some(name.clone());
            summary.custom_name = Some(name);
        }
        let output = serde_json::json!({
            "session": summary,
            "chunks": chunks,
        });
        println!("{output}");
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

pub fn stats(engine: &IndexEngine, json: bool) -> Result<()> {
    let claude = engine.session_count(Some("claude"))?;
    let opencode = engine.session_count(Some("opencode"))?;
    let copilot = engine.session_count(Some("copilot"))?;
    let cursor = engine.session_count(Some("cursor"))?;
    let codex = engine.session_count(Some("codex"))?;
    let total = engine.session_count(None)?;

    if json {
        let semantic_status = if semantic::is_available() {
            match semantic::status() {
                Ok(st) => serde_json::json!({
                    "available": true,
                    "model": st.model,
                    "indexed_chunks": st.indexed_chunks,
                }),
                Err(e) => serde_json::json!({ "available": true, "error": e.to_string() }),
            }
        } else {
            serde_json::json!({ "available": false })
        };
        let backends: Vec<serde_json::Value> = llm::detect_backends()
            .into_iter()
            .map(|b| serde_json::json!({ "name": b.name, "model": b.model }))
            .collect();
        let output = serde_json::json!({
            "sessions": {
                "claude": claude,
                "opencode": opencode,
                "copilot": copilot,
                "cursor": cursor,
                "codex": codex,
                "total": total,
            },
            "semantic": semantic_status,
            "llm_backends": backends,
            "data_dir": config::data_dir().display().to_string(),
        });
        println!("{output}");
        return Ok(());
    }

    println!("\x1b[1mIndexed sessions:\x1b[0m");
    println!("  \x1b[35mClaude Code:\x1b[0m {claude}");
    println!("  \x1b[36mOpenCode:\x1b[0m    {opencode}");
    println!("  \x1b[33mCopilot:\x1b[0m     {copilot}");
    println!("  \x1b[32mCursor:\x1b[0m      {cursor}");
    println!("  \x1b[91mCodex:\x1b[0m       {codex}");
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
        println!("\x1b[90mSemantic search: not installed (cargo install sessfind-semantic)\x1b[0m");
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
    Ok(())
}

// ── Tools ──

/// AI CLI tools found on PATH (binary name == source name for all sources).
fn installed_tools() -> Vec<Source> {
    [
        Source::ClaudeCode,
        Source::OpenCode,
        Source::Copilot,
        Source::Cursor,
        Source::Codex,
    ]
    .into_iter()
    .filter(|s| which::which(s.as_str()).is_ok())
    .collect()
}

/// List installed tools with a ready new-session command for `dir`.
pub fn tools_list(dir: Option<&str>, json: bool) -> Result<()> {
    let dir = match dir {
        Some(d) => d.to_string(),
        None => std::env::current_dir()?.to_string_lossy().to_string(),
    };
    let tools: Vec<ToolInfo> = installed_tools()
        .into_iter()
        .map(|source| ToolInfo {
            name: source.as_str().to_string(),
            new_session: new_session_command(source, &dir),
            chat_capable: chat_command(source, &dir, "").is_some(),
        })
        .collect();

    if json {
        println!("{}", serde_json::to_string(&tools)?);
        return Ok(());
    }
    if tools.is_empty() {
        println!("No AI CLI tools found on PATH.");
        return Ok(());
    }
    println!("\x1b[1m{:<12} New session command\x1b[0m", "Tool");
    println!("\x1b[90m{}\x1b[0m", "-".repeat(50));
    for t in &tools {
        println!("{:<12} {}", t.name, t.new_session.args.join(" "));
    }
    Ok(())
}

// ── Project summaries & chat ──

/// Sessions in the given directory, newest first; errors when empty.
fn project_sessions(engine: &IndexEngine, dir: &str) -> Result<Vec<SearchResult>> {
    let mut sessions = engine.list_sessions()?;
    sessions.retain(|s| s.project == dir);
    if sessions.is_empty() {
        anyhow::bail!("No indexed sessions in {dir}");
    }
    Ok(sessions)
}

/// Prompt asking an LLM to describe what the project is about, based on
/// session titles and a sample of recent conversation content.
fn build_summary_prompt(dir: &str, sessions: &[SearchResult], samples: &[String]) -> String {
    let mut listing = String::new();
    for s in sessions.iter().take(20) {
        let title = s.title.as_deref().unwrap_or(&s.snippet);
        listing.push_str(&format!(
            "- {} — {}\n",
            s.timestamp.format("%Y-%m-%d"),
            title
        ));
    }
    let mut sample_text = String::new();
    for sample in samples {
        sample_text.push_str(sample);
        sample_text.push_str("\n---\n");
    }
    format!(
        r#"You are summarizing a software project for a session-browser sidebar.
Project directory: {dir}

AI coding sessions recorded in this project (newest first):
{listing}
Excerpts from recent sessions:
{sample_text}
Write a 2-3 sentence description of what this project is and what has been
worked on recently. Use the dominant language of the sessions. Return ONLY the
description text, no headings, no markdown."#
    )
}

pub fn projects_summarize(
    engine: &IndexEngine,
    store: &MetadataStore,
    dir: &str,
    tool: Option<&str>,
    json: bool,
) -> Result<()> {
    let sessions = project_sessions(engine, dir)?;

    let backends = llm::detect_backends();
    let backend = match tool {
        Some(name) => backends
            .iter()
            .find(|b| b.name == name)
            .ok_or_else(|| anyhow::anyhow!("LLM backend '{name}' not detected"))?,
        None => backends.first().ok_or_else(|| {
            anyhow::anyhow!("No LLM CLI tools detected (claude, opencode, copilot)")
        })?,
    };

    // Sample the first chunk of the five most recent sessions, truncated.
    let samples: Vec<String> = sessions
        .iter()
        .take(5)
        .filter_map(|s| engine.get_session_chunks(&s.session_id).ok())
        .filter_map(|chunks| chunks.first().map(|c| truncate_str(&c.snippet, 800)))
        .collect();

    eprintln!("Summarizing {dir} with {}…", backend.display());
    let prompt = build_summary_prompt(dir, &sessions, &samples);
    let description = llm::invoke(backend, &prompt)?.trim().to_string();
    if description.is_empty() {
        anyhow::bail!("LLM returned an empty summary");
    }
    store.set_project_description(dir, &description, &backend.name)?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "project_dir": dir,
                "description": description,
                "tool": backend.name,
            })
        );
    } else {
        println!("{description}");
    }
    Ok(())
}

/// Markdown brief injected as the opening prompt of a "chat about this
/// project" session.
fn build_project_brief(
    dir: &str,
    sessions: &[SearchResult],
    tags: &[String],
    description: Option<&str>,
) -> String {
    let mut brief = String::new();
    brief.push_str(&format!(
        "I'm starting a working session in the project at `{dir}`.\n\n"
    ));
    if let Some(desc) = description {
        brief.push_str(&format!("About this project: {desc}\n\n"));
    }
    if !tags.is_empty() {
        brief.push_str(&format!("Project tags: {}\n\n", tags.join(", ")));
    }
    brief.push_str("Recent AI coding sessions in this project:\n");
    for s in sessions.iter().take(15) {
        let title = s.title.as_deref().unwrap_or(&s.snippet);
        brief.push_str(&format!(
            "- {} [{}] {} (id: {})\n",
            s.timestamp.format("%Y-%m-%d"),
            s.source.as_str(),
            title,
            s.session_id
        ));
    }
    brief.push_str(
        "\nYou can inspect any past session with `sessfind show <id>` and search \
         them with `sessfind search <query>`.\n\
         Familiarize yourself with the project, then ask me what I want to work on.",
    );
    brief
}

pub fn projects_chat(
    engine: &IndexEngine,
    store: &MetadataStore,
    dir: &str,
    tool: Option<&str>,
    json: bool,
) -> Result<()> {
    let sessions = project_sessions(engine, dir)?;
    let tags = store
        .project_tags_map()?
        .get(dir)
        .cloned()
        .unwrap_or_default();
    let descriptions = store.project_descriptions_map()?;
    let brief = build_project_brief(
        dir,
        &sessions,
        &tags,
        descriptions.get(dir).map(|s| s.as_str()),
    );

    let chat_tools: Vec<Source> = installed_tools()
        .into_iter()
        .filter(|s| chat_command(*s, dir, "").is_some())
        .collect();
    let source = match tool {
        Some(name) => {
            let source = Source::parse_source(name)
                .ok_or_else(|| anyhow::anyhow!("Unknown tool: {name}"))?;
            if !chat_tools.contains(&source) {
                let names: Vec<&str> = chat_tools.iter().map(|s| s.as_str()).collect();
                anyhow::bail!(
                    "'{name}' cannot open a chat with an initial prompt. Available: {}",
                    names.join(", ")
                );
            }
            source
        }
        None => *chat_tools
            .first()
            .ok_or_else(|| anyhow::anyhow!("No chat-capable AI CLI tools found on PATH"))?,
    };

    let spec = chat_command(source, dir, &brief).expect("source filtered to chat-capable above");
    if json {
        println!("{}", serde_json::to_string(&spec)?);
    } else {
        println!("{}", spec.args.join(" "));
    }
    Ok(())
}

// ── Tags ──

/// Guard against tagging a session that isn't indexed (would orphan the row).
fn ensure_session_exists(engine: &IndexEngine, session_id: &str) -> Result<()> {
    if engine.get_session_chunks(session_id)?.is_empty() {
        anyhow::bail!("No indexed session with ID: {session_id}");
    }
    Ok(())
}

pub fn tag_add(
    engine: &IndexEngine,
    store: &MetadataStore,
    session_id: &str,
    tags: &[String],
) -> Result<()> {
    ensure_session_exists(engine, session_id)?;
    for tag in tags {
        store.add_tag(session_id, tag)?;
    }
    println!("Tagged {session_id} with: {}", tags.join(", "));
    Ok(())
}

pub fn tag_rm(store: &MetadataStore, session_id: &str, tags: &[String]) -> Result<()> {
    let mut removed = Vec::new();
    for tag in tags {
        if store.remove_tag(session_id, tag)? {
            removed.push(tag.clone());
        }
    }
    if removed.is_empty() {
        println!("No matching tags on {session_id}");
    } else {
        println!("Removed from {session_id}: {}", removed.join(", "));
    }
    Ok(())
}

pub fn tag_list(store: &MetadataStore, json: bool) -> Result<()> {
    let tags = store.list_tags()?;
    if json {
        println!("{}", serde_json::to_string(&tags)?);
        return Ok(());
    }
    if tags.is_empty() {
        println!("No tags yet.");
        return Ok(());
    }
    println!("\x1b[1m{:<24} Sessions\x1b[0m", "Tag");
    println!("\x1b[90m{}\x1b[0m", "-".repeat(40));
    for t in &tags {
        println!("{:<24} {}", t.tag, t.session_count);
    }
    Ok(())
}

pub fn tag_add_project(store: &MetadataStore, dir: &str, tags: &[String]) -> Result<()> {
    for tag in tags {
        store.add_project_tag(dir, tag)?;
    }
    println!("Tagged directory {dir} with: {}", tags.join(", "));
    Ok(())
}

pub fn tag_rm_project(store: &MetadataStore, dir: &str, tags: &[String]) -> Result<()> {
    let mut removed = Vec::new();
    for tag in tags {
        if store.remove_project_tag(dir, tag)? {
            removed.push(tag.clone());
        }
    }
    if removed.is_empty() {
        println!("No matching tags on {dir}");
    } else {
        println!("Removed from {dir}: {}", removed.join(", "));
    }
    Ok(())
}

// ── Session rename ──

pub fn session_rename(
    engine: &IndexEngine,
    store: &MetadataStore,
    session_id: &str,
    name: Option<&str>,
) -> Result<()> {
    match name {
        Some(name) if !name.trim().is_empty() => {
            ensure_session_exists(engine, session_id)?;
            store.set_session_name(session_id, name.trim())?;
            println!("Renamed {session_id} to: {}", name.trim());
        }
        _ => {
            if store.clear_session_name(session_id)? {
                println!("Cleared custom name of {session_id}");
            } else {
                println!("No custom name set for {session_id}");
            }
        }
    }
    Ok(())
}

fn colored_source(source: Source) -> String {
    match source {
        Source::ClaudeCode => format!("\x1b[35m{:<10}\x1b[0m", "claude"),
        Source::OpenCode => format!("\x1b[36m{:<10}\x1b[0m", "opencode"),
        Source::Copilot => format!("\x1b[33m{:<10}\x1b[0m", "copilot"),
        Source::Cursor => format!("\x1b[32m{:<10}\x1b[0m", "cursor"),
        Source::Codex => format!("\x1b[91m{:<10}\x1b[0m", "codex"),
    }
}

fn project_display_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn result(session_id: &str, project: &str) -> SearchResult {
        SearchResult {
            chunk_id: format!("claude:{session_id}:0"),
            session_id: session_id.into(),
            source: Source::ClaudeCode,
            project: project.into(),
            timestamp: Utc::now(),
            title: Some("t".into()),
            snippet: "USER: hi".into(),
            score: 1.0,
        }
    }

    #[test]
    fn session_summary_maps_resume_commands() {
        let summary = session_summary(&result("s1", "/proj"));
        assert_eq!(summary.resume.args, vec!["claude", "--resume", "s1"]);
        assert_eq!(summary.resume.cwd.as_deref(), Some("/proj"));
        assert_eq!(summary.new_session.args, vec!["claude"]);
        assert!(summary.tags.is_empty());
    }

    #[test]
    fn project_display_name_is_last_component() {
        assert_eq!(project_display_name("/home/user/my-repo"), "my-repo");
        assert_eq!(project_display_name("plain"), "plain");
    }

    #[test]
    fn truncate_helpers() {
        assert_eq!(truncate_str("short", 10), "short");
        assert_eq!(truncate_str("longer-than-max", 10), "longer-...");
        assert_eq!(truncate_project("/a/b/c", 10), "/a/b/c");
        assert!(truncate_project("/very/long/project/path", 10).starts_with("..."));
    }
}
