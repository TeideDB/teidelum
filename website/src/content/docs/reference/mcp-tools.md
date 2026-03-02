---
title: MCP Tools
description: Complete reference for all five MCP tools
---

Teidelum exposes five tools via the Model Context Protocol. AI agents call these tools to search, query, explore, and sync data.

## search

**Description:** Full-text search across all connected sources.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `query` | string | yes | — | Search query text |
| `sources` | string[] | no | all | Filter to specific sources |
| `limit` | number | no | 10 | Max results to return |
| `date_from` | string | no | — | ISO 8601 date lower bound |
| `date_to` | string | no | — | ISO 8601 date upper bound |

**Returns:** Array of search results with `id`, `source`, `title`, `snippet` (HTML), and `score`.

**Example:**

```json
{
  "query": "authentication JWT",
  "sources": ["notion"],
  "limit": 5
}
```

---

## sql

**Description:** Run analytical queries over structured data from all sources.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | yes | SQL query to execute |

**Returns:** `QueryResult` with `columns` (name + dtype) and `rows` (array of arrays).

**Example:**

```json
{
  "query": "SELECT name, role FROM team_members WHERE department = 'Engineering'"
}
```

---

## describe

**Description:** List available tables, schemas, and relationships.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `source` | string | no | all | Filter to a specific source |

**Returns:** JSON with `tables` array and `relationships` array.

**Example:**

```json
{
  "source": "demo"
}
```

---

## graph

**Description:** Traverse relationships between entities (neighbors, paths).

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `table` | string | yes | — | Starting node's table |
| `key` | string | yes | — | Node identifier value |
| `key_col` | string | no | "name" | Key column name |
| `operation` | string | no | "neighbors" | "neighbors" or "path" |
| `depth` | number | no | 2 | Max traversal hops (max 10) |
| `direction` | string | no | "both" | "forward", "reverse", or "both" |
| `rel_types` | string[] | no | all | Filter relationship types |
| `to_table` | string | path only | — | Target table (path operation) |
| `to_key` | string | path only | — | Target key (path operation) |
| `to_key_col` | string | no | key_col | Target key column (path operation) |

**Returns (neighbors):** `{ nodes: [...], edges: [...] }`

**Returns (path):** `{ found: bool, path: [...], hops: number }`

**Example (neighbors):**

```json
{
  "table": "team_members",
  "key": "Alice Chen",
  "operation": "neighbors",
  "depth": 2,
  "direction": "both"
}
```

**Example (path):**

```json
{
  "table": "project_tasks",
  "key": "Implement JWT rotation",
  "key_col": "title",
  "operation": "path",
  "to_table": "team_members",
  "to_key": "Alice Chen",
  "depth": 5
}
```

---

## sync

**Description:** Trigger incremental sync for connected sources.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `source` | string | no | all | Sync a specific source |

**Returns:** Sync status with counts of added/updated/deleted records.

**Example:**

```json
{
  "source": "notion"
}
```

:::note
Sync sources are not yet implemented in the current release. The tool returns a placeholder response.
:::
