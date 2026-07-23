//! Implementations of the CLI subcommands (human and `--json` output).
//! JSON goes to stdout only on success; errors go to stderr with exit != 0.

use std::collections::HashMap;

use anyhow::Result;
use sessfind_common::{
    Capabilities, ProjectGroup, SearchMethods, SessionSummary, ToolInfo, chat_command,
    new_session_command, resume_command, session_key,
};

use crate::indexer::engine::IndexEngine;
use crate::metadata::MetadataStore;
use crate::models::SearchParams;
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
    "source-qualified-sessions",
    "direct-session-tags",
    "source-freshness",
    "catalog-reconciliation",
    "session-grouped-search",
];

pub fn session_summary(r: &SearchResult) -> SessionSummary {
    SessionSummary {
        session_key: session_key(r.source, &r.session_id),
        session_id: r.session_id.clone(),
        source: r.source,
        project: r.project.clone(),
        title: r.title.clone(),
        custom_name: None,
        timestamp: r.timestamp,
        snippet: r.snippet.clone(),
        direct_tags: Vec::new(),
        tags: Vec::new(),
        resume: resume_command(r.source, &r.session_id, &r.project),
        new_session: new_session_command(r.source, &r.project),
    }
}

pub fn apply_custom_names(store: &MetadataStore, sessions: &mut [SearchResult]) -> Result<()> {
    let refs: Vec<(String, String)> = sessions
        .iter()
        .map(|session| {
            (
                session_key(session.source, &session.session_id),
                session.session_id.clone(),
            )
        })
        .collect();
    let names = store.names_for_sessions(&refs)?;
    for session in sessions {
        let key = session_key(session.source, &session.session_id);
        if let Some(name) = names.get(&key) {
            session.title = Some(name.clone());
        }
    }
    Ok(())
}

pub fn capabilities() -> Result<()> {
    let caps = Capabilities {
        version: env!("CARGO_PKG_VERSION").to_string(),
        json_api_version: JSON_API_VERSION,
        features: FEATURES.iter().map(|s| s.to_string()).collect(),
        search_methods: SearchMethods {
            fts: true,
            fuzzy: true,
            semantic: semantic::is_available() && semantic::status().is_ok(),
            llm: llm::detect_backends()?
                .into_iter()
                .map(|b| b.name)
                .collect(),
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
            ids.contains(&session_key(r.source, &r.session_id))
                || ids.contains(&r.session_id)
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
    let refs: Vec<(String, String)> = sessions
        .iter()
        .map(|r| (session_key(r.source, &r.session_id), r.session_id.clone()))
        .collect();
    let mut tags_by_session = store.tags_for_sessions(&refs)?;
    let mut names = store.names_for_sessions(&refs)?;
    let project_tags = store.project_tags_map()?;
    let summaries: Vec<SessionSummary> = sessions
        .iter()
        .map(|r| {
            let mut summary = session_summary(r);
            let key = session_key(r.source, &r.session_id);
            let direct_tags = tags_by_session.remove(&key).unwrap_or_default();
            let mut tags = direct_tags.clone();
            if let Some(inherited) = project_tags.get(&r.project) {
                for tag in inherited {
                    if !tags.contains(tag) {
                        tags.push(tag.clone());
                    }
                }
                tags.sort();
            }
            summary.direct_tags = direct_tags;
            summary.tags = tags;
            if let Some(name) = names.remove(&key) {
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
            .filter(|r| seen.insert((r.source, &r.session_id)))
            .take(3)
            .collect()
    };
    for r in &unique_sessions {
        println!("\x1b[90m  {} ({})\x1b[0m", r.session_id, r.source.as_str());
    }
    Ok(())
}

/// Supplement engine matches with sessfind-owned searchable metadata. Source
/// titles and projects are indexed by Tantivy; this path adds custom titles and
/// direct/inherited tags, which can change without rewriting source history.
pub fn metadata_search_matches(
    engine: &IndexEngine,
    store: &MetadataStore,
    params: &SearchParams,
) -> Result<Vec<SearchResult>> {
    let mut sessions = engine.list_sessions()?;
    if let Some(source) = &params.source {
        sessions.retain(|session| session.source.as_str() == source);
    }
    if let Some(project) = &params.project {
        let project = project.to_lowercase();
        sessions.retain(|session| session.project.to_lowercase().contains(&project));
    }
    if let Some(after) = params.after {
        sessions.retain(|session| session.timestamp >= after);
    }
    if let Some(before) = params.before {
        sessions.retain(|session| session.timestamp <= before);
    }

    let refs: Vec<(String, String)> = sessions
        .iter()
        .map(|session| {
            (
                session_key(session.source, &session.session_id),
                session.session_id.clone(),
            )
        })
        .collect();
    let names = store.names_for_sessions(&refs)?;
    let tags = store.tags_for_sessions(&refs)?;
    let project_tags = store.project_tags_map()?;

    let terms = parse_metadata_terms(&params.query);
    let required: Vec<&MetadataTerm> = terms
        .iter()
        .filter(|term| term.occur == MetadataOccur::Required)
        .collect();
    let optional: Vec<&MetadataTerm> = terms
        .iter()
        .filter(|term| term.occur == MetadataOccur::Optional)
        .collect();
    let excluded: Vec<&MetadataTerm> = terms
        .iter()
        .filter(|term| term.occur == MetadataOccur::Excluded)
        .collect();

    let mut matches = Vec::new();
    for mut session in sessions {
        let key = session_key(session.source, &session.session_id);
        let custom_name = names.get(&key);
        let mut effective_tags = tags.get(&key).cloned().unwrap_or_default();
        if let Some(inherited) = project_tags.get(&session.project) {
            effective_tags.extend(inherited.iter().cloned());
        }
        let haystack = format!(
            "{}\n{}\n{}\n{}",
            custom_name.map(String::as_str).unwrap_or(""),
            session.title.as_deref().unwrap_or(""),
            session.project,
            effective_tags.join("\n")
        )
        .to_lowercase();
        let required_match = required
            .iter()
            .all(|term| metadata_term_matches(term, &haystack));
        let optional_match = optional.is_empty()
            || optional
                .iter()
                .any(|term| metadata_term_matches(term, &haystack));
        let excluded_match = excluded
            .iter()
            .any(|term| metadata_term_matches(term, &haystack));
        if required_match
            && optional_match
            && !excluded_match
            && (!required.is_empty() || !optional.is_empty())
        {
            if let Some(name) = custom_name {
                session.title = Some(name.clone());
            }
            session.snippet = if effective_tags.is_empty() {
                format!("Matched session metadata: {}", session.project)
            } else {
                format!("Matched tags: {}", effective_tags.join(", "))
            };
            session.score = 0.1;
            matches.push(session);
        }
    }
    Ok(matches)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetadataOccur {
    Optional,
    Required,
    Excluded,
}

struct MetadataTerm {
    value: String,
    occur: MetadataOccur,
    prefix: bool,
}

fn parse_metadata_terms(query: &str) -> Vec<MetadataTerm> {
    let chars: Vec<char> = query.chars().collect();
    let mut terms = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        while chars.get(index).is_some_and(|value| value.is_whitespace()) {
            index += 1;
        }
        if index >= chars.len() {
            break;
        }
        let occur = match chars[index] {
            '+' => {
                index += 1;
                MetadataOccur::Required
            }
            '-' => {
                index += 1;
                MetadataOccur::Excluded
            }
            _ => MetadataOccur::Optional,
        };
        let quoted = chars.get(index) == Some(&'"');
        if quoted {
            index += 1;
        }
        let start = index;
        if quoted {
            while chars.get(index).is_some_and(|value| *value != '"') {
                index += 1;
            }
        } else {
            while chars.get(index).is_some_and(|value| !value.is_whitespace()) {
                index += 1;
            }
        }
        let mut value: String = chars[start..index].iter().collect();
        if quoted && chars.get(index) == Some(&'"') {
            index += 1;
        }
        let prefix = !quoted && value.ends_with('*');
        if prefix {
            value.pop();
        }
        let value = value.to_lowercase();
        if !value.is_empty() {
            terms.push(MetadataTerm {
                value,
                occur,
                prefix,
            });
        }
    }
    terms
}

fn metadata_term_matches(term: &MetadataTerm, haystack: &str) -> bool {
    if term.prefix {
        haystack
            .split(|value: char| !value.is_alphanumeric() && value != '_' && value != '-')
            .any(|word| word.starts_with(&term.value))
    } else {
        haystack.contains(&term.value)
    }
}

pub fn show(
    engine: &IndexEngine,
    store: &MetadataStore,
    session_id: &str,
    source: Option<&str>,
    json: bool,
) -> Result<()> {
    let chunks = session_chunks(engine, session_id, source)?;
    if chunks.is_empty() {
        anyhow::bail!(
            "No session found with ID: {session_id}. Use the complete ID from search results"
        );
    }
    let key = session_key(chunks[0].source, session_id);

    if json {
        let mut summary = session_summary(&chunks[0]);
        let direct_tags = store.tags_for_session(&key, session_id)?;
        let mut tags = direct_tags.clone();
        if let Some(inherited) = store.project_tags_map()?.get(&summary.project) {
            for tag in inherited {
                if !tags.contains(tag) {
                    tags.push(tag.clone());
                }
            }
            tags.sort();
        }
        summary.direct_tags = direct_tags;
        summary.tags = tags;
        if let Some(name) = store
            .names_for_sessions(&[(key.clone(), session_id.to_string())])?
            .remove(&key)
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
    let sync_states = engine.source_sync_states()?;
    let source_status = |source: &str, count: usize| {
        let state = sync_states.iter().find(|state| state.source == source);
        let status = match state {
            Some(state) if state.last_error.is_some() && state.last_success.is_some() => "stale",
            Some(state) if state.last_error.is_some() => "failed",
            Some(_) if count == 0 => "absent",
            Some(_) => "available",
            None if count == 0 => "absent",
            None => "stale",
        };
        serde_json::json!({
            "status": status,
            "sessions": count,
            "last_success": state.and_then(|state| state.last_success),
            "last_attempt": state.and_then(|state| state.last_attempt),
            "error": state.and_then(|state| state.last_error.as_deref()),
        })
    };

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
        let backends: Vec<serde_json::Value> = llm::detect_backends()?
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
                "distinct_total": total,
            },
            "sources": {
                "claude": source_status("claude", claude),
                "opencode": source_status("opencode", opencode),
                "copilot": source_status("copilot", copilot),
                "cursor": source_status("cursor", cursor),
                "codex": source_status("codex", codex),
            },
            "semantic": semantic_status,
            "llm_backends": backends,
            "watcher": crate::service::status_json(),
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
    println!("\x1b[1mSource freshness:\x1b[0m");
    for (source, count) in [
        ("claude", claude),
        ("opencode", opencode),
        ("copilot", copilot),
        ("cursor", cursor),
        ("codex", codex),
    ] {
        let value = source_status(source, count);
        let status = value["status"].as_str().unwrap_or("unknown");
        let last = value["last_success"].as_str().unwrap_or("never");
        println!("  {source:<10} {status:<10} last success: {last}");
        if let Some(error) = value["error"].as_str() {
            println!("    error: {error}");
        }
    }
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
    let backends = llm::detect_backends()?;
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
    let watcher = crate::service::status_json();
    println!(
        "\x1b[1mWatcher:\x1b[0m {}",
        watcher["state"].as_str().unwrap_or("unknown")
    );
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
fn project_sessions(engine: &IndexEngine, dir: &str) -> Result<(String, Vec<SearchResult>)> {
    let sessions = engine.list_sessions()?;
    let resolved = if sessions.iter().any(|s| s.project == dir) {
        dir.to_string()
    } else {
        let canonical = std::fs::canonicalize(dir).ok();
        sessions
            .iter()
            .map(|s| s.project.as_str())
            .find(|project| {
                canonical
                    .as_ref()
                    .is_some_and(|path| std::fs::canonicalize(project).ok().as_ref() == Some(path))
            })
            .map(str::to_string)
            .unwrap_or_else(|| dir.to_string())
    };
    let sessions: Vec<SearchResult> = sessions
        .into_iter()
        .filter(|s| s.project == resolved)
        .collect();
    if sessions.is_empty() {
        anyhow::bail!("No indexed sessions in {dir}");
    }
    Ok((resolved, sessions))
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
    if let Some(tool) = tool
        && !["claude", "opencode", "copilot"].contains(&tool)
    {
        anyhow::bail!("Unsupported summary tool: {tool}. Available: claude, opencode, copilot");
    }
    let (dir, sessions) = project_sessions(engine, dir)?;

    let backends = llm::detect_backends()?;
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
        .filter_map(|s| session_chunks(engine, &s.session_id, Some(s.source.as_str())).ok())
        .filter_map(|chunks| chunks.first().map(|c| truncate_str(&c.snippet, 800)))
        .collect();

    eprintln!(
        "Summarizing {dir} with {}… Sending session titles and excerpts from up to five recent conversations to this provider.",
        backend.display()
    );
    let prompt = build_summary_prompt(&dir, &sessions, &samples);
    let description = llm::invoke_with_budget(backend, &prompt, 0.50)?
        .trim()
        .to_string();
    if description.is_empty() {
        anyhow::bail!("LLM returned an empty summary");
    }
    store.set_project_description(&dir, &description, &backend.name)?;

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
    if let Some(tool) = tool
        && !["claude", "opencode", "codex"].contains(&tool)
    {
        anyhow::bail!("Unsupported chat tool: {tool}. Available: claude, opencode, codex");
    }
    let (dir, sessions) = project_sessions(engine, dir)?;
    let tags = store
        .project_tags_map()?
        .get(&dir)
        .cloned()
        .unwrap_or_default();
    let descriptions = store.project_descriptions_map()?;
    let brief = build_project_brief(
        &dir,
        &sessions,
        &tags,
        descriptions.get(&dir).map(|s| s.as_str()),
    );

    let chat_tools: Vec<Source> = installed_tools()
        .into_iter()
        .filter(|s| chat_command(*s, &dir, "").is_some())
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

    let spec = chat_command(source, &dir, &brief).expect("source filtered to chat-capable above");
    if json {
        println!("{}", serde_json::to_string(&spec)?);
    } else {
        println!("{}", spec.args.join(" "));
    }
    Ok(())
}

// ── Tags ──

fn session_chunks(
    engine: &IndexEngine,
    session_id: &str,
    source: Option<&str>,
) -> Result<Vec<SearchResult>> {
    let mut sources: Vec<Source> = engine
        .list_sessions()?
        .into_iter()
        .filter(|session| session.session_id == session_id)
        .map(|session| session.source)
        .collect();
    sources.sort_by_key(|source| source.as_str());
    sources.dedup();

    let selected = if let Some(source) = source {
        Some(
            Source::parse_source(source)
                .ok_or_else(|| anyhow::anyhow!("Unknown source: {source}"))?,
        )
    } else if sources.len() > 1 {
        let names: Vec<&str> = sources.iter().map(Source::as_str).collect();
        anyhow::bail!(
            "Session ID {session_id} exists in multiple sources ({}); pass --source",
            names.join(", ")
        );
    } else {
        sources.first().copied()
    };

    let mut chunks = engine.get_session_chunks(session_id)?;
    if let Some(source) = selected {
        chunks.retain(|chunk| chunk.source == source);
    }
    Ok(chunks)
}

fn resolve_session(
    engine: &IndexEngine,
    session_id: &str,
    source: Option<&str>,
) -> Result<(Source, String)> {
    let chunks = session_chunks(engine, session_id, source)?;
    let Some(first) = chunks.first() else {
        anyhow::bail!("No indexed session with ID: {session_id}");
    };
    Ok((first.source, session_key(first.source, session_id)))
}

fn migrate_session_metadata(
    engine: &IndexEngine,
    store: &MetadataStore,
    session_id: &str,
) -> Result<()> {
    let mut keys: Vec<String> = engine
        .list_sessions()?
        .into_iter()
        .filter(|session| session.session_id == session_id)
        .map(|session| session_key(session.source, session_id))
        .collect();
    keys.sort();
    keys.dedup();
    store.migrate_legacy_session(session_id, &keys)
}

pub fn tag_add(
    engine: &IndexEngine,
    store: &MetadataStore,
    session_id: &str,
    source: Option<&str>,
    tags: &[String],
) -> Result<()> {
    let (_, key) = resolve_session(engine, session_id, source)?;
    migrate_session_metadata(engine, store, session_id)?;
    for tag in tags {
        store.add_tag(&key, tag)?;
    }
    println!("Tagged {session_id} with: {}", tags.join(", "));
    Ok(())
}

pub fn tag_rm(
    engine: &IndexEngine,
    store: &MetadataStore,
    session_id: &str,
    source: Option<&str>,
    tags: &[String],
) -> Result<()> {
    let (_, key) = resolve_session(engine, session_id, source)?;
    migrate_session_metadata(engine, store, session_id)?;
    let mut removed = Vec::new();
    for tag in tags {
        if store.remove_tag(&key, session_id, tag)? {
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

pub fn tag_list(engine: &IndexEngine, store: &MetadataStore, json: bool) -> Result<()> {
    let sessions = engine.list_sessions()?;
    let refs: Vec<(String, String)> = sessions
        .iter()
        .map(|s| (session_key(s.source, &s.session_id), s.session_id.clone()))
        .collect();
    let direct = store.tags_for_sessions(&refs)?;
    let project_tags = store.project_tags_map()?;
    let mut counts: HashMap<String, usize> = HashMap::new();
    for session in &sessions {
        let key = session_key(session.source, &session.session_id);
        let mut effective = direct.get(&key).cloned().unwrap_or_default();
        if let Some(inherited) = project_tags.get(&session.project) {
            effective.extend(inherited.iter().cloned());
        }
        effective.sort();
        effective.dedup();
        for tag in effective {
            *counts.entry(tag).or_default() += 1;
        }
    }
    let mut tags: Vec<sessfind_common::TagCount> = counts
        .into_iter()
        .map(|(tag, session_count)| sessfind_common::TagCount { tag, session_count })
        .collect();
    tags.sort_by(|a, b| {
        b.session_count
            .cmp(&a.session_count)
            .then_with(|| a.tag.cmp(&b.tag))
    });
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

pub fn tag_add_project(
    engine: &IndexEngine,
    store: &MetadataStore,
    dir: &str,
    tags: &[String],
) -> Result<()> {
    let (dir, _) = project_sessions(engine, dir)?;
    for tag in tags {
        store.add_project_tag(&dir, tag)?;
    }
    println!("Tagged directory {dir} with: {}", tags.join(", "));
    Ok(())
}

pub fn tag_rm_project(
    engine: &IndexEngine,
    store: &MetadataStore,
    dir: &str,
    tags: &[String],
) -> Result<()> {
    let (dir, _) = project_sessions(engine, dir)?;
    let mut removed = Vec::new();
    for tag in tags {
        if store.remove_project_tag(&dir, tag)? {
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
    source: Option<&str>,
    name: Option<&str>,
) -> Result<()> {
    let (_, key) = resolve_session(engine, session_id, source)?;
    migrate_session_metadata(engine, store, session_id)?;
    match name {
        Some(name) if !name.trim().is_empty() => {
            store.set_session_name(&key, name.trim())?;
            println!("Renamed {session_id} to: {}", name.trim());
        }
        _ => {
            if store.clear_session_name(&key, session_id)? {
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

    #[test]
    fn metadata_terms_preserve_fts_operators_and_phrases() {
        let terms = parse_metadata_terms(r#"+shopping "exact phrase" -legacy shopp*"#);
        assert_eq!(terms.len(), 4);
        assert_eq!(terms[0].occur, MetadataOccur::Required);
        assert_eq!(terms[0].value, "shopping");
        assert_eq!(terms[1].occur, MetadataOccur::Optional);
        assert_eq!(terms[1].value, "exact phrase");
        assert_eq!(terms[2].occur, MetadataOccur::Excluded);
        assert_eq!(terms[2].value, "legacy");
        assert!(terms[3].prefix);
        assert!(metadata_term_matches(&terms[3], "a shopping assistant"));
    }
}
