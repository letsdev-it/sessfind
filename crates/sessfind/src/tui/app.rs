use std::collections::HashSet;

use crate::indexer::engine::{IndexEngine, SearchParams};
use crate::models::{SearchResult, Source};
use crate::semantic;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    Fts,
    Fuzzy,
    Semantic,
}

impl SearchMode {
    pub fn label(&self) -> &'static str {
        match self {
            SearchMode::Fts => "Full-Text Search",
            SearchMode::Fuzzy => "Fuzzy",
            SearchMode::Semantic => "Semantic",
        }
    }

    pub fn next(&self, semantic_available: bool) -> Self {
        match self {
            SearchMode::Fts => SearchMode::Fuzzy,
            SearchMode::Fuzzy => {
                if semantic_available {
                    SearchMode::Semantic
                } else {
                    SearchMode::Fts
                }
            }
            SearchMode::Semantic => SearchMode::Fts,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Search,
    Results,
}

pub struct App<'a> {
    pub input: String,
    pub cursor_pos: usize,
    pub results: Vec<SearchResult>,
    pub selected: usize,
    pub detail_chunks: Vec<SearchResult>,
    pub detail_scroll: usize,
    pub search_mode: SearchMode,
    pub semantic_available: bool,
    pub semantic_searching: bool,
    pub focus: Focus,
    pub should_quit: bool,
    pub resume_session: Option<(String, Source, String)>, // (session_id, source, project)
    pub show_help: bool,
    engine: &'a IndexEngine,
    all_chunks: Vec<SearchResult>,
    cached_session_id: Option<String>,
}

impl<'a> App<'a> {
    pub fn new(engine: &'a IndexEngine) -> anyhow::Result<Self> {
        let all_chunks = engine.list_all_chunks()?;
        let semantic_available = semantic::is_available();

        // Show all sessions initially (deduplicated)
        let results = dedup_by_session(&all_chunks);

        let mut app = Self {
            input: String::new(),
            cursor_pos: 0,
            results,
            selected: 0,
            detail_chunks: Vec::new(),
            detail_scroll: 0,
            search_mode: SearchMode::Fts,
            semantic_available,
            semantic_searching: false,
            focus: Focus::Search,
            should_quit: false,
            resume_session: None,
            show_help: false,
            engine,
            all_chunks,
            cached_session_id: None,
        };

        app.load_detail();
        Ok(app)
    }

    pub fn on_input_changed(&mut self) {
        self.selected = 0;
        self.detail_scroll = 0;

        if self.input.is_empty() {
            self.results = dedup_by_session(&self.all_chunks);
        } else {
            match self.search_mode {
                SearchMode::Fts => self.search_fts(),
                SearchMode::Fuzzy => self.search_fuzzy(),
                // Semantic: don't search on every keystroke (debounced via Enter)
                SearchMode::Semantic => {}
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

    pub fn toggle_mode(&mut self) {
        self.search_mode = self.search_mode.next(self.semantic_available);
        self.on_input_changed();
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Search => Focus::Results,
            Focus::Results => Focus::Search,
        };
    }

    pub fn resume_selected(&mut self) {
        if let Some(r) = self.results.get(self.selected) {
            self.resume_session = Some((r.session_id.clone(), r.source, r.project.clone()));
            self.should_quit = true;
        }
    }

    pub fn resume_command(&self) -> Option<ResumeCommand> {
        let (session_id, source, project) = self.resume_session.as_ref()?;
        let args = match source {
            Source::ClaudeCode => vec!["claude".into(), "--resume".into(), session_id.clone()],
            Source::Copilot => vec!["copilot".into(), format!("--resume={session_id}")],
            Source::OpenCode => vec!["opencode".into(), "--session".into(), session_id.clone()],
        };
        Some(ResumeCommand {
            args,
            cwd: if *source == Source::ClaudeCode {
                Some(project.clone())
            } else {
                None
            },
        })
    }
}

pub struct ResumeCommand {
    pub args: Vec<String>,
    /// Working directory to cd into before exec (needed for Claude Code).
    pub cwd: Option<String>,
}

fn dedup_by_session(results: &[SearchResult]) -> Vec<SearchResult> {
    let mut seen = HashSet::new();
    results
        .iter()
        .filter(|r| seen.insert(r.session_id.clone()))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn search_mode_next_without_semantic() {
        assert_eq!(SearchMode::Fts.next(false), SearchMode::Fuzzy);
        assert_eq!(SearchMode::Fuzzy.next(false), SearchMode::Fts);
    }

    #[test]
    fn search_mode_next_with_semantic() {
        assert_eq!(SearchMode::Fts.next(true), SearchMode::Fuzzy);
        assert_eq!(SearchMode::Fuzzy.next(true), SearchMode::Semantic);
        assert_eq!(SearchMode::Semantic.next(true), SearchMode::Fts);
    }

    #[test]
    fn search_mode_labels() {
        assert_eq!(SearchMode::Fts.label(), "Full-Text Search");
        assert_eq!(SearchMode::Fuzzy.label(), "Fuzzy");
        assert_eq!(SearchMode::Semantic.label(), "Semantic");
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
}
