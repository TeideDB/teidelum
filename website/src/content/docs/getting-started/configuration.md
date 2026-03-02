---
title: Configuration
description: Configure Teidelum's data directory and logging
---

## Data Directory

Teidelum stores all data in a single directory. Set it via the `TEIDELUM_DATA` environment variable:

```bash
export TEIDELUM_DATA=/path/to/data
./target/release/teidelum
```

Default: `./data/` (relative to the working directory).

### Directory Layout

```
data/
├── tables/           # Splayed columnar tables (teide format)
│   ├── sym           # Shared symbol file
│   ├── team_members/ # One directory per table
│   │   ├── .d        # Table marker
│   │   ├── name      # Column files
│   │   └── role
│   └── project_tasks/
├── docs/             # Markdown documents (indexed for search)
│   ├── auth-rfc.md
│   └── deployment-runbook.md
└── index/            # Tantivy full-text search index
```

## Logging

Teidelum uses the `RUST_LOG` environment variable for log level control:

```bash
RUST_LOG=info ./target/release/teidelum    # default
RUST_LOG=debug ./target/release/teidelum   # verbose
RUST_LOG=warn ./target/release/teidelum    # quiet
```

Logs go to **stderr** (stdout is reserved for the MCP stdio transport).

## Demo Data

On first run, if the `tables/` or `docs/` directories don't exist, Teidelum generates demo data:

- **team_members** — names, roles, departments
- **project_tasks** — titles, statuses, priorities, assignees
- **incidents** — descriptions, severities, reporters
- **Sample documents** — markdown files covering auth, deployment, rate limiting, etc.

To start fresh, delete the data directory:

```bash
rm -rf data/
./target/release/teidelum
```
