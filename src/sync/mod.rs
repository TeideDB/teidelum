pub mod notion;
pub mod zulip;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A structured record to be inserted into a columnar table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredRecord {
    /// Target table name (e.g. "notion_pages", "zulip_messages").
    pub table: String,
    /// Column name -> JSON value pairs.
    pub fields: serde_json::Map<String, serde_json::Value>,
}

/// A document to be indexed for full-text search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchDocument {
    /// Unique document ID.
    pub id: String,
    /// Source identifier (e.g. "notion", "zulip").
    pub source: String,
    /// Document title.
    pub title: String,
    /// Full text content for indexing.
    pub body: String,
    /// Optional metadata fields.
    pub metadata: serde_json::Map<String, serde_json::Value>,
}

/// Output of a single sync run.
#[derive(Debug, Default)]
pub struct SyncOutput {
    pub records: Vec<StructuredRecord>,
    pub documents: Vec<SearchDocument>,
}

/// Status returned after a sync completes.
#[derive(Debug, Serialize, Deserialize)]
pub struct SyncStatus {
    pub source: String,
    pub added: usize,
    pub updated: usize,
    pub deleted: usize,
}

/// A sync source pulls data from an external API, transforms it into
/// structured records (for columnar storage) and search documents
/// (for full-text indexing).
///
/// Sync is incremental: implementations track a cursor/checkpoint
/// to pull only changed data on subsequent runs.
#[async_trait]
pub trait SyncSource: Send + Sync {
    /// Unique name for this source (e.g. "notion", "zulip").
    fn name(&self) -> &str;

    /// Run an incremental sync. The cursor is opaque state from the
    /// previous run (None on first sync).
    async fn sync(&self, cursor: Option<&str>) -> Result<(SyncOutput, Option<String>)>;
}
