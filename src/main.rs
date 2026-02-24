use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use tracing_subscriber::EnvFilter;

mod catalog;
mod connector;
mod mcp;
mod router;
mod search;
mod sync;

use catalog::Catalog;
use mcp::Teidelum;
use router::QueryRouter;
use search::SearchEngine;

#[tokio::main]
async fn main() -> Result<()> {
    // Tracing must go to stderr — stdout is the MCP stdio transport.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("teidelum starting");

    // TODO: load config, open data directory
    let catalog = Catalog::new();
    let search_engine = SearchEngine::open(std::path::Path::new("data/index"))?;
    let query_router = QueryRouter::new(catalog.clone());

    let server = Teidelum::new(catalog, search_engine, query_router);

    let service = server.serve(stdio()).await.inspect_err(|e| {
        tracing::error!("serving error: {:?}", e);
    })?;

    service.waiting().await?;
    Ok(())
}
