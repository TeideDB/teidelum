---
title: Data Modeling
description: Design tables and relationships for effective graph queries
---

Good data modeling makes Teidelum's graph traversal and SQL queries more effective.

## Table Design

### Use string keys for graph traversal

The graph engine identifies nodes by a key column value. Use human-readable string keys:

```sql
-- Good: human-readable key
CREATE TABLE team_members (name VARCHAR, role VARCHAR, department VARCHAR)

-- Less useful for graph: numeric IDs require looking up the value
CREATE TABLE team_members (id BIGINT, name VARCHAR, role VARCHAR)
```

Both work, but string keys make graph results more readable and useful for AI agents.

### Keep tables focused

One table per entity type. Don't combine team members and tasks into a single table:

```
team_members: name, role, department
project_tasks: title, status, priority, assignee
incidents: description, severity, reporter
```

### Use consistent naming

- Table names: `snake_case`, plural (`team_members`, `project_tasks`)
- Column names: `snake_case` (`first_name`, `created_at`)
- FK columns: name should hint at the relationship (`assignee`, `reporter`)

## Relationship Design

### Model real-world connections

Each relationship should represent a meaningful real-world connection:

```rust
// Task → Person: who is responsible
Relationship {
    from_table: "project_tasks",
    from_col: "assignee",
    to_table: "team_members",
    to_col: "name",
    relation: "assigned_to",
}

// Incident → Person: who reported it
Relationship {
    from_table: "incidents",
    from_col: "reporter",
    to_table: "team_members",
    to_col: "name",
    relation: "reported_by",
}
```

### Relationship naming

The `relation` label should describe the edge in the **forward direction** (from → to):

- `assigned_to` (task → person)
- `reported_by` (incident → person)
- `belongs_to` (item → category)
- `depends_on` (task → task)

### Multiple relationships between tables

You can have multiple relationships between the same pair of tables:

```rust
// Tasks have both an assignee and a reviewer
Relationship { from_table: "tasks", from_col: "assignee", to_table: "people", to_col: "name", relation: "assigned_to" }
Relationship { from_table: "tasks", from_col: "reviewer", to_table: "people", to_col: "name", relation: "reviewed_by" }
```

Use `rel_types` filtering in graph queries to follow only specific relationship types.

## Graph Traversal Patterns

### Hub entities

Entities connected to many others (like team members) act as hubs. Querying neighbors of a hub with high depth returns a large result set. Use `depth: 1` for hubs.

### Chain traversal

For indirect relationships (task → assignee → other tasks), use `depth: 2` with directional filtering:

```json
{
  "table": "project_tasks",
  "key": "Implement JWT rotation",
  "key_col": "title",
  "depth": 2,
  "direction": "both"
}
```

This finds: task → assignee → other tasks assigned to the same person.
