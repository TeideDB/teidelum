use std::path::{Path, PathBuf};

use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use tracing_subscriber::EnvFilter;

mod catalog;
mod connector;
mod demo;
mod mcp;
mod router;
mod search;
mod sync;

use catalog::{Catalog, ColumnInfo, StorageType, TableEntry};
use mcp::Teidelum;
use router::QueryRouter;
use search::SearchEngine;

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
        demo::generate(&data)?;
    }

    // Initialize search engine and index documents
    let search_engine = SearchEngine::open(&data.join("index"))?;
    index_documents(&search_engine, &data.join("docs"))?;

    // Initialize SQL engine and load splayed tables
    let query_router = QueryRouter::new()?;
    let mut catalog = Catalog::new();
    load_tables(&query_router, &mut catalog, &data.join("tables"))?;

    tracing::info!("teidelum ready — serving MCP over stdio");

    let server = Teidelum::new(catalog, search_engine, query_router);

    let service = server.serve(stdio()).await.inspect_err(|e| {
        tracing::error!("serving error: {:?}", e);
    })?;

    service.waiting().await?;
    Ok(())
}

fn index_documents(engine: &SearchEngine, docs_dir: &Path) -> Result<()> {
    if !docs_dir.exists() {
        return Ok(());
    }

    let mut documents = Vec::new();

    for entry in std::fs::read_dir(docs_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "md") {
            let content = std::fs::read_to_string(&path)?;
            let filename = path.file_stem().unwrap().to_string_lossy().to_string();

            // Extract title from first # heading, or use filename
            let title = content
                .lines()
                .find(|l| l.starts_with("# "))
                .map(|l| l.trim_start_matches("# ").to_string())
                .unwrap_or_else(|| filename.clone());

            // Infer source from content or default to "docs"
            let source = if content.contains("zulip") || filename.contains("zulip") || filename.contains("standup") || filename.contains("incident") {
                "zulip"
            } else {
                "notion"
            };

            documents.push((filename, source.to_string(), title, content));
        }
    }

    let count = engine.index_documents(
        &documents
            .iter()
            .map(|(id, src, title, body)| {
                (id.clone(), src.clone(), title.clone(), body.clone())
            })
            .collect::<Vec<_>>(),
    )?;

    tracing::info!("indexed {count} documents for full-text search");
    Ok(())
}

fn load_tables(router: &QueryRouter, catalog: &mut Catalog, tables_dir: &Path) -> Result<()> {
    if !tables_dir.exists() {
        return Ok(());
    }

    let sym_path = tables_dir.join("sym");
    let sym = if sym_path.exists() {
        Some(sym_path.as_path())
    } else {
        None
    };

    for entry in std::fs::read_dir(tables_dir)? {
        let entry = entry?;
        let path = entry.path();

        // Splayed tables are directories containing a .d file
        if path.is_dir() && path.join(".d").exists() {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            router.load_splayed(&name, &path, sym)?;

            // Query the table to get real column names and types
            if let Some((nrows, _ncols)) = router.table_info(&name) {
                let result = router.query_sync(&format!("SELECT * FROM {name} LIMIT 1"))?;
                let columns = result.columns.iter().map(|c| ColumnInfo {
                    name: c.name.clone(),
                    dtype: c.dtype.clone(),
                }).collect::<Vec<_>>();
                let ncols = columns.len();

                catalog.register_table(TableEntry {
                    name: name.clone(),
                    source: "demo".to_string(),
                    storage: StorageType::Local,
                    columns,
                    row_count: Some(nrows as u64),
                });

                tracing::info!("registered table: {name} ({nrows} rows, {ncols} cols)");
            }
        }
    }

    Ok(())
}
