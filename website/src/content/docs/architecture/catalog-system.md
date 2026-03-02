---
title: Catalog System
description: How Teidelum tracks tables, schemas, and relationships
---

The catalog is the metadata registry at the heart of Teidelum. It tracks what data is available, where it lives, and how it's related.

## TableEntry

Each registered table has a `TableEntry`:

```rust
pub struct TableEntry {
    pub name: String,           // Table name (valid SQL identifier)
    pub source: String,         // Origin (e.g., "notion", "demo")
    pub storage: StorageType,   // Local or Remote
    pub columns: Vec<ColumnInfo>,
    pub row_count: Option<u64>,
}
```

### StorageType

- **Local**: Data is stored in teide's columnar format on disk. SQL queries execute locally.
- **Remote**: Data lives in an external system. SQL queries are dispatched to connectors.

## Relationships

FK relationships link columns across tables:

```rust
pub struct Relationship {
    pub from_table: String,
    pub from_col: String,
    pub to_table: String,
    pub to_col: String,
    pub relation: String,  // Label (e.g., "assigned_to")
}
```

Relationships can be registered before the referenced tables exist. The graph engine rebuilds its topology whenever the catalog changes.

## Identifier Validation

All table names, column names, and relationship fields are validated as safe SQL identifiers: must start with a letter or underscore, followed by alphanumeric characters or underscores. This prevents SQL injection in dynamically constructed queries.

## The `describe` Tool

The catalog powers the `describe` MCP tool, which returns all tables and relationships as JSON. It supports optional source filtering:

```json
{
  "tables": [
    {
      "name": "team_members",
      "source": "demo",
      "storage": "local",
      "columns": [
        { "name": "name", "dtype": "string" },
        { "name": "role", "dtype": "string" }
      ],
      "row_count": 10
    }
  ],
  "relationships": [
    {
      "from_table": "project_tasks",
      "from_col": "assignee",
      "to_table": "team_members",
      "to_col": "name",
      "relation": "assigned_to"
    }
  ]
}
```
