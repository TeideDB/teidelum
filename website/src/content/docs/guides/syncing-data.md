---
title: Syncing Data
description: Pull data from external tools into Teidelum
---

Sync sources pull data from external APIs and split it into two streams:

1. **Structured records** → columnar tables (for SQL queries)
2. **Search documents** → full-text index (for search)

This dual-storage pattern ensures data is queryable both structurally and by content.

## The SyncSource Trait

Every sync source implements the `SyncSource` trait:

```rust
#[async_trait]
pub trait SyncSource: Send + Sync {
    /// Unique name for this source (e.g. "notion", "zulip").
    fn name(&self) -> &str;

    /// Run an incremental sync. The cursor is opaque state from the
    /// previous run (None on first sync).
    async fn sync(
        &self,
        cursor: Option<&str>,
    ) -> Result<(SyncOutput, Option<String>)>;
}
```

## SyncOutput

A sync run produces `SyncOutput` containing two collections:

```rust
pub struct SyncOutput {
    pub records: Vec<StructuredRecord>,
    pub documents: Vec<SearchDocument>,
}
```

### StructuredRecord

Columnar data destined for SQL tables:

```rust
pub struct StructuredRecord {
    pub table: String,
    pub fields: serde_json::Map<String, serde_json::Value>,
}
```

### SearchDocument

Free-text content destined for the search index:

```rust
pub struct SearchDocument {
    pub id: String,
    pub source: String,
    pub title: String,
    pub body: String,
    pub metadata: serde_json::Map<String, serde_json::Value>,
}
```

## Incremental Sync

The `cursor` parameter enables incremental sync:

1. **First sync**: `cursor` is `None`. Pull all data and return a cursor string.
2. **Subsequent syncs**: `cursor` contains the previous run's state. Pull only changed data since that cursor.

The cursor format is opaque — each source defines its own (timestamps, page tokens, etc.).

## Triggering Sync

Use the `sync` MCP tool:

```json
{ "source": "my_source" }
```

Or omit `source` to sync all registered sources.
