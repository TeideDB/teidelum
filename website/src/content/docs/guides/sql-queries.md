---
title: SQL Queries
description: Run analytical queries over structured data
---

Teidelum routes SQL queries to its columnar engine (teide) for local tables. Tables are stored in a splayed columnar format optimized for analytical queries.

## Supported Data Types

| Type | SQL Type | Description |
|------|----------|-------------|
| `bool` | BOOLEAN | True/false |
| `i32` | BIGINT | 32-bit integer (stored as BIGINT) |
| `i64` | BIGINT | 64-bit integer |
| `f64` | DOUBLE | 64-bit floating point |
| `string` | VARCHAR | Variable-length text |
| `date` | DATE | Calendar date |
| `time` | TIME | Time of day |
| `timestamp` | TIMESTAMP | Date and time |

## Query Examples

### List all tables

Use the `describe` tool to see available tables and their schemas.

### Basic SELECT

```sql
SELECT name, role FROM team_members
```

### Filtering

```sql
SELECT title, status FROM project_tasks WHERE priority = 'high'
```

### Aggregation

```sql
SELECT status, count(*) as cnt
FROM project_tasks
GROUP BY status
```

### Ordering and limits

```sql
SELECT title, priority
FROM project_tasks
ORDER BY priority
LIMIT 10
```

## Query Results

Results are returned as JSON with column schemas and rows:

```json
{
  "columns": [
    { "name": "name", "dtype": "string" },
    { "name": "role", "dtype": "string" }
  ],
  "rows": [
    ["Alice Chen", "Backend Engineer"],
    ["Bob Smith", "Frontend Engineer"]
  ]
}
```

## Error Handling

Invalid SQL returns an error message. Common issues:

- **Table not found**: Check available tables with `describe`
- **Column not found**: Verify column names in the table schema
- **Syntax error**: Standard SQL syntax is expected
