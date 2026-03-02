---
title: Full-Text Search
description: Index documents and run full-text search queries
---

Teidelum uses [tantivy](https://github.com/quickwit-oss/tantivy) for full-text search with BM25 ranking and fuzzy matching.

## How Documents Are Indexed

Documents are indexed with four fields:

| Field | Type | Purpose |
|-------|------|---------|
| `id` | STRING (stored) | Unique document identifier |
| `source` | STRING (stored) | Origin (e.g., "notion", "zulip") |
| `title` | TEXT (stored) | Document title — tokenized and searchable |
| `body` | TEXT (stored) | Full content — tokenized and searchable |

The `title` and `body` fields are tokenized for full-text search. The `id` and `source` fields are stored for filtering and attribution.

## Search Query Parameters

The `search` MCP tool accepts:

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `query` | string | required | Full-text search query |
| `sources` | string[] | all | Filter to specific sources |
| `limit` | number | 10 | Maximum results |
| `date_from` | string | none | ISO 8601 date lower bound |
| `date_to` | string | none | ISO 8601 date upper bound |

## Query Syntax

Queries use tantivy's query parser, which supports:

- **Simple terms**: `authentication` — matches documents containing the word
- **Multi-word**: `JWT token rotation` — matches documents containing any of the words (OR by default)
- **Phrase search**: `"token rotation"` — matches the exact phrase
- **Field-scoped**: `title:authentication` — search only in the title field

## Search Results

Each result includes:

```json
{
  "id": "auth-rfc",
  "source": "notion",
  "title": "Authentication RFC",
  "snippet": "This document covers <b>JWT</b> <b>authentication</b> and <b>token</b> management",
  "score": 12.34
}
```

- **snippet**: HTML-highlighted excerpt showing matched terms in context
- **score**: BM25 relevance score (higher = more relevant)

## Filtering by Source

To search only within specific sources:

```json
{
  "query": "deployment",
  "sources": ["notion"]
}
```
