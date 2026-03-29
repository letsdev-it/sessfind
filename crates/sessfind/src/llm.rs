use std::path::PathBuf;

use anyhow::Result;

use crate::config::Config;

/// An LLM CLI backend that can be used for agentic search.
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
        ("claude", "claude", &["-p"], "--model"),
        ("opencode", "opencode", &["run"], "-m"),
        ("copilot", "copilot", &["-p"], "--model"),
    ];

    for &(name, bin, headless_args, model_flag) in definitions {
        if let Ok(path) = which::which(bin) {
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

/// Build a prompt that asks the LLM to generate FTS queries for the user's intent.
pub fn build_query_gen_prompt(user_query: &str) -> String {
    format!(
        r#"You are a search query generator for an AI coding session search engine.
The engine uses full-text search (tantivy/BM25) on conversation logs between users and AI assistants.

Query syntax supported:
- word1 word2 = OR (any word matches)
- +word1 +word2 = AND (all words required)
- "exact phrase" = phrase match
- -word = exclude word
- prefix* = prefix wildcard

User's search intent: {user_query}

Generate 10-15 FTS queries that would find relevant sessions. Think about:
- Exact keywords the conversation might contain
- Synonyms and related technical terms
- English AND Polish variants if the intent uses either language

Return ONLY a JSON array of query strings. No other text, no markdown fences.
Example: ["+CI +fix", "\"github actions\"", "pipeline deploy"]"#
    )
}

/// Parse the LLM response containing generated FTS queries.
pub fn parse_query_gen_response(response: &str) -> Vec<String> {
    let json_str = strip_markdown_fences(response);

    match serde_json::from_str::<Vec<String>>(json_str.trim()) {
        Ok(queries) => queries.into_iter().filter(|q| !q.is_empty()).collect(),
        Err(_) => Vec::new(),
    }
}

/// Strip markdown code fences from LLM response.
fn strip_markdown_fences(response: &str) -> &str {
    let trimmed = response.trim();
    trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|s| s.strip_suffix("```"))
        .map(|s| s.trim())
        .unwrap_or(trimmed)
}

/// Invoke the LLM backend in headless mode with the given prompt.
pub fn invoke(backend: &LlmBackend, prompt: &str) -> Result<String> {
    let mut cmd = std::process::Command::new(&backend.binary);

    for arg in &backend.headless_args {
        cmd.arg(arg);
    }

    if let Some(ref model) = backend.model {
        cmd.arg(backend.model_flag).arg(model);
    }

    cmd.arg(prompt);

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
        anyhow::bail!("LLM invocation via {} failed: {stderr}", backend.name);
    }

    let stdout = String::from_utf8(output.stdout)?;
    Ok(stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_gen_prompt_contains_intent() {
        let prompt = build_query_gen_prompt("fix CI pipeline");
        assert!(prompt.contains("fix CI pipeline"));
        assert!(prompt.contains("JSON array"));
    }

    #[test]
    fn parse_query_gen_valid() {
        let response = r#"["+CI +fix", "\"github actions\"", "pipeline deploy"]"#;
        let queries = parse_query_gen_response(response);
        assert_eq!(queries.len(), 3);
        assert_eq!(queries[0], "+CI +fix");
        assert_eq!(queries[1], "\"github actions\"");
        assert_eq!(queries[2], "pipeline deploy");
    }

    #[test]
    fn parse_query_gen_markdown_fenced() {
        let response = "```json\n[\"+auth\", \"login JWT\"]\n```";
        let queries = parse_query_gen_response(response);
        assert_eq!(queries.len(), 2);
        assert_eq!(queries[0], "+auth");
    }

    #[test]
    fn parse_query_gen_invalid_returns_empty() {
        let queries = parse_query_gen_response("sorry I can't help");
        assert!(queries.is_empty());
    }

    #[test]
    fn parse_query_gen_filters_empty_strings() {
        let response = r#"["auth", "", "login"]"#;
        let queries = parse_query_gen_response(response);
        assert_eq!(queries.len(), 2);
        assert_eq!(queries[0], "auth");
        assert_eq!(queries[1], "login");
    }

    #[test]
    fn strip_fences_plain() {
        assert_eq!(strip_markdown_fences("[\"a\"]"), "[\"a\"]");
    }

    #[test]
    fn strip_fences_json() {
        assert_eq!(strip_markdown_fences("```json\n[\"a\"]\n```"), "[\"a\"]");
    }

    #[test]
    fn strip_fences_bare() {
        assert_eq!(strip_markdown_fences("```\n[\"a\"]\n```"), "[\"a\"]");
    }
}
