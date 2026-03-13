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

    /// Run a full-text search query with prefix matching on the last term.
    pub fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>> {
        let searcher = self.reader.searcher();

        let mut query_parser =
            QueryParser::for_index(&self.index, vec![self.f_title, self.f_body]);
        query_parser.set_conjunction_by_default();

        // Build a query that does prefix matching on the last word so partial
        // input works (e.g. "hel" matches "hello").  Earlier words use exact
        // term matching.  If there is only one word we still prefix-match it.
        let raw = query.text.trim().to_lowercase();
        let words: Vec<&str> = raw.split_whitespace().collect();

        let parsed: Box<dyn tantivy::query::Query> = if words.is_empty() {
            query_parser.parse_query(&query.text)?
        } else {
            use tantivy::query::{BooleanQuery, FuzzyTermQuery, Occur, PhrasePrefixQuery};
            let mut clauses: Vec<(Occur, Box<dyn tantivy::query::Query>)> = Vec::new();

            // For each complete word (all except last), add a fuzzy term query
            // across both title and body fields so minor typos still match.
            for &word in &words[..words.len() - 1] {
                let mut field_clauses: Vec<(Occur, Box<dyn tantivy::query::Query>)> = Vec::new();
                for field in [self.f_title, self.f_body] {
                    let term = tantivy::Term::from_field_text(field, word);
                    let fq = FuzzyTermQuery::new(term, 1, true);
                    field_clauses.push((Occur::Should, Box::new(fq)));
                }
                clauses.push((Occur::Must, Box::new(BooleanQuery::new(field_clauses))));
            }

            // Last word gets prefix matching so results appear as the user types.
            let last = words[words.len() - 1];
            let mut prefix_clauses: Vec<(Occur, Box<dyn tantivy::query::Query>)> = Vec::new();
            for field in [self.f_title, self.f_body] {
                let pq = PhrasePrefixQuery::new_with_offset(
                    vec![(0, tantivy::Term::from_field_text(field, last))],
                );
                prefix_clauses.push((Occur::Should, Box::new(pq)));
                // Also add fuzzy match for the last word in case it's a complete
                // word with a typo.
                let term = tantivy::Term::from_field_text(field, last);
                let fq = FuzzyTermQuery::new(term, 1, true);
                prefix_clauses.push((Occur::Should, Box::new(fq)));
            }
            clauses.push((Occur::Must, Box::new(BooleanQuery::new(prefix_clauses))));

            Box::new(BooleanQuery::new(clauses))
        };

        let top_docs = searcher.search(&parsed, &TopDocs::with_limit(query.limit))?;

        // Use a simple parsed query for snippet highlighting since
        // SnippetGenerator doesn't highlight FuzzyTermQuery/PhrasePrefixQuery.
        let snippet_parser =
            QueryParser::for_index(&self.index, vec![self.f_title, self.f_body]);
        let snippet_query = snippet_parser.parse_query(&query.text)?;
        let snippet_gen = SnippetGenerator::create(&searcher, &*snippet_query, self.f_body)?;

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
            (
                "d1".to_string(),
                "test".to_string(),
                "Alpha".to_string(),
                "alpha content".to_string(),
            ),
            (
                "d2".to_string(),
                "test".to_string(),
                "Beta".to_string(),
                "beta content".to_string(),
            ),
            (
                "d3".to_string(),
                "test".to_string(),
                "Gamma".to_string(),
                "gamma content".to_string(),
            ),
        ];
        engine.index_documents(&docs).unwrap();

        // Delete d1 and d3
        let deleted = engine
            .delete_documents(&["d1".to_string(), "d3".to_string()])
            .unwrap();
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

    #[test]
    fn test_index_and_search_basic() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = SearchEngine::open(tmp.path()).unwrap();

        let docs = vec![(
            "doc1".to_string(),
            "test".to_string(),
            "Getting Started".to_string(),
            "This guide covers installation and setup of the application".to_string(),
        )];
        engine.index_documents(&docs).unwrap();

        let results = engine
            .search(&SearchQuery {
                text: "installation".to_string(),
                sources: None,
                limit: 10,
                date_from: None,
                date_to: None,
            })
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "doc1");
        assert_eq!(results[0].source, "test");
        assert_eq!(results[0].title, "Getting Started");
    }

    #[test]
    fn test_search_title_match() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = SearchEngine::open(tmp.path()).unwrap();

        let docs = vec![(
            "doc1".to_string(),
            "test".to_string(),
            "Kubernetes Deployment Guide".to_string(),
            "This document covers container orchestration".to_string(),
        )];
        engine.index_documents(&docs).unwrap();

        let results = engine
            .search(&SearchQuery {
                text: "Kubernetes".to_string(),
                sources: None,
                limit: 10,
                date_from: None,
                date_to: None,
            })
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "doc1");
    }

    #[test]
    fn test_search_returns_scores() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = SearchEngine::open(tmp.path()).unwrap();

        let docs = vec![
            (
                "d1".to_string(),
                "t".to_string(),
                "Auth".to_string(),
                "authentication authentication authentication".to_string(),
            ),
            (
                "d2".to_string(),
                "t".to_string(),
                "Other".to_string(),
                "something else with authentication mentioned once".to_string(),
            ),
        ];
        engine.index_documents(&docs).unwrap();

        let results = engine
            .search(&SearchQuery {
                text: "authentication".to_string(),
                sources: None,
                limit: 10,
                date_from: None,
                date_to: None,
            })
            .unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.score > 0.0));
        assert!(results[0].score >= results[1].score);
    }

    #[test]
    fn test_search_source_filter() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = SearchEngine::open(tmp.path()).unwrap();

        let docs = vec![
            (
                "d1".to_string(),
                "notion".to_string(),
                "Notion Doc".to_string(),
                "important project documentation".to_string(),
            ),
            (
                "d2".to_string(),
                "zulip".to_string(),
                "Zulip Thread".to_string(),
                "important discussion thread".to_string(),
            ),
        ];
        engine.index_documents(&docs).unwrap();

        let results = engine
            .search(&SearchQuery {
                text: "important".to_string(),
                sources: Some(vec!["notion".to_string()]),
                limit: 10,
                date_from: None,
                date_to: None,
            })
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, "notion");
    }

    #[test]
    fn test_search_source_filter_excludes() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = SearchEngine::open(tmp.path()).unwrap();

        let docs = vec![(
            "d1".to_string(),
            "notion".to_string(),
            "Title".to_string(),
            "unique content here".to_string(),
        )];
        engine.index_documents(&docs).unwrap();

        let results = engine
            .search(&SearchQuery {
                text: "unique".to_string(),
                sources: Some(vec!["zulip".to_string()]),
                limit: 10,
                date_from: None,
                date_to: None,
            })
            .unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = SearchEngine::open(tmp.path()).unwrap();

        let docs: Vec<_> = (0..10)
            .map(|i| {
                (
                    format!("d{i}"),
                    "test".to_string(),
                    format!("Doc {i}"),
                    "common keyword repeated here".to_string(),
                )
            })
            .collect();
        engine.index_documents(&docs).unwrap();

        let results = engine
            .search(&SearchQuery {
                text: "keyword".to_string(),
                sources: None,
                limit: 3,
                date_from: None,
                date_to: None,
            })
            .unwrap();
        assert!(!results.is_empty(), "should return at least one result");
        assert!(results.len() <= 3, "should respect the limit");
    }

    #[test]
    fn test_search_no_results() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = SearchEngine::open(tmp.path()).unwrap();

        let docs = vec![(
            "d1".to_string(),
            "test".to_string(),
            "Title".to_string(),
            "some content".to_string(),
        )];
        engine.index_documents(&docs).unwrap();

        let results = engine
            .search(&SearchQuery {
                text: "xyznonexistent".to_string(),
                sources: None,
                limit: 10,
                date_from: None,
                date_to: None,
            })
            .unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_index_empty_batch() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = SearchEngine::open(tmp.path()).unwrap();

        let count = engine.index_documents(&[]).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_index_duplicate_ids() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = SearchEngine::open(tmp.path()).unwrap();

        let docs = vec![
            (
                "same_id".to_string(),
                "test".to_string(),
                "First".to_string(),
                "first version content".to_string(),
            ),
            (
                "same_id".to_string(),
                "test".to_string(),
                "Second".to_string(),
                "second version content".to_string(),
            ),
        ];
        let count = engine.index_documents(&docs).unwrap();
        assert_eq!(count, 2);

        let results = engine
            .search(&SearchQuery {
                text: "content".to_string(),
                sources: None,
                limit: 10,
                date_from: None,
                date_to: None,
            })
            .unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_snippet_contains_match() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = SearchEngine::open(tmp.path()).unwrap();

        let docs = vec![(
            "d1".to_string(),
            "test".to_string(),
            "Title".to_string(),
            "The authentication system uses JWT tokens for security".to_string(),
        )];
        engine.index_documents(&docs).unwrap();

        let results = engine
            .search(&SearchQuery {
                text: "authentication".to_string(),
                sources: None,
                limit: 10,
                date_from: None,
                date_to: None,
            })
            .unwrap();
        assert_eq!(results.len(), 1);
        let snippet = &results[0].snippet;
        assert!(
            snippet.contains("authentication") || snippet.contains("Authentication"),
            "snippet should reference matched term, got: {snippet}"
        );
    }

    #[test]
    fn test_search_multiple_terms() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = SearchEngine::open(tmp.path()).unwrap();

        let docs = vec![
            (
                "d1".to_string(),
                "test".to_string(),
                "Auth".to_string(),
                "authentication and authorization patterns".to_string(),
            ),
            (
                "d2".to_string(),
                "test".to_string(),
                "Deploy".to_string(),
                "deployment and monitoring setup".to_string(),
            ),
        ];
        engine.index_documents(&docs).unwrap();

        let results = engine
            .search(&SearchQuery {
                text: "authentication authorization".to_string(),
                sources: None,
                limit: 10,
                date_from: None,
                date_to: None,
            })
            .unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].id, "d1");
    }

    #[test]
    fn test_search_prefix_matching() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = SearchEngine::open(tmp.path()).unwrap();

        let docs = vec![
            (
                "d1".to_string(),
                "test".to_string(),
                "Hello World".to_string(),
                "authentication system for the application".to_string(),
            ),
            (
                "d2".to_string(),
                "test".to_string(),
                "Other".to_string(),
                "something unrelated entirely".to_string(),
            ),
        ];
        engine.index_documents(&docs).unwrap();

        // Partial prefix "auth" should match "authentication"
        let results = engine
            .search(&SearchQuery {
                text: "auth".to_string(),
                sources: None,
                limit: 10,
                date_from: None,
                date_to: None,
            })
            .unwrap();
        assert!(!results.is_empty(), "prefix 'auth' should match 'authentication'");
        assert_eq!(results[0].id, "d1");

        // Partial prefix "hel" should match title "Hello"
        let results = engine
            .search(&SearchQuery {
                text: "hel".to_string(),
                sources: None,
                limit: 10,
                date_from: None,
                date_to: None,
            })
            .unwrap();
        assert!(!results.is_empty(), "prefix 'hel' should match 'Hello'");
        assert_eq!(results[0].id, "d1");
    }
}
