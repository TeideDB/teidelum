use anyhow::Result;
use async_trait::async_trait;
use teidelum_sync_core::{SyncOutput, SyncSource};

pub struct ZulipSync {
    // TODO: API config (server URL, credentials)
}

impl ZulipSync {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl SyncSource for ZulipSync {
    fn name(&self) -> &str {
        "zulip"
    }

    async fn sync(&self, _cursor: Option<&str>) -> Result<(SyncOutput, Option<String>)> {
        // TODO: call Zulip API, extract messages/streams/topics,
        //       split into structured records + search documents
        Ok((SyncOutput::default(), None))
    }
}
