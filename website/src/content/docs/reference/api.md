---
title: API Reference
description: TeidelumApi public methods and types
---

`TeidelumApi` is the unified programmatic interface. All subsystems are accessed through this single facade.

## Construction

### `TeidelumApi::new(data_dir: &Path) -> Result<Self>`

Create an empty instance. Initializes the search index, query router, catalog, and graph engine. No data is loaded.

### `TeidelumApi::open(data_dir: &Path) -> Result<Self>`

Open an existing data directory. Loads all splayed tables from `{data_dir}/tables/` and indexes all markdown documents from `{data_dir}/docs/`.

## Table Operations

### `create_table(name, source, columns, rows) -> Result<()>`

Create a new table with the given schema and data.

- `name: &str` — table name (must be a valid SQL identifier)
- `source: &str` — origin identifier
- `columns: &[ColumnSchema]` — column definitions
- `rows: &[Vec<Value>]` — row data

Rows are inserted in batches of 1,000. If insertion fails, the table is dropped (rollback).

### `register_table(entry: TableEntry)`

Register a pre-built table entry in the catalog (e.g., for remote connectors).

### `query(sql: &str) -> Result<QueryResult>`

Execute a SQL query against the local engine.

## Search Operations

### `add_documents(docs: &[SearchDocument]) -> Result<usize>`

Index documents into the full-text search engine. Returns the count of documents indexed.

### `search(query: &SearchQuery) -> Result<Vec<SearchResult>>`

Run a full-text search query.

## Relationship Operations

### `register_relationship(rel: Relationship) -> Result<()>`

Register a single FK relationship and rebuild the graph engine.

### `register_relationships(rels: Vec<Relationship>) -> Result<()>`

Register multiple relationships in bulk. Validates all relationships before mutating the catalog. Rebuilds the graph engine once at the end.

## Graph Operations

### `neighbors(table, key_col, key_value, depth, direction, rel_types) -> Result<JsonValue>`

Find all nodes reachable from the starting node up to the given depth.

### `path(table, key_col, key_value, to_table, to_key_col, to_key, depth, direction, rel_types) -> Result<JsonValue>`

Find the shortest path between two nodes.

## Catalog Operations

### `describe(source_filter: Option<&str>) -> Result<JsonValue>`

Return a JSON description of all tables and relationships, optionally filtered by source.

## Accessor Methods

### `search_engine() -> &Arc<SearchEngine>`

Access the search engine directly.

### `query_router() -> &Arc<QueryRouter>`

Access the query router directly.
