//! Shared post-processing of search results: sorting and per-session dedup.
//! Used by the TUI, the CLI and the JSON API so all frontends behave the same.

use std::collections::HashMap;

use crate::models::SearchResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    /// Time descending (newest first), then score descending as tiebreaker.
    TimeDesc,
    /// Score descending (best match first), then time descending as tiebreaker.
    ScoreDesc,
}

impl SortOrder {
    pub fn label(&self) -> &'static str {
        match self {
            SortOrder::TimeDesc => "Newest first",
            SortOrder::ScoreDesc => "Best match",
        }
    }

    pub fn next(self) -> Self {
        match self {
            SortOrder::TimeDesc => SortOrder::ScoreDesc,
            SortOrder::ScoreDesc => SortOrder::TimeDesc,
        }
    }
}

/// Apply sort order to a results vector in place.
pub fn apply_sort(results: &mut [SearchResult], order: SortOrder) {
    match order {
        SortOrder::TimeDesc => {
            results.sort_by(|a, b| {
                b.timestamp.cmp(&a.timestamp).then_with(|| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            });
        }
        SortOrder::ScoreDesc => {
            results.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| b.timestamp.cmp(&a.timestamp))
            });
        }
    }
}

/// Dedup by session, keeping the highest-scoring result per session.
/// Final results sorted according to the given sort order.
pub fn dedup_by_session(results: &[SearchResult], order: SortOrder) -> Vec<SearchResult> {
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
    apply_sort(&mut out, order);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Source;
    use chrono::Utc;

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
        let deduped = dedup_by_session(&results, SortOrder::ScoreDesc);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].session_id, "s1");
        assert_eq!(deduped[0].chunk_id, "a:s1:0"); // keeps first
        assert_eq!(deduped[1].session_id, "s2");
    }

    #[test]
    fn dedup_empty() {
        let deduped = dedup_by_session(&[], SortOrder::ScoreDesc);
        assert!(deduped.is_empty());
    }

    #[test]
    fn dedup_keeps_highest_score_per_session() {
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
        let deduped = dedup_by_session(&results, SortOrder::ScoreDesc);
        assert_eq!(deduped.len(), 2);
        // Sorted by score descending
        assert_eq!(deduped[0].session_id, "s1");
        assert_eq!(deduped[0].score, 0.9);
        assert_eq!(deduped[1].session_id, "s2");
        assert_eq!(deduped[1].score, 0.3);
    }

    #[test]
    fn sort_order_time_desc_sorts_by_timestamp() {
        use chrono::Duration;

        let now = Utc::now();
        let old = now - Duration::hours(2);

        let results = vec![
            SearchResult {
                chunk_id: "a:s1:0".into(),
                session_id: "s1".into(),
                source: Source::ClaudeCode,
                project: "/p".into(),
                timestamp: old,
                title: None,
                snippet: "old session".into(),
                score: 0.9,
            },
            SearchResult {
                chunk_id: "a:s2:0".into(),
                session_id: "s2".into(),
                source: Source::OpenCode,
                project: "/q".into(),
                timestamp: now,
                title: None,
                snippet: "new session".into(),
                score: 0.5,
            },
        ];
        let deduped = dedup_by_session(&results, SortOrder::TimeDesc);
        assert_eq!(deduped.len(), 2);
        // Newest first despite lower score
        assert_eq!(deduped[0].session_id, "s2");
        assert_eq!(deduped[1].session_id, "s1");
    }

    #[test]
    fn sort_order_toggle_cycles() {
        assert_eq!(SortOrder::TimeDesc.next(), SortOrder::ScoreDesc);
        assert_eq!(SortOrder::ScoreDesc.next(), SortOrder::TimeDesc);
    }

    #[test]
    fn sort_order_labels() {
        assert_eq!(SortOrder::TimeDesc.label(), "Newest first");
        assert_eq!(SortOrder::ScoreDesc.label(), "Best match");
    }

    #[test]
    fn apply_sort_reorders_in_place() {
        use chrono::Duration;

        let now = Utc::now();
        let old = now - Duration::hours(2);

        let mut results = vec![
            SearchResult {
                chunk_id: "a:s1:0".into(),
                session_id: "s1".into(),
                source: Source::ClaudeCode,
                project: "/p".into(),
                timestamp: old,
                title: None,
                snippet: "old high score".into(),
                score: 0.9,
            },
            SearchResult {
                chunk_id: "a:s2:0".into(),
                session_id: "s2".into(),
                source: Source::OpenCode,
                project: "/q".into(),
                timestamp: now,
                title: None,
                snippet: "new low score".into(),
                score: 0.5,
            },
        ];

        // ScoreDesc: highest score first
        apply_sort(&mut results, SortOrder::ScoreDesc);
        assert_eq!(results[0].session_id, "s1");
        assert_eq!(results[1].session_id, "s2");

        // TimeDesc: newest first
        apply_sort(&mut results, SortOrder::TimeDesc);
        assert_eq!(results[0].session_id, "s2");
        assert_eq!(results[1].session_id, "s1");
    }
}
