---
title: Graph Traversal
description: Navigate relationships between entities using BFS
---

Teidelum's graph engine performs BFS traversal over foreign key relationships registered in the catalog. It supports neighbor discovery and shortest-path finding.

## Registering Relationships

Before graph traversal works, you must register FK relationships. Each relationship links a column in one table to a column in another:

```rust
Relationship {
    from_table: "project_tasks",
    from_col: "assignee",
    to_table: "team_members",
    to_col: "name",
    relation: "assigned_to",
}
```

This means: `project_tasks.assignee` references `team_members.name`, and the relationship is called `assigned_to`.

## Graph Operations

### Neighbors

Discover all entities reachable from a starting node up to a given depth.

Parameters:

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `table` | string | required | Starting node's table |
| `key` | string | required | Node identifier value |
| `key_col` | string | "name" | Column used to identify the node |
| `operation` | string | "neighbors" | Set to "neighbors" |
| `depth` | number | 2 | Max traversal hops (capped at 10) |
| `direction` | string | "both" | "forward", "reverse", or "both" |
| `rel_types` | string[] | all | Filter to specific relationship types |

Example — find everything connected to Alice Chen within 2 hops:

```json
{
  "table": "team_members",
  "key": "Alice Chen",
  "key_col": "name",
  "operation": "neighbors",
  "depth": 2,
  "direction": "both"
}
```

Response:

```json
{
  "nodes": [
    { "table": "team_members", "key": "Alice Chen", "properties": { "role": "Backend Engineer" } },
    { "table": "project_tasks", "key": "Implement JWT rotation", "properties": { "status": "in_progress" } }
  ],
  "edges": [
    {
      "from_table": "project_tasks", "from_key": "Implement JWT rotation",
      "to_table": "team_members", "to_key": "Alice Chen",
      "relation": "assigned_to"
    }
  ]
}
```

### Path

Find the shortest path between two specific nodes.

Additional parameters for path operations:

| Parameter | Type | Description |
|-----------|------|-------------|
| `to_table` | string | Target node's table (required) |
| `to_key` | string | Target node's identifier (required) |
| `to_key_col` | string | Target's key column (defaults to `key_col`) |

Example — find path from a task to a team member:

```json
{
  "table": "project_tasks",
  "key": "Implement JWT rotation",
  "key_col": "title",
  "operation": "path",
  "to_table": "team_members",
  "to_key": "Alice Chen",
  "to_key_col": "name",
  "depth": 5
}
```

Response:

```json
{
  "found": true,
  "path": [
    { "table": "project_tasks", "key": "Implement JWT rotation" },
    { "table": "team_members", "key": "Alice Chen" }
  ],
  "hops": 1
}
```

## Direction Filtering

- **"forward"**: Follow relationships in the defined direction (`from_table` → `to_table`)
- **"reverse"**: Follow relationships backwards (`to_table` → `from_table`)
- **"both"**: Follow relationships in either direction

## Depth Limits

Maximum traversal depth is capped at **10 hops** to prevent unbounded query storms. The `depth` parameter controls how many hops to traverse (default: 2).
