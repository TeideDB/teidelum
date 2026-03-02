---
title: Agent Workflows
description: End-to-end examples of AI agents using Teidelum
---

These examples show how an AI agent chains Teidelum's tools to answer complex questions.

## Find who's working on what

**User asks:** "Who is working on high-priority tasks?"

**Agent workflow:**

1. Query for high-priority tasks:
   ```json
   { "tool": "sql", "query": "SELECT title, assignee, status FROM project_tasks WHERE priority = 'high'" }
   ```

2. For each assignee, get their details:
   ```json
   { "tool": "sql", "query": "SELECT name, role, department FROM team_members WHERE name = 'Alice Chen'" }
   ```

3. Agent synthesizes: "Alice Chen (Backend Engineer) is working on 'Implement JWT rotation' (in progress) and Bob Smith (SRE) is handling 'Fix connection pool leak' (open)."

## Research a topic across all sources

**User asks:** "What do we know about rate limiting?"

**Agent workflow:**

1. Search for documents:
   ```json
   { "tool": "search", "query": "rate limiting", "limit": 5 }
   ```

2. Get related structured data:
   ```json
   { "tool": "sql", "query": "SELECT title, status FROM project_tasks WHERE title LIKE '%rate limit%'" }
   ```

3. Agent combines document content with task status for a complete picture.

## Trace an incident to its owner

**User asks:** "Who reported the API timeout incident and what else are they working on?"

**Agent workflow:**

1. Find the incident:
   ```json
   { "tool": "sql", "query": "SELECT description, reporter, severity FROM incidents WHERE description LIKE '%API timeout%'" }
   ```

2. Traverse the graph to find related entities:
   ```json
   {
     "tool": "graph",
     "table": "incidents",
     "key": "API gateway timeout affecting 5% of requests",
     "key_col": "description",
     "operation": "neighbors",
     "depth": 2,
     "direction": "both"
   }
   ```

3. Agent follows the graph from incident → reporter → their other tasks and incidents.

## Discover the data landscape

**User asks:** "What data do we have?"

**Agent workflow:**

1. Describe the catalog:
   ```json
   { "tool": "describe" }
   ```

2. Agent reads the table schemas and relationships, then explains what's available in natural language.
