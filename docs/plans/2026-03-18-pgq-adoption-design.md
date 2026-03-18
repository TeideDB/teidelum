# PGQ Adoption Design

**Date:** 2026-03-18
**Goal:** Full adoption of teide-rs graph capabilities via PGQ SQL syntax, replacing the hand-rolled graph engine.

## Overview

teide-rs now ships a native graph engine with CSR-indexed relationships, SQL/PGQ syntax (`CREATE PROPERTY GRAPH`, `GRAPH_TABLE ... MATCH`), and graph algorithms (PageRank, Dijkstra, Louvain, connected components, clustering coefficient). Teidelum currently has a hand-rolled BFS graph engine (`graph.rs`, ~500 lines) that traverses catalog FK relationships by issuing SQL queries at each hop.

This design replaces the custom graph engine entirely with teide-rs's native PGQ support, exposed through the existing `sql` MCP tool. No new MCP tools are added — agents use PGQ SQL syntax for all graph operations.

## Approach: Top-Down, Three Phases

### Phase 1: PGQ Pass-Through

Verify that PGQ queries work through `session.execute()` and fix any result-reading gaps in `router.rs`.

**Likely gaps:**
- `col_type_name()` — may need new type tag mappings for graph result columns
- `CREATE PROPERTY GRAPH` — should return `ExecResult::Ddl(msg)`, already handled
- `GRAPH_TABLE` queries — should return `ExecResult::Query`, already handled

**Work:**
- Write integration tests exercising PGQ through `QueryRouter`
- Fix any type mapping gaps in `col_type_name()` / `read_value()`

### Phase 2: Auto-Generated Property Graphs

When relationships are registered (via `add_relationship` or on startup replay), automatically issue `CREATE PROPERTY GRAPH` so PGQ is immediately usable.

**Naming convention:** `pg_{from_table}_{to_table}_{relation}` — deterministic, discoverable.

**When graphs get created:**
- `add_relationship` — after registering FK in catalog, issue `CREATE PROPERTY GRAPH` through the router
- Startup — replay all persisted catalog relationships to recreate property graphs (teide sessions are in-memory)

**Graph shape per relationship:**
```sql
CREATE PROPERTY GRAPH pg_users_channels_member
  VERTEX TABLES (
    users LABEL User,
    channels LABEL Channel
  )
  EDGE TABLES (
    memberships
      SOURCE KEY (user_id) REFERENCES users (id)
      DESTINATION KEY (channel_id) REFERENCES channels (id)
      LABEL member
  )
```

One relationship = one property graph. Agents can create richer multi-edge graphs manually via `sql`.

**Edge case — FK on source table:** A relationship like `messages.channel_id -> channels.id` uses the `messages` table itself as the edge table. The FK column is on the source table, which maps naturally to PGQ where the source table doubles as the edge table.

**`describe` enhancement:** Add a "Property Graphs" section listing auto-generated graphs with their vertex/edge labels.

### Phase 3: Deprecate `graph` Tool

**Remove:**
- `src/graph.rs` (~500 lines) — `GraphEngine`, BFS traversal, neighbor/path-finding
- `src/mcp.rs` — `graph` tool definition and handler
- `src/api.rs` — `GraphEngine` field, `graph()` accessor, graph initialization
- `src/main.rs` — relationship registration into graph engine (keep catalog registration)
- Graph-specific unit tests (replaced by PGQ integration tests)

**What replaces it for agents:**

Instead of:
```
graph tool: neighbors of user 42 via "member" relationship
```

Agents use:
```sql
SELECT * FROM GRAPH_TABLE (pg_users_channels_member
  MATCH (u:User)-[:member]->(c:Channel)
  WHERE u.id = 42
  COLUMNS (c.id, c.name))
```

More powerful (variable-length paths, algorithms, filtering) at the cost of requiring PGQ syntax — but LLMs handle that well.

## Testing Strategy

**Phase 1 — PGQ pass-through tests:**
- Create tables + insert data via `QueryRouter`
- `CREATE PROPERTY GRAPH` returns success
- `GRAPH_TABLE ... MATCH` 1-hop and variable-length patterns return correct rows
- Algorithm functions (PAGERANK, LOUVAIN, CONNECTED_COMPONENT, CLUSTERING_COEFFICIENT) return expected columns and plausible values
- Verify `col_type_name` handles all returned types

**Phase 2 — Auto-generation tests:**
- `add_relationship` -> property graph is immediately queryable
- Restart (reconstruct `TeidelumApi`) -> graphs recreated from persisted catalog
- Edge case: relationship where source table is the edge table
- `describe` output includes property graphs section

**Phase 3 — Removal verification:**
- Existing integration tests pass after `graph.rs` deletion
- `describe` no longer mentions the `graph` tool
- PGQ tests cover all former `graph` tool use cases (neighbors, paths)

## What Changes

| Component | Change |
|-----------|--------|
| `router.rs` | Possibly new type mappings in `col_type_name()` |
| `api.rs` | Add auto-graph creation in `add_relationship`; remove `GraphEngine` (Phase 3) |
| `mcp.rs` | Remove `graph` tool (Phase 3) |
| `graph.rs` | Delete (Phase 3) |
| `main.rs` | Remove graph engine init (Phase 3) |
| `catalog.rs` | No changes — still manages FK relationships |
| `describe` output | Add property graphs section |

## What Stays the Same

- `add_relationship` MCP tool (still registers FKs, now also creates property graphs)
- `sql` MCP tool interface (PGQ is just SQL)
- `search` MCP tool
- All sync modules
- All chat functionality
