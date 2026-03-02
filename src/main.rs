use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use rmcp::{transport::stdio, ServiceExt};
use tracing_subscriber::EnvFilter;

use teidelum::api::TeidelumApi;
use teidelum::catalog::Relationship;
use teidelum::mcp::Teidelum;

#[derive(Parser)]
#[command(name = "teidelum", about = "Local-first MCP server with REST API")]
struct Cli {
    /// Enable HTTP server on this port
    #[arg(long)]
    port: Option<u16>,

    /// Bind address for the HTTP server
    #[arg(long, default_value = "127.0.0.1")]
    bind: String,

    /// Data directory
    #[arg(long, env = "TEIDELUM_DATA", default_value = "data")]
    data: PathBuf,
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

    let cli = Cli::parse();

    // Generate demo data if not present
    if !cli.data.join("tables").exists() || !cli.data.join("docs").exists() {
        tracing::info!("generating demo data...");
        teidelum::demo::generate(&cli.data)?;
    }

    let api = TeidelumApi::open(&cli.data)?;

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

    if let Some(port) = cli.port {
        tracing::info!("starting HTTP server on {}:{}", cli.bind, port);
        // TODO: start HTTP server (Task 10)
        todo!("HTTP server not yet implemented");
    } else {
        tracing::info!("teidelum ready — serving MCP over stdio");
        let server = Teidelum::new(api);
        let service = server.serve(stdio()).await.inspect_err(|e| {
            tracing::error!("serving error: {:?}", e);
        })?;
        service.waiting().await?;
    }

    Ok(())
}
