use anyhow::Result;
use serde::{Deserialize, Serialize};

/// A single search result with source attribution and relevance score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub source: String,
    pub title: String,
    pub snippet: String,
    pub score: f32,
}

/// Parameters for a full-text search query.
pub struct SearchQuery {
    pub text: String,
    pub sources: Option<Vec<String>>,
    pub limit: usize,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
}

/// Full-text search engine backed by tantivy.
pub struct SearchEngine {
    // TODO: tantivy Index + IndexReader
}

impl SearchEngine {
    /// Open or create a search index at the given path.
    pub fn open(_path: &std::path::Path) -> Result<Self> {
        // TODO: build tantivy schema, open/create index
        Ok(Self {})
    }

    /// Index a batch of documents.
    pub fn index(
        &self,
        _documents: &[teidelum_sync_core::SearchDocument],
    ) -> Result<usize> {
        // TODO: add documents to tantivy index, commit
        Ok(0)
    }

    /// Run a full-text search query.
    pub fn search(&self, _query: &SearchQuery) -> Result<Vec<SearchResult>> {
        // TODO: parse query, execute against tantivy, collect results
        Ok(Vec::new())
    }
}
