use anyhow::Result;
use tracing_subscriber::EnvFilter;

mod router;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tracing::info!("teidelum starting");

    // TODO: initialize catalog, search engine, connectors, sync sources
    // TODO: start MCP protocol server (stdio or HTTP)

    Ok(())
}
