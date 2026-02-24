# Teidelum

A compact, local-first MCP server that syncs work tools and connects live data sources into a single searchable and queryable index. One binary. Zero config. Data never leaves the machine.

## The Problem

Modern teams scatter knowledge across Notion, Zulip, and internal data systems. AI agents need to search and reason over all of it. Today's options:

```
Per-app MCP servers          Elasticsearch-based indexers
┌──────┐ ┌───────┐          ┌──────────────────────────┐
│Notion│ │ Zulip │  ← API   │  .md files on disk       │
│ MCP  │ │  MCP  │  every   │         ▼                │
└──────┘ └───────┘  call     │  Elasticsearch (500MB+)  │
No cross-source queries      │  RAM-mapped, heap-bound  │
No offline                   └──────────────────────────┘
No analytics                 Heavy. Fragile. RAM ceiling.
```

Neither provides cross-source analytics. Neither is compact. Neither works offline.

## The Solution

```
┌──────────────────────────────────────────────────────────┐
│               Teidelum — Single Binary (<15MB)           │
│                                                          │
│  ┌────────────────────────────────────────────────────┐  │
│  │              MCP Protocol Layer                     │  │
│  │       search · sql · describe · sync               │  │
│  └──────────────────────┬─────────────────────────────┘  │
│                         │                                 │
│  ┌──────────────────────▼─────────────────────────────┐  │
│  │              Query Router                           │  │
│  │   text → FTS   analytics → SQL   remote → connector │  │
│  └───────┬──────────────┬──────────────────┬──────────┘  │
│          │              │                  │              │
│  ┌───────▼───────┐ ┌───▼──────────┐ ┌─────▼──────────┐  │
│  │  Full-Text    │ │  libteide    │ │  Connectors    │  │
│  │  Search       │ │  Columnar    │ │  (live query)  │  │
│  │               │ │  Engine      │ │                │  │
│  │  tantivy      │ │  SQL, joins  │ │  kdb+          │  │
│  │  BM25, fuzzy  │ │  aggregation │ │  (future: pg)  │  │
│  └───────────────┘ └──────────────┘ └────────────────┘  │
│                                                          │
│  ┌────────────────────────────────────────────────────┐  │
│  │              Metadata Catalog                       │  │
│  │  schemas · FK relationships · local vs remote       │  │
│  └────────────────────────────────────────────────────┘  │
│                                                          │
│  ┌────────────────────────────────────────────────────┐  │
│  │              Sync Scheduler (tokio)                 │  │
│  │                                                     │  │
│  │   ┌────────┐  ┌───────┐                            │  │
│  │   │ Notion │  │ Zulip │    on-demand + interval     │  │
│  │   └────────┘  └───────┘                            │  │
│  └────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────┘
```

Sync once. Query live. Works offline.

## Architecture

### Connectors vs Sync

Teidelum distinguishes two data access patterns:

```
Sync (import locally)                Connector (query live)
─────────────────────                ────────────────────────
Pull data from API                   Query external system
Transform and store in libteide      Return results directly
Enables offline search + analytics   No data duplication
Runs on schedule or on-demand        On-demand only

Used for:                            Used for:
  Notion (pages, tasks, mentions)      kdb+ (large live datasets)
  Zulip  (messages, streams)           Future: Postgres, APIs
```

Sync modules and connectors are server-managed — compiled into the binary, feature-flagged, running as tokio tasks. No external daemons.

```
cargo build --release --features notion,zulip,kdb
```

### Crate Structure

```
teidelum/
  Cargo.toml                (workspace)
  crates/
    server/                 tokio, MCP protocol, query router, binary
    search/                 tantivy wrapper, SearchEngine trait
    catalog/                metadata catalog, FK maps, local vs remote

    connector-core/         trait: query external source live
    connector-kdb/          kdb+ live query adapter

    sync-core/              trait: pull → transform → store locally
    sync-notion/            Notion incremental sync
    sync-zulip/             Zulip incremental sync
```

Dependency flow:

```
server
  ├── search           (tantivy FTS)
  ├── catalog          (metadata + FK relationships)
  ├── teide-rs         (external crate — libteide SQL engine)
  ├── connector-core
  ├── connector-kdb    (feature = "kdb")
  ├── sync-core
  ├── sync-notion      (feature = "notion")
  └── sync-zulip       (feature = "zulip")
```

### Query Router

The query router checks the metadata catalog to determine how to serve each query:

```
sql("SELECT * FROM zulip_messages WHERE ...")
  │
  ▼
catalog: zulip_messages → local (synced)
  │
  ▼
teide-rs executes against local columnar storage
```

```
sql("SELECT * FROM trades WHERE date = today()")
  │
  ▼
catalog: trades → remote (connector-kdb)
  │
  ▼
connector-kdb proxies query to live kdb+ server
```

### Sync Pipeline

Each sync module pulls data from its source and splits it into two streams:

```
Source API
    │
    ▼
Sync Module
    │
    ├── Structured fields ──→ Columnar tables (libteide)
    │   (author, status,       Enables SQL analytics
    │    dates, metadata)
    │
    └── Free-form content ──→ Full-text index (tantivy)
        (page body, messages)  Enables ranked search
```

Sync is incremental — only changed data is pulled on subsequent runs.

### Metadata Catalog

The catalog stores schema information and relationships between tables. No graph database — just a libteide table describing foreign keys:

```
_catalog_tables         name · source · storage · row_count
────────────────────────────────────────────────────────────
                        notion_pages · notion · local · 1420
                        zulip_messages · zulip · local · 8301
                        trades · kdb · remote · —

_catalog_relationships  from_table · from_col · to_table · to_col · relation
────────────────────────────────────────────────────────────────────────────
                        notion_tasks · page_id · notion_pages · id · belongs_to
                        notion_mentions · page_id · notion_pages · id · belongs_to
                        notion_links · to_page · notion_pages · id · references
                        zulip_messages · stream · zulip_streams · id · belongs_to
                        all_documents · source_id · notion_pages · id · unifies
```

The `describe` tool returns this catalog to the AI agent, so it knows what tables exist, how they relate, and whether they are local or remote.

## Data Model

### Notion (synced locally)

```
notion_pages     id · title · parent_id · created_by · created_at · updated_at · word_count
notion_tasks     id · page_id · text · is_checked · assignee_mention
notion_mentions  page_id · mention_type · target_name
notion_links     from_page · to_page

Page content ──→ full-text index
```

Notion databases map properties directly to columns. Freeform pages: metadata and typed blocks (tasks, mentions, links) become structured tables. Body text goes to the full-text index.

### Zulip (synced locally)

```
zulip_messages   id · stream · topic · sender · timestamp · type
zulip_streams    id · name · description · is_private
zulip_topics     stream_id · name · last_message_ts · message_count

Message content ──→ full-text index
```

Zulip's stream/topic hierarchy provides natural structure most chat tools lack.

### kdb+ (live connector)

```
Schema derived from remote table structure.
Columns mapped through connector-kdb at query time.
No local storage — queries proxied to live kdb+ server.
Pure analytics path, no full-text index needed.
```

### Cross-source

```
all_documents    id · source · source_id · title · content_preview · author · created_at
```

A unified view across all synced sources for cross-tool queries.

## MCP API

Four tools exposed to AI agents:

```
┌─────────────────────────────────────────────────────────────────────────┐
│  Tool: search                                                           │
│  "Full-text search across all connected sources"                        │
│                                                                         │
│  Input:                                                                 │
│    query       string    required    "authentication redesign"          │
│    sources     string[]  optional    ["notion", "zulip"]               │
│    limit       int       optional    10                                 │
│    date_from   string    optional    "2026-01-01"                      │
│    date_to     string    optional    "2026-02-28"                      │
│                                                                         │
│  Returns: ranked results with source, title, snippet, score             │
│  Engine: full-text index (BM25 scoring, fuzzy matching)                 │
├─────────────────────────────────────────────────────────────────────────┤
│  Tool: sql                                                              │
│  "Run analytical queries over structured data from all sources"         │
│                                                                         │
│  Input:                                                                 │
│    query       string    required    SQL statement                      │
│                                                                         │
│  Returns: rows and columns, typed (int, string, float, timestamp)       │
│  Engine: libteide (local tables), connector (remote tables)             │
│  Router: catalog determines local vs remote execution                   │
├─────────────────────────────────────────────────────────────────────────┤
│  Tool: describe                                                         │
│  "List available tables, schemas, and relationships"                    │
│                                                                         │
│  Input:                                                                 │
│    source      string    optional    filter by source                   │
│                                                                         │
│  Returns: table names, column names, column types, row counts,          │
│           storage type (local/remote), FK relationships                  │
├─────────────────────────────────────────────────────────────────────────┤
│  Tool: sync                                                             │
│  "Trigger incremental sync for connected sources"                       │
│                                                                         │
│  Input:                                                                 │
│    source      string    optional    sync single source or all          │
│                                                                         │
│  Returns: sync status, documents added/updated/deleted                  │
└─────────────────────────────────────────────────────────────────────────┘
```

### Query Flow: Text Search

```
User: "What did the team discuss about deployments this month?"
     │
     ▼
Agent calls: search("deployments", date_from="2026-02-01")
     │
     ▼
Full-text index returns ranked results:

  source  │ title                       │ score
  ────────┼─────────────────────────────┼──────
  zulip   │ #ops > deployment-feb-20    │ 0.94
  notion  │ Deployment Runbook v3       │ 0.87
  zulip   │ #eng > CI pipeline changes  │ 0.71
```

### Query Flow: SQL Analytics

```
User: "Which Zulip topics had the most activity this week?"
     │
     ▼
Agent calls: sql("
  SELECT topic, COUNT(*) as msgs
  FROM zulip_messages
  WHERE timestamp >= '2026-02-17'
  GROUP BY topic
  ORDER BY msgs DESC
  LIMIT 10
")
     │
     ▼
libteide returns (local execution):

  topic              │ msgs
  ───────────────────┼─────
  deployment-feb-20  │  47
  auth-redesign      │  31
  hiring-updates     │  18
```

### Query Flow: Live Connector

```
User: "Show today's trading volume by symbol"
     │
     ▼
Agent calls: sql("
  SELECT sym, SUM(size) as volume
  FROM trades
  WHERE date = 2026.02.24
  GROUP BY sym
  ORDER BY volume DESC
  LIMIT 10
")
     │
     ▼
catalog: trades → remote (connector-kdb)
     │
     ▼
connector-kdb proxies to live kdb+ server:

  sym   │ volume
  ──────┼────────────
  AAPL  │  12,450,000
  MSFT  │   8,230,000
  TSLA  │   6,890,000
```

### Query Flow: Cross-Source

```
User: "Which Notion specs have active Zulip discussions?"
     │
     ▼
Agent calls:
  1. search("spec", sources=["notion"])
  2. For each result → search(title, sources=["zulip"])
     │
     ▼
Cross-source insight:

  Notion spec              │ Zulip threads │ Last discussed
  ─────────────────────────┼───────────────┼──────────────
  Auth Redesign RFC        │ 12 threads    │ 2 days ago
  Rate Limiting Design     │ 3 threads     │ 2 weeks ago
  Migration Plan v2        │ 0 threads     │ never
```

### Query Flow: Hybrid (Search + Analyze)

```
User: "Find all Zulip messages about outages and show frequency by month"
     │
     ▼
Agent calls:
  1. search("outage incident downtime", sources=["zulip"])
  2. sql("
       SELECT DATE_TRUNC('month', timestamp) as month, COUNT(*) as incidents
       FROM zulip_messages
       WHERE id IN (... matched IDs from search ...)
       GROUP BY month ORDER BY month
     ")
     │
     ▼
  month      │ incidents
  ───────────┼──────────
  2025-11    │  3
  2025-12    │  7
  2026-01    │  2
  2026-02    │  1
```

## Why libteide

The analytical engine underneath is libteide — a pure C17 columnar dataframe engine with lazy fusion execution and a 7-pass query optimizer.

```
                    libteide vs alternatives

  Binary size       ██ <1MB
  Elasticsearch     ████████████████████████████████████████ ~500MB
  Meilisearch       ████████████████████ ~100MB
  DuckDB            ██████████ ~50MB

  RAM usage         On-disk columnar, not memory-bound
  Elasticsearch     Heap-bound, GB-scale RAM required

  Query speed       2.4-3.7x faster than DuckDB on H2O.ai benchmarks
                    SIMD vectorized, morsel-driven parallel execution
```

libteide handles the analytical queries that search engines cannot: aggregation, joins, window functions, cross-source correlation. All in-process, zero network overhead.

## Comparison

```
                    ES-based    Per-app     Cloud        Teidelum
                    indexer     MCPs        search
                    ─────────   ────────    ─────────    ────────
Binary size         ~500MB      N/A         N/A          <15MB
RAM bound           Yes         N/A         N/A          No
Cross-source        No          No          Limited      Yes
SQL analytics       Weak        No          No           Native
Live connectors     No          No          No           Yes
Offline             Yes         No          No           Yes
Privacy             Self-host   API calls   Cloud        Local
Setup               Complex     Per-app     Zero         Zero
```

## Roadmap

### v0.1 — Core

- Notion sync: pages, databases, tasks, mentions, links
- Zulip sync: streams, topics, messages
- kdb+ connector: live query proxy
- MCP server: search, sql, describe, sync tools
- Metadata catalog with FK relationships
- Incremental sync with cursor tracking

### v0.2 — Polish

- Background sync scheduler (interval-based)
- Cross-source unified queries
- Incremental index updates without full re-index
- Connector/sync plugin interface for future sources

### v1.0 — Full Integration

- Replace tantivy with libteide native full-text search
- Inverted index stored as libteide columnar tables
- BM25 scoring as native query function
- Single engine, zero external dependencies
- Binary target: <5MB
