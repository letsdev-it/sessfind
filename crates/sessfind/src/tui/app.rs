use std::collections::HashMap;
use std::sync::mpsc;

use chrono::{DateTime, Utc};

use crate::indexer::engine::{IndexEngine, SearchParams};
use crate::llm::{self, LlmBackend};
use crate::models::{SearchResult, Source};
use crate::semantic;

#[derive(Debug, Clone)]
pub enum SearchMode {
    Fts,
    Fuzzy,
    Semantic,
    Llm(LlmBackend),
}

impl SearchMode {
    pub fn label(&self) -> String {
        match self {
            SearchMode::Fts => "Full-Text Search".into(),
            SearchMode::Fuzzy => "Fuzzy".into(),
            SearchMode::Semantic => "Semantic".into(),
            SearchMode::Llm(backend) => {
                format!("LLM ({})", backend.display())
            }
        }
    }

    pub fn is_llm(&self) -> bool {
        matches!(self, SearchMode::Llm(_))
    }

    pub fn is_deferred(&self) -> bool {
        matches!(self, SearchMode::Semantic | SearchMode::Llm(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Search,
    Results,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultsPane {
    List,
    Preview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeOption {
    SessionDir,
    CurrentDir,
    Cancel,
}

impl ResumeOption {
    pub const ALL: [ResumeOption; 3] = [
        ResumeOption::SessionDir,
        ResumeOption::CurrentDir,
        ResumeOption::Cancel,
    ];
}

#[derive(Debug, Clone)]
pub struct ResumeConfirmState {
    pub session_id: String,
    pub source: Source,
    pub project: String,
    pub title: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub selected: usize,
    pub session_dir_exists: bool,
}

pub struct App<'a> {
    pub input: String,
    pub cursor_pos: usize,
    pub results: Vec<SearchResult>,
    pub selected: usize,
    pub detail_chunks: Vec<SearchResult>,
    pub detail_scroll: usize,
    pub available_modes: Vec<SearchMode>,
    pub mode_index: usize,
    pub semantic_searching: bool,
    pub llm_searching: bool,
    pub focus: Focus,
    pub results_pane: ResultsPane,
    pub should_quit: bool,
    pub resume_session: Option<(String, Source, String)>, // (session_id, source, project)
    pub confirm_resume: Option<ResumeConfirmState>,
    pub show_help: bool,
    pub help_scroll: usize,
    pub update_rx: mpsc::Receiver<Option<String>>,
    pub latest_version: Option<String>,
    engine: &'a IndexEngine,
    all_chunks: Vec<SearchResult>,
    cached_session_id: Option<String>,
}

impl<'a> App<'a> {
    pub fn new(engine: &'a IndexEngine) -> anyhow::Result<Self> {
        let all_chunks = engine.list_all_chunks()?;

        // Build available modes: FTS, Fuzzy, [Semantic], [LLM backends]
        let mut available_modes: Vec<SearchMode> = vec![SearchMode::Fts, SearchMode::Fuzzy];
        if semantic::is_available() {
            available_modes.push(SearchMode::Semantic);
        }
        for backend in llm::detect_backends() {
            available_modes.push(SearchMode::Llm(backend));
        }

        // Show all sessions initially (deduplicated)
        let results = dedup_by_session(&all_chunks);

        let update_rx = crate::version_check::check_latest_version_async();

        let mut app = Self {
            input: String::new(),
            cursor_pos: 0,
            results,
            selected: 0,
            detail_chunks: Vec::new(),
            detail_scroll: 0,
            available_modes,
            mode_index: 0,
            semantic_searching: false,
            llm_searching: false,
            focus: Focus::Search,
            results_pane: ResultsPane::List,
            should_quit: false,
            resume_session: None,
            confirm_resume: None,
            show_help: false,
            help_scroll: 0,
            update_rx,
            latest_version: None,
            engine,
            all_chunks,
            cached_session_id: None,
        };

        app.load_detail();
        Ok(app)
    }

    pub fn search_mode(&self) -> &SearchMode {
        &self.available_modes[self.mode_index]
    }

    pub fn on_input_changed(&mut self) {
        self.selected = 0;
        self.detail_scroll = 0;

        if self.input.is_empty() {
            self.results = dedup_by_session(&self.all_chunks);
        } else {
            match self.search_mode().clone() {
                SearchMode::Fts => self.search_fts(),
                SearchMode::Fuzzy => self.search_fuzzy(),
                // Deferred modes: don't search on every keystroke (triggered via Enter)
                SearchMode::Semantic | SearchMode::Llm(_) => {}
            }
        }

        self.load_detail();
    }

    /// Mark that semantic search should be triggered on next tick.
    pub fn request_semantic_search(&mut self) {
        if self.input.is_empty() {
            self.results = dedup_by_session(&self.all_chunks);
            self.load_detail();
            return;
        }
        self.semantic_searching = true;
    }

    /// Actually run the semantic search (called from event loop after UI redraw).
    pub fn run_pending_semantic_search(&mut self) {
        if !self.semantic_searching {
            return;
        }
        self.semantic_searching = false;

        let params = SearchParams {
            query: self.input.clone(),
            limit: 50,
            source: None,
            project: None,
            after: None,
            before: None,
        };

        match semantic::search(&params) {
            Ok(results) => self.results = dedup_by_session(&results),
            Err(_) => self.results.clear(),
        }

        self.selected = 0;
        self.detail_scroll = 0;
        self.load_detail();
    }

    /// Mark that LLM search should be triggered on next tick.
    pub fn request_llm_search(&mut self) {
        if self.input.is_empty() {
            self.results = dedup_by_session(&self.all_chunks);
            self.load_detail();
            return;
        }
        self.llm_searching = true;
    }

    /// Actually run the LLM search (called from event loop after UI redraw).
    /// The LLM generates FTS queries, which are then executed and merged.
    pub fn run_pending_llm_search(&mut self) {
        if !self.llm_searching {
            return;
        }
        self.llm_searching = false;

        let backend = match self.search_mode().clone() {
            SearchMode::Llm(b) => b,
            _ => return,
        };

        let prompt = llm::build_query_gen_prompt(&self.input);

        let queries = match llm::invoke(&backend, &prompt) {
            Ok(response) => {
                let parsed = llm::parse_query_gen_response(&response);
                if parsed.is_empty() {
                    // Fallback: use the original query as-is
                    vec![self.input.clone()]
                } else {
                    parsed
                }
            }
            Err(_) => {
                // On error, fall back to plain FTS with user query
                vec![self.input.clone()]
            }
        };

        // Run each generated query and merge results
        let mut all_results = Vec::new();
        for query in queries {
            let params = SearchParams {
                query,
                limit: 30,
                source: None,
                project: None,
                after: None,
                before: None,
            };
            if let Ok(results) = self.engine.search(&params) {
                all_results.extend(results);
            }
        }

        self.results = dedup_by_session_best_score(&all_results);
        self.selected = 0;
        self.detail_scroll = 0;
        self.load_detail();
    }

    fn search_fts(&mut self) {
        let params = SearchParams {
            query: self.input.clone(),
            limit: 50,
            source: None,
            project: None,
            after: None,
            before: None,
        };

        match self.engine.search(&params) {
            Ok(results) => self.results = dedup_by_session(&results),
            Err(_) => self.results.clear(),
        }
    }

    fn search_fuzzy(&mut self) {
        let query = self.input.to_lowercase();
        let filtered: Vec<SearchResult> = self
            .all_chunks
            .iter()
            .filter(|c| {
                c.snippet.to_lowercase().contains(&query)
                    || c.project.to_lowercase().contains(&query)
                    || c.title
                        .as_deref()
                        .is_some_and(|t| t.to_lowercase().contains(&query))
            })
            .cloned()
            .collect();

        self.results = dedup_by_session(&filtered);
    }

    pub fn load_detail(&mut self) {
        if self.results.is_empty() {
            self.detail_chunks.clear();
            self.cached_session_id = None;
            return;
        }

        let session_id = &self.results[self.selected].session_id;

        // Cache: don't reload if same session
        if self.cached_session_id.as_deref() == Some(session_id) {
            return;
        }

        match self.engine.get_session_chunks(session_id) {
            Ok(chunks) => {
                self.detail_chunks = chunks;
                self.detail_scroll = 0;
                self.cached_session_id = Some(session_id.clone());
            }
            Err(_) => {
                self.detail_chunks.clear();
                self.cached_session_id = None;
            }
        }
    }

    pub fn select_next(&mut self) {
        if !self.results.is_empty() {
            self.selected = (self.selected + 1).min(self.results.len() - 1);
            self.load_detail();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.results.is_empty() {
            self.selected = self.selected.saturating_sub(1);
            self.load_detail();
        }
    }

    pub fn scroll_detail_down(&mut self) {
        self.detail_scroll += 5;
    }

    pub fn scroll_detail_up(&mut self) {
        self.detail_scroll = self.detail_scroll.saturating_sub(5);
    }

    pub fn scroll_detail_top(&mut self) {
        self.detail_scroll = 0;
    }

    pub fn scroll_detail_bottom(&mut self) {
        self.detail_scroll = usize::MAX / 2;
    }

    pub fn toggle_mode(&mut self) {
        self.mode_index = (self.mode_index + 1) % self.available_modes.len();
        self.on_input_changed();
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Search => {
                self.results_pane = ResultsPane::List;
                Focus::Results
            }
            Focus::Results => Focus::Search,
        };
    }

    pub fn resume_selected(&mut self) {
        if let Some(r) = self.results.get(self.selected) {
            let session_dir_exists = std::path::Path::new(&r.project).is_dir();
            self.confirm_resume = Some(ResumeConfirmState {
                session_id: r.session_id.clone(),
                source: r.source,
                project: r.project.clone(),
                title: r.title.clone(),
                timestamp: r.timestamp,
                selected: 0,
                session_dir_exists,
            });
        }
    }

    pub fn confirm_resume_select(&mut self, option: ResumeOption) {
        match option {
            ResumeOption::SessionDir => {
                if let Some(state) = self.confirm_resume.take() {
                    if !state.session_dir_exists {
                        let _ = std::fs::create_dir_all(&state.project);
                    }
                    self.resume_session = Some((state.session_id, state.source, state.project));
                    self.should_quit = true;
                }
            }
            ResumeOption::CurrentDir => {
                if let Some(state) = self.confirm_resume.take() {
                    let cwd = std::env::current_dir()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| ".".into());
                    self.resume_session = Some((state.session_id, state.source, cwd));
                    self.should_quit = true;
                }
            }
            ResumeOption::Cancel => {
                self.confirm_resume = None;
            }
        }
    }

    pub fn resume_command(&self) -> Option<ResumeCommand> {
        let (session_id, source, project) = self.resume_session.as_ref()?;
        let args = match source {
            Source::ClaudeCode => vec!["claude".into(), "--resume".into(), session_id.clone()],
            Source::Copilot => vec!["copilot".into(), format!("--resume={session_id}")],
            Source::OpenCode => vec!["opencode".into(), "--session".into(), session_id.clone()],
            Source::Cursor => vec!["cursor".into(), project.clone()],
            Source::Codex => vec!["codex".into(), "resume".into(), session_id.clone()],
        };
        Some(ResumeCommand {
            args,
            cwd: Some(project.clone()),
        })
    }
}

pub struct ResumeCommand {
    pub args: Vec<String>,
    /// Working directory to cd into before exec (needed for Claude Code).
    pub cwd: Option<String>,
}

/// Dedup by session, keeping the highest-scoring result per session.
/// Final results sorted by score descending, then by timestamp descending.
fn dedup_by_session(results: &[SearchResult]) -> Vec<SearchResult> {
    let mut best: HashMap<String, SearchResult> = HashMap::new();
    for r in results {
        best.entry(r.session_id.clone())
            .and_modify(|existing| {
                if r.score > existing.score {
                    *existing = r.clone();
                }
            })
            .or_insert_with(|| r.clone());
    }
    let mut out: Vec<SearchResult> = best.into_values().collect();
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.timestamp.cmp(&a.timestamp))
    });
    out
}

/// Dedup by session, keeping the highest-scoring result per session.
/// Final results sorted by score descending, then by timestamp descending.
fn dedup_by_session_best_score(results: &[SearchResult]) -> Vec<SearchResult> {
    dedup_by_session(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn search_mode_labels() {
        assert_eq!(SearchMode::Fts.label(), "Full-Text Search");
        assert_eq!(SearchMode::Fuzzy.label(), "Fuzzy");

        // Without model override
        let backend = test_backend("claude");
        assert_eq!(SearchMode::Llm(backend).label(), "LLM (claude)");

        // With model override
        let mut backend = test_backend("claude");
        backend.model = Some("sonnet".into());
        assert_eq!(SearchMode::Llm(backend).label(), "LLM (claude:sonnet)");
    }

    #[test]
    fn dedup_by_session_removes_duplicates() {
        let ts = Utc::now();
        let results = vec![
            SearchResult {
                chunk_id: "a:s1:0".into(),
                session_id: "s1".into(),
                source: Source::ClaudeCode,
                project: "/p".into(),
                timestamp: ts,
                title: None,
                snippet: "chunk 0".into(),
                score: 1.0,
            },
            SearchResult {
                chunk_id: "a:s1:1".into(),
                session_id: "s1".into(),
                source: Source::ClaudeCode,
                project: "/p".into(),
                timestamp: ts,
                title: None,
                snippet: "chunk 1".into(),
                score: 0.9,
            },
            SearchResult {
                chunk_id: "a:s2:0".into(),
                session_id: "s2".into(),
                source: Source::OpenCode,
                project: "/q".into(),
                timestamp: ts,
                title: None,
                snippet: "other".into(),
                score: 0.8,
            },
        ];
        let deduped = dedup_by_session(&results);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].session_id, "s1");
        assert_eq!(deduped[0].chunk_id, "a:s1:0"); // keeps first
        assert_eq!(deduped[1].session_id, "s2");
    }

    #[test]
    fn dedup_empty() {
        let deduped = dedup_by_session(&[]);
        assert!(deduped.is_empty());
    }

    fn test_backend(name: &str) -> LlmBackend {
        LlmBackend {
            name: name.into(),
            binary: std::path::PathBuf::from("/usr/bin/test"),
            headless_args: vec!["-p"],
            model_flag: "--model",
            model: None,
        }
    }

    #[test]
    fn search_mode_is_deferred() {
        assert!(!SearchMode::Fts.is_deferred());
        assert!(!SearchMode::Fuzzy.is_deferred());
        assert!(SearchMode::Semantic.is_deferred());
        assert!(SearchMode::Llm(test_backend("claude")).is_deferred());
    }

    #[test]
    fn search_mode_is_llm() {
        assert!(!SearchMode::Fts.is_llm());
        assert!(!SearchMode::Fuzzy.is_llm());
        assert!(!SearchMode::Semantic.is_llm());
        assert!(SearchMode::Llm(test_backend("claude")).is_llm());
    }

    #[test]
    fn search_mode_semantic_label() {
        assert_eq!(SearchMode::Semantic.label(), "Semantic");
    }

    #[test]
    fn dedup_best_score_keeps_highest() {
        let ts = Utc::now();
        let results = vec![
            SearchResult {
                chunk_id: "a:s1:0".into(),
                session_id: "s1".into(),
                source: Source::ClaudeCode,
                project: "/p".into(),
                timestamp: ts,
                title: None,
                snippet: "low score".into(),
                score: 0.5,
            },
            SearchResult {
                chunk_id: "a:s2:0".into(),
                session_id: "s2".into(),
                source: Source::OpenCode,
                project: "/q".into(),
                timestamp: ts,
                title: None,
                snippet: "other".into(),
                score: 0.3,
            },
            SearchResult {
                chunk_id: "a:s1:1".into(),
                session_id: "s1".into(),
                source: Source::ClaudeCode,
                project: "/p".into(),
                timestamp: ts,
                title: None,
                snippet: "high score".into(),
                score: 0.9,
            },
        ];
        let deduped = dedup_by_session_best_score(&results);
        assert_eq!(deduped.len(), 2);
        // Sorted by score descending
        assert_eq!(deduped[0].session_id, "s1");
        assert_eq!(deduped[0].score, 0.9);
        assert_eq!(deduped[1].session_id, "s2");
        assert_eq!(deduped[1].score, 0.3);
    }
}
