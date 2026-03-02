---
title: Configuration Reference
description: All environment variables, defaults, and directory structure
---

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `TEIDELUM_DATA` | `./data/` | Path to the data directory |
| `RUST_LOG` | `info` | Log level (`error`, `warn`, `info`, `debug`, `trace`) |

## Data Directory Structure

```
{TEIDELUM_DATA}/
├── tables/              # Splayed columnar tables
│   ├── sym              # Shared symbol file for enumerated strings
│   └── {table_name}/    # One directory per table
│       ├── .d           # Table marker file
│       └── {column}     # One file per column (binary format)
├── docs/                # Markdown documents for full-text indexing
│   └── *.md             # Each .md file is indexed
└── index/               # Tantivy search index
    └── (tantivy files)  # Managed by tantivy, do not modify
```

## Table Directory Format

Each table is a directory containing:

- **`.d`** — marker file indicating this is a splayed table
- **Column files** — one binary file per column, named after the column

The `sym` file in the `tables/` root is a shared symbol table for enumerated string columns (categorical data).

## Logging Output

All logs go to **stderr**. Stdout is reserved for the MCP stdio transport. This means you can pipe Teidelum's output to an MCP client while still seeing logs:

```bash
RUST_LOG=debug ./teidelum 2>teidelum.log
```

## Defaults

| Setting | Default Value |
|---------|---------------|
| Data directory | `./data/` |
| Search result limit | 10 |
| Graph traversal depth | 2 |
| Max graph depth | 10 |
| Key column for graph | `"name"` |
| Graph direction | `"both"` |
| Graph operation | `"neighbors"` |
| Index writer heap | 50 MB |
| Insert batch size | 1,000 rows |
