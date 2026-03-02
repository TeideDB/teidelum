---
title: Quick Start
description: Get Teidelum running and connected to an AI agent in minutes
---

## Run the Server

Teidelum serves MCP over stdio. Start it:

```bash
./target/release/teidelum
```

On first run, it generates demo data (team members, project tasks, incidents, and sample documents) so you can explore immediately.

## Connect to Your AI Agent

Add Teidelum to your MCP client configuration. For Claude Desktop or Claude Code, add to your MCP settings:

```json
{
  "mcpServers": {
    "teidelum": {
      "command": "/path/to/teidelum"
    }
  }
}
```

Replace `/path/to/teidelum` with the absolute path to your built binary.

## Try Your First Queries

Once connected, your AI agent has access to five tools:

### Search for documents

Ask your agent: *"Search for authentication docs"*

This triggers the `search` tool, which performs full-text search with BM25 ranking across all indexed documents.

### Query structured data

Ask: *"Show me all team members"*

This triggers the `sql` tool:

```sql
SELECT * FROM team_members
```

### Explore relationships

Ask: *"Find all tasks assigned to Alice Chen"*

This triggers the `graph` tool, traversing FK relationships between `project_tasks` and `team_members`.

### See what's available

Ask: *"Describe the available data"*

This triggers the `describe` tool, returning all tables, their schemas, and registered relationships.
