use std::path::PathBuf;
use std::sync::Arc;

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

    // Initialize chat tables
    teidelum::chat::models::init_chat_tables(&api)?;
    tracing::info!("chat tables initialized");

    if let Some(port) = cli.port {
        // HTTP mode: run REST API + stdio MCP in parallel
        let api = Arc::new(api);

        let server = Teidelum::new_with_shared(api.clone());

        let http_handle = tokio::spawn({
            let api = api.clone();
            let bind = cli.bind.clone();
            async move { teidelum::server::start(api, &bind, port).await }
        });

        tracing::info!(
            "teidelum ready — serving MCP over stdio + HTTP on {}:{}",
            cli.bind,
            port
        );

        let mcp_handle = tokio::spawn(async move {
            let service = server.serve(stdio()).await.inspect_err(|e| {
                tracing::error!("MCP serving error: {:?}", e);
            })?;
            service.waiting().await?;
            Ok::<_, anyhow::Error>(())
        });

        tokio::select! {
            r = http_handle => r??,
            r = mcp_handle => r??,
        }
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
