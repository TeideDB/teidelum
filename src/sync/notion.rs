use anyhow::Result;
use async_trait::async_trait;

use super::{SyncOutput, SyncSource};

#[derive(Default)]
pub struct NotionSync {
    // TODO: API token, workspace config
}

impl NotionSync {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SyncSource for NotionSync {
    fn name(&self) -> &str {
        "notion"
    }

    async fn sync(&self, _cursor: Option<&str>) -> Result<(SyncOutput, Option<String>)> {
        // TODO: call Notion API, extract pages/tasks/mentions/links,
        //       split into structured records + search documents
        Ok((SyncOutput::default(), None))
    }
}
