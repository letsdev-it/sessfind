use std::sync::mpsc;

use chrono::{DateTime, Utc};

use crate::indexer::engine::{IndexEngine, SearchParams};
use crate::llm::{self, LlmBackend};
use crate::models::{SearchResult, Source};
use crate::search::results::{SortOrder, apply_sort, dedup_by_session};
use crate::semantic;
use sessfind_common::CommandSpec;

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
    pub sort_order: SortOrder,
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
    pub fn new(engine: &'a IndexEngine, initial_mode: Option<&str>) -> anyhow::Result<Self> {
        let all_chunks = engine.list_all_chunks()?;

        // Build available modes: FTS, Fuzzy, [Semantic], [LLM backends]
        let mut available_modes: Vec<SearchMode> = vec![SearchMode::Fts, SearchMode::Fuzzy];
        if semantic::is_available() {
            available_modes.push(SearchMode::Semantic);
        }
        for backend in llm::detect_backends() {
            available_modes.push(SearchMode::Llm(backend));
        }

        // Resolve initial mode index
        let mode_index = initial_mode
            .and_then(|m| {
                let m = m.to_lowercase();
                available_modes.iter().position(|mode| match mode {
                    SearchMode::Fts => m == "fts",
                    SearchMode::Fuzzy => m == "fuzzy",
                    SearchMode::Semantic => m == "semantic",
                    SearchMode::Llm(_) => m == "llm",
                })
            })
            .unwrap_or(0);

        // Show all sessions initially (deduplicated)
        let results = dedup_by_session(&all_chunks, SortOrder::TimeDesc);

        let update_rx = crate::version_check::check_latest_version_async();

        let mut app = Self {
            input: String::new(),
            cursor_pos: 0,
            results,
            selected: 0,
            detail_chunks: Vec::new(),
            detail_scroll: 0,
            available_modes,
            mode_index,
            semantic_searching: false,
            llm_searching: false,
            focus: Focus::Search,
            results_pane: ResultsPane::List,
            sort_order: SortOrder::TimeDesc,
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
            self.results = dedup_by_session(&self.all_chunks, self.sort_order);
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
            self.results = dedup_by_session(&self.all_chunks, self.sort_order);
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
            Ok(results) => self.results = dedup_by_session(&results, self.sort_order),
            Err(_) => self.results.clear(),
        }

        self.selected = 0;
        self.detail_scroll = 0;
        self.load_detail();
    }

    /// Mark that LLM search should be triggered on next tick.
    pub fn request_llm_search(&mut self) {
        if self.input.is_empty() {
            self.results = dedup_by_session(&self.all_chunks, self.sort_order);
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

        let base = SearchParams {
            query: String::new(),
            limit: 30,
            source: None,
            project: None,
            after: None,
            before: None,
        };

        let merged = match llm::expanded_search(self.engine, &backend, &self.input, &base) {
            Ok(expanded) => expanded.results,
            Err(_) => {
                // On error, fall back to plain FTS with the user query
                let params = SearchParams {
                    query: self.input.clone(),
                    ..base
                };
                self.engine.search(&params).unwrap_or_default()
            }
        };

        self.results = dedup_by_session(&merged, self.sort_order);
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
            Ok(results) => self.results = dedup_by_session(&results, self.sort_order),
            Err(_) => self.results.clear(),
        }
    }

    fn search_fuzzy(&mut self) {
        let params = SearchParams {
            query: self.input.clone(),
            limit: 50,
            source: None,
            project: None,
            after: None,
            before: None,
        };

        match self.engine.search_fuzzy(&params) {
            Ok(results) => self.results = dedup_by_session(&results, self.sort_order),
            Err(_) => self.results.clear(),
        }
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
        self.detail_scroll = self.detail_scroll.saturating_add(5);
    }

    pub fn scroll_detail_up(&mut self) {
        self.detail_scroll = self.detail_scroll.saturating_sub(5);
    }

    pub fn scroll_detail_top(&mut self) {
        self.detail_scroll = 0;
    }

    pub fn toggle_mode(&mut self) {
        self.mode_index = (self.mode_index + 1) % self.available_modes.len();
        self.on_input_changed();
    }

    pub fn toggle_sort(&mut self) {
        self.sort_order = self.sort_order.next();
        apply_sort(&mut self.results, self.sort_order);
        self.selected = 0;
        self.load_detail();
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

    pub fn resume_command(&self) -> Option<CommandSpec> {
        let (session_id, source, project) = self.resume_session.as_ref()?;
        Some(sessfind_common::resume_command(
            *source, session_id, project,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
