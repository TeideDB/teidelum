---
title: Custom Connector
description: Step-by-step guide to building a live query adapter
---

This example walks through building a connector that queries an external database in real time.

## Step 1: Define the Struct

```rust
use anyhow::Result;
use async_trait::async_trait;
use teidelum::connector::{ColumnSchema, Connector, QueryResult, Value};

pub struct MyDbConnector {
    host: String,
    port: u16,
}

impl MyDbConnector {
    pub fn new(host: String, port: u16) -> Self {
        Self { host, port }
    }
}
```

## Step 2: Implement Table Discovery

```rust
#[async_trait]
impl Connector for MyDbConnector {
    fn name(&self) -> &str {
        "mydb"
    }

    async fn list_tables(&self) -> Result<Vec<(String, Vec<ColumnSchema>)>> {
        // Query the remote database for its schema
        Ok(vec![
            (
                "mydb_metrics".to_string(),
                vec![
                    ColumnSchema { name: "timestamp".into(), dtype: "timestamp".into() },
                    ColumnSchema { name: "metric".into(), dtype: "string".into() },
                    ColumnSchema { name: "value".into(), dtype: "f64".into() },
                ],
            ),
        ])
    }

    async fn query(&self, sql: &str) -> Result<QueryResult> {
        // Translate SQL to native query language and execute
        let native_query = self.translate_sql(sql)?;
        let raw_result = self.execute_native(&native_query).await?;
        self.convert_to_query_result(raw_result)
    }
}
```

## Step 3: SQL Translation

The hardest part of a connector is translating SQL to the source's native query language:

```rust
impl MyDbConnector {
    fn translate_sql(&self, sql: &str) -> Result<String> {
        // Parse the SQL and convert to your database's query format
        // This is source-specific — each database has its own language
        todo!("implement SQL translation")
    }
}
```

## Step 4: Register with Catalog

After implementing the connector, register its tables:

```rust
use teidelum::catalog::{TableEntry, ColumnInfo, StorageType};

// Register each table the connector exposes
api.register_table(TableEntry {
    name: "mydb_metrics".to_string(),
    source: "mydb".to_string(),
    storage: StorageType::Remote,  // Key: marks as remote
    columns: vec![
        ColumnInfo { name: "timestamp".into(), dtype: "timestamp".into() },
        ColumnInfo { name: "metric".into(), dtype: "string".into() },
        ColumnInfo { name: "value".into(), dtype: "f64".into() },
    ],
    row_count: None,  // Unknown for remote tables
});
```

## Key Differences from Sync Sources

| | Sync Source | Connector |
|---|---|---|
| **Data storage** | Copies data locally | Queries live, no local copy |
| **Latency** | Fast (local reads) | Depends on remote source |
| **Freshness** | As of last sync | Always current |
| **Use case** | Historical data, search | Real-time metrics, live queries |
