use std::path::PathBuf;

use anyhow::Result;
use rmcp::{transport::stdio, ServiceExt};
use tracing_subscriber::EnvFilter;

use teidelum::api::TeidelumApi;
use teidelum::catalog::Relationship;
use teidelum::mcp::Teidelum;

fn data_dir() -> PathBuf {
    std::env::var("TEIDELUM_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("data"))
}

#[tokio::main]
async fn main() -> Result<()> {
    // Tracing must go to stderr — stdout is the MCP stdio transport.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let data = data_dir();

    // Generate demo data if not present
    if !data.join("tables").exists() || !data.join("docs").exists() {
        tracing::info!("generating demo data...");
        teidelum::demo::generate(&data)?;
    }

    let api = TeidelumApi::open(&data)?;

    // Register FK relationships between demo tables
    api.register_relationships(vec![
        Relationship {
            from_table: "project_tasks".to_string(),
            from_col: "assignee".to_string(),
            to_table: "team_members".to_string(),
            to_col: "name".to_string(),
            relation: "assigned_to".to_string(),
        },
        Relationship {
            from_table: "incidents".to_string(),
            from_col: "reporter".to_string(),
            to_table: "team_members".to_string(),
            to_col: "name".to_string(),
            relation: "reported_by".to_string(),
        },
    ])?;

    tracing::info!("teidelum ready — serving MCP over stdio");

    let server = Teidelum::new(api);

    let service = server.serve(stdio()).await.inspect_err(|e| {
        tracing::error!("serving error: {:?}", e);
    })?;

    service.waiting().await?;
    Ok(())
}
