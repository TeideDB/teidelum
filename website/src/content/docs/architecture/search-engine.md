---
title: Search Engine
description: Full-text indexing and search with tantivy
---

Teidelum uses [tantivy](https://github.com/quickwit-oss/tantivy) for full-text search. Tantivy is a Rust search engine library inspired by Apache Lucene.

## Index Schema

The search index has four fields:

| Field | Tantivy Type | Flags | Purpose |
|-------|-------------|-------|---------|
| `id` | text | STRING, STORED | Unique document ID, exact match only |
| `source` | text | STRING, STORED | Source attribution, exact match only |
| `title` | text | TEXT, STORED | Tokenized for full-text search |
| `body` | text | TEXT, STORED | Tokenized for full-text search |

`STRING` fields are indexed as-is (no tokenization). `TEXT` fields are tokenized into terms for full-text search.

## Indexing Pipeline

Documents are indexed in batches via `index_documents`:

1. Acquire an `IndexWriter` with a 50MB heap budget
2. Add each document to the writer
3. Commit the batch
4. Reload the reader so new documents are immediately searchable

## Search Pipeline

1. Parse the query using tantivy's `QueryParser` against `title` and `body` fields
2. Execute search with `TopDocs` collector (limited by `limit` parameter)
3. Generate highlighted snippets using `SnippetGenerator`
4. Filter results by source if `sources` parameter is set
5. Return results ordered by BM25 relevance score

## Storage

The search index is stored on disk using memory-mapped files (`MmapDirectory`). The index directory is at `{TEIDELUM_DATA}/index/`.

If the index directory already exists, it's opened. Otherwise, a new empty index is created.

## Reader Policy

The index reader uses `ReloadPolicy::OnCommitWithDelay` — it automatically picks up new documents after commits, with a small delay for efficiency.
