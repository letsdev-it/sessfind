use std::path::PathBuf;

use anyhow::Result;

use crate::config::Config;
use crate::models::SearchResult;

/// An LLM CLI backend that can be used for search re-ranking.
#[derive(Debug, Clone)]
pub struct LlmBackend {
    pub name: String,
    pub binary: PathBuf,
    pub headless_args: Vec<&'static str>,
    pub model_flag: &'static str,
    /// Model override from config. None = let the tool decide.
    pub model: Option<String>,
}

impl LlmBackend {
    /// Display label: "claude" or "claude:sonnet" if model override is set.
    pub fn display(&self) -> String {
        match &self.model {
            Some(m) => format!("{}:{}", self.name, m),
            None => self.name.clone(),
        }
    }
}

/// Detect installed LLM CLI tools that support headless mode.
/// Loads config to resolve per-backend model overrides.
pub fn detect_backends() -> Vec<LlmBackend> {
    let config = Config::load();
    let mut backends = Vec::new();

    let definitions: &[(&str, &str, &[&str], &str)] = &[
        ("claude", "claude", &["-p", "--no-session-persistence"], "--model"),
        ("opencode", "opencode", &["run"], "-m"),
        ("copilot", "copilot", &["-p"], "--model"),
    ];

    for &(name, bin, headless_args, model_flag) in definitions {
        if let Ok(path) = which::which(bin) {
            // Read model from config; empty string or missing = None
            let model = config
                .llm_model(name)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());

            backends.push(LlmBackend {
                name: name.to_string(),
                binary: path,
                headless_args: headless_args.to_vec(),
                model_flag,
                model,
            });
        }
    }

    backends
}

/// Build a re-ranking prompt from the user query and FTS candidate results.
pub fn build_rerank_prompt(query: &str, candidates: &[SearchResult]) -> String {
    let mut candidates_json = String::from("[\n");
    for (i, r) in candidates.iter().enumerate() {
        let snippet_preview: String = r
            .snippet
            .chars()
            .take(200)
            .map(|c| if c == '\n' { ' ' } else { c })
            .collect();
        let title = r.title.as_deref().unwrap_or("");
        if i > 0 {
            candidates_json.push_str(",\n");
        }
        candidates_json.push_str(&format!(
            r#"  {{"i":{i},"session_id":"{}","source":"{}","project":"{}","title":"{}","snippet":"{}"}}"#,
            r.session_id,
            r.source.as_str(),
            r.project.replace('"', "'"),
            title.replace('"', "'"),
            snippet_preview.replace('"', "'").replace('\\', ""),
        ));
    }
    candidates_json.push_str("\n]");

    format!(
        r#"You are a search re-ranking engine for AI coding session logs. Given a user query and candidate sessions, return the most relevant ones.

USER QUERY: {query}

CANDIDATES:
{candidates_json}

Return ONLY a JSON array of objects with "i" (candidate index) and "score" (0.0-1.0 relevance). Max 20 results, sorted by score descending. No other text, no markdown fences.
Example: [{{"i":0,"score":0.95}},{{"i":3,"score":0.8}}]"#
    )
}

/// Parse the LLM re-ranking response and map back to full SearchResult objects.
pub fn parse_rerank_response(response: &str, candidates: &[SearchResult]) -> Vec<SearchResult> {
    // Strip markdown code fences if present
    let json_str = response
        .trim()
        .strip_prefix("```json")
        .or_else(|| response.trim().strip_prefix("```"))
        .and_then(|s| s.strip_suffix("```"))
        .unwrap_or(response.trim());

    #[derive(serde::Deserialize)]
    struct RankedItem {
        i: usize,
        score: f32,
    }

    let ranked: Vec<RankedItem> = match serde_json::from_str(json_str.trim()) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    ranked
        .into_iter()
        .filter_map(|item| {
            candidates.get(item.i).map(|r| {
                let mut result = r.clone();
                result.score = item.score;
                result
            })
        })
        .collect()
}

/// Run LLM search: invoke the backend in headless mode with the given prompt.
pub fn search(backend: &LlmBackend, prompt: &str) -> Result<String> {
    let mut cmd = std::process::Command::new(&backend.binary);

    // Add headless args
    for arg in &backend.headless_args {
        cmd.arg(arg);
    }

    // Add model flag only if user configured a model override
    if let Some(ref model) = backend.model {
        cmd.arg(backend.model_flag).arg(model);
    }

    // For claude/copilot, prompt is positional arg at the end.
    // For opencode run, the message is also positional.
    cmd.arg(prompt);

    // Extra flags per backend
    match backend.name.as_str() {
        "claude" => {
            cmd.arg("--max-budget-usd").arg("0.05");
        }
        "copilot" => {
            cmd.arg("--allow-all-tools");
        }
        _ => {}
    }

    let output = cmd.output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("LLM search via {} failed: {stderr}", backend.name);
    }

    let stdout = String::from_utf8(output.stdout)?;
    Ok(stdout)
}


#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use crate::models::Source;

    fn sample_candidates() -> Vec<SearchResult> {
        vec![
            SearchResult {
                chunk_id: "c1".into(),
                session_id: "s1".into(),
                source: Source::ClaudeCode,
                project: "/project/a".into(),
                timestamp: Utc::now(),
                title: Some("Fix auth bug".into()),
                snippet: "USER: fix the auth bug\nASSISTANT: done".into(),
                score: 1.0,
            },
            SearchResult {
                chunk_id: "c2".into(),
                session_id: "s2".into(),
                source: Source::OpenCode,
                project: "/project/b".into(),
                timestamp: Utc::now(),
                title: None,
                snippet: "USER: add logging\nASSISTANT: added".into(),
                score: 0.9,
            },
        ]
    }

    #[test]
    fn build_prompt_contains_query_and_candidates() {
        let candidates = sample_candidates();
        let prompt = build_rerank_prompt("auth bug", &candidates);
        assert!(prompt.contains("auth bug"));
        assert!(prompt.contains("s1"));
        assert!(prompt.contains("s2"));
        assert!(prompt.contains("Fix auth bug"));
    }

    #[test]
    fn parse_valid_response() {
        let candidates = sample_candidates();
        let response = r#"[{"i":1,"score":0.95},{"i":0,"score":0.7}]"#;
        let results = parse_rerank_response(response, &candidates);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].session_id, "s2");
        assert_eq!(results[0].score, 0.95);
        assert_eq!(results[1].session_id, "s1");
    }

    #[test]
    fn parse_markdown_fenced_response() {
        let candidates = sample_candidates();
        let response = "```json\n[{\"i\":0,\"score\":0.8}]\n```";
        let results = parse_rerank_response(response, &candidates);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].session_id, "s1");
    }

    #[test]
    fn parse_invalid_response_returns_empty() {
        let candidates = sample_candidates();
        let results = parse_rerank_response("sorry I can't help", &candidates);
        assert!(results.is_empty());
    }

    #[test]
    fn parse_out_of_bounds_index_skipped() {
        let candidates = sample_candidates();
        let response = r#"[{"i":0,"score":0.9},{"i":99,"score":0.5}]"#;
        let results = parse_rerank_response(response, &candidates);
        assert_eq!(results.len(), 1);
    }
}
