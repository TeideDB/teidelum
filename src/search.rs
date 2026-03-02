use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tantivy::collector::TopDocs;
use tantivy::directory::MmapDirectory;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::snippet::SnippetGenerator;
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument};

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
    index: Index,
    reader: IndexReader,
    _schema: Schema,
    f_id: Field,
    f_source: Field,
    f_title: Field,
    f_body: Field,
}

impl SearchEngine {
    /// Open or create a search index at the given path.
    pub fn open(path: &Path) -> Result<Self> {
        let (schema, f_id, f_source, f_title, f_body) = Self::build_schema();

        std::fs::create_dir_all(path)?;

        let mmap_dir = MmapDirectory::open(path)?;
        let index = if Index::exists(&mmap_dir)? {
            Index::open_in_dir(path)?
        } else {
            Index::create_in_dir(path, schema.clone())?
        };

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        Ok(Self {
            index,
            reader,
            _schema: schema,
            f_id,
            f_source,
            f_title,
            f_body,
        })
    }

    fn build_schema() -> (Schema, Field, Field, Field, Field) {
        let mut builder = Schema::builder();
        let f_id = builder.add_text_field("id", STRING | STORED);
        let f_source = builder.add_text_field("source", STRING | STORED);
        let f_title = builder.add_text_field("title", TEXT | STORED);
        let f_body = builder.add_text_field("body", TEXT | STORED);
        (builder.build(), f_id, f_source, f_title, f_body)
    }

    /// Index a batch of documents. Returns the number indexed.
    pub fn index_documents(
        &self,
        documents: &[(String, String, String, String)], // (id, source, title, body)
    ) -> Result<usize> {
        let mut writer: IndexWriter = self.index.writer(50_000_000)?;
        let count = documents.len();

        for (id, source, title, body) in documents {
            writer.add_document(doc!(
                self.f_id => id.as_str(),
                self.f_source => source.as_str(),
                self.f_title => title.as_str(),
                self.f_body => body.as_str(),
            ))?;
        }

        writer.commit()?;
        self.reader.reload()?;

        Ok(count)
    }

    /// Delete documents by their id field. Returns the number of delete operations issued.
    pub fn delete_documents(&self, ids: &[String]) -> Result<usize> {
        let mut writer: IndexWriter = self.index.writer(50_000_000)?;
        let mut count = 0;

        for id in ids {
            let term = tantivy::Term::from_field_text(self.f_id, id);
            writer.delete_term(term);
            count += 1;
        }

        writer.commit()?;
        self.reader.reload()?;

        Ok(count)
    }

    /// Run a full-text search query.
    pub fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>> {
        let searcher = self.reader.searcher();

        let query_parser = QueryParser::for_index(&self.index, vec![self.f_title, self.f_body]);
        let parsed = query_parser.parse_query(&query.text)?;

        let top_docs = searcher.search(&parsed, &TopDocs::with_limit(query.limit))?;

        let snippet_gen = SnippetGenerator::create(&searcher, &*parsed, self.f_body)?;

        let mut results = Vec::new();
        for (score, doc_addr) in top_docs {
            let doc: TantivyDocument = searcher.doc(doc_addr)?;

            let id = doc
                .get_first(self.f_id)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let source = doc
                .get_first(self.f_source)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let title = doc
                .get_first(self.f_title)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // Filter by source if specified
            if let Some(ref sources) = query.sources {
                if !sources.iter().any(|s| s == &source) {
                    continue;
                }
            }

            let snippet = snippet_gen.snippet_from_doc(&doc);
            let snippet_text = snippet.to_html();

            results.push(SearchResult {
                id,
                source,
                title,
                snippet: snippet_text,
                score,
            });
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delete_documents() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = SearchEngine::open(tmp.path()).unwrap();

        // Index 3 docs
        let docs = vec![
            ("d1".to_string(), "test".to_string(), "Alpha".to_string(), "alpha content".to_string()),
            ("d2".to_string(), "test".to_string(), "Beta".to_string(), "beta content".to_string()),
            ("d3".to_string(), "test".to_string(), "Gamma".to_string(), "gamma content".to_string()),
        ];
        engine.index_documents(&docs).unwrap();

        // Delete d1 and d3
        let deleted = engine.delete_documents(&["d1".to_string(), "d3".to_string()]).unwrap();
        assert_eq!(deleted, 2);

        // Search should only find d2
        let results = engine
            .search(&SearchQuery {
                text: "content".to_string(),
                sources: None,
                limit: 10,
                date_from: None,
                date_to: None,
            })
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "d2");
    }

    #[test]
    fn test_delete_documents_nonexistent() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = SearchEngine::open(tmp.path()).unwrap();

        // delete_documents returns count of delete operations, not matched docs
        let deleted = engine.delete_documents(&["ghost".to_string()]).unwrap();
        assert_eq!(deleted, 1);
    }
}
