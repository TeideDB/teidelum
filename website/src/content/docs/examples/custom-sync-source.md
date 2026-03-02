---
title: Custom Sync Source
description: Step-by-step guide to building a sync adapter
---

This example walks through building a sync source that pulls data from an external API.

## Step 1: Define the Struct

```rust
use anyhow::Result;
use async_trait::async_trait;
use teidelum::sync::{SearchDocument, StructuredRecord, SyncOutput, SyncSource};

pub struct MyAppSync {
    api_url: String,
    api_token: String,
}

impl MyAppSync {
    pub fn new(api_url: String, api_token: String) -> Self {
        Self { api_url, api_token }
    }
}
```

## Step 2: Implement the Trait

```rust
#[async_trait]
impl SyncSource for MyAppSync {
    fn name(&self) -> &str {
        "myapp"
    }

    async fn sync(
        &self,
        cursor: Option<&str>,
    ) -> Result<(SyncOutput, Option<String>)> {
        // 1. Fetch data from API (using cursor for incremental sync)
        let items = self.fetch_items(cursor).await?;

        let mut output = SyncOutput::default();

        for item in &items {
            // 2. Create structured record for SQL queries
            let mut fields = serde_json::Map::new();
            fields.insert("id".into(), item.id.clone().into());
            fields.insert("title".into(), item.title.clone().into());
            fields.insert("status".into(), item.status.clone().into());

            output.records.push(StructuredRecord {
                table: "myapp_items".into(),
                fields,
            });

            // 3. Create search document for full-text search
            output.documents.push(SearchDocument {
                id: format!("myapp-{}", item.id),
                source: "myapp".into(),
                title: item.title.clone(),
                body: item.description.clone(),
                metadata: serde_json::Map::new(),
            });
        }

        // 4. Return new cursor for next incremental sync
        let new_cursor = items.last().map(|i| i.updated_at.clone());

        Ok((output, new_cursor))
    }
}
```

## Step 3: Handle Incremental Sync

The cursor enables pulling only new/changed data:

```rust
impl MyAppSync {
    async fn fetch_items(&self, cursor: Option<&str>) -> Result<Vec<Item>> {
        let url = match cursor {
            Some(since) => format!("{}/items?updated_since={}", self.api_url, since),
            None => format!("{}/items", self.api_url),
        };
        // ... HTTP request and parsing
    }
}
```

## Key Points

- **Dual output**: Always produce both structured records (for SQL) and search documents (for full-text search) when the data supports it.
- **Incremental cursors**: Use timestamps, page tokens, or any opaque string that lets you resume from where you left off.
- **Idempotent**: Multiple syncs with the same cursor should produce the same result. The system handles deduplication at the storage layer.
