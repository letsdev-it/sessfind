use anyhow::Result;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

const MAX_CHARS: usize = 1600;

pub struct Embedder {
    model: TextEmbedding,
}

impl Embedder {
    pub fn new() -> Result<Self> {
        let model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::MultilingualE5Small).with_show_download_progress(true),
        )?;
        Ok(Self { model })
    }

    /// Embed a single query string (prefixed with "query: " for e5 convention).
    pub fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        let text = format!("query: {}", truncate(query));
        let embeddings = self.model.embed(vec![text], None)?;
        Ok(embeddings.into_iter().next().unwrap())
    }

    /// Embed a batch of passages (prefixed with "passage: " for e5 convention).
    pub fn embed_passages(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let prefixed: Vec<String> = texts
            .iter()
            .map(|t| format!("passage: {}", truncate(t)))
            .collect();
        let embeddings = self.model.embed(prefixed, None)?;
        Ok(embeddings)
    }
}

fn truncate(s: &str) -> &str {
    if s.len() <= MAX_CHARS {
        return s;
    }
    // Find a char boundary near MAX_CHARS
    let mut end = MAX_CHARS;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}
