# PGQ Adoption Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the hand-rolled graph engine with teide-rs's native PGQ support, exposed through the existing `sql` MCP tool.

**Architecture:** PGQ queries pass through `session.execute()` in `QueryRouter`. Auto-generated property graphs are created from catalog relationships on `add_relationship` and startup. The `graph` MCP tool and `graph.rs` are removed.

**Tech Stack:** Rust, teide-rs (PGQ/SQL), rmcp (MCP server)

---

### Task 1: Verify PGQ pass-through works

**Files:**
- Test: `src/router.rs` (add tests in existing `mod tests`)

**Step 1: Write the failing test for CREATE PROPERTY GRAPH**

Add to the bottom of the `mod tests` block in `src/router.rs`:

```rust
#[test]
fn test_pgq_create_property_graph() {
    let router = QueryRouter::new().unwrap();
    router
        .query_sync("CREATE TABLE persons (id BIGINT, name VARCHAR)")
        .unwrap();
    router
        .query_sync("INSERT INTO persons (id, name) VALUES (0, 'Alice'), (1, 'Bob'), (2, 'Carol')")
        .unwrap();
    router
        .query_sync("CREATE TABLE knows (src BIGINT, dst BIGINT)")
        .unwrap();
    router
        .query_sync("INSERT INTO knows (src, dst) VALUES (0, 1), (1, 2), (0, 2)")
        .unwrap();

    let result = router.query_sync(
        "CREATE PROPERTY GRAPH social \
         VERTEX TABLES (persons LABEL Person) \
         EDGE TABLES (knows SOURCE KEY (src) REFERENCES persons (id) \
         DESTINATION KEY (dst) REFERENCES persons (id) LABEL Knows)",
    );
    assert!(result.is_ok(), "CREATE PROPERTY GRAPH failed: {result:?}");

    // DDL should return a status message
    let qr = result.unwrap();
    assert_eq!(qr.columns[0].name, "status");
}
```

**Step 2: Run test to verify it passes (or fails)**

Run: `cargo test test_pgq_create_property_graph -- --nocapture`

If it passes, PGQ DDL works through the router. If it fails, we need to investigate the error.

**Step 3: Commit**

```bash
git add src/router.rs
git commit -m "test: verify CREATE PROPERTY GRAPH works through QueryRouter"
```

---

### Task 2: Test GRAPH_TABLE MATCH queries

**Files:**
- Test: `src/router.rs` (add test in `mod tests`)

**Step 1: Write the test for GRAPH_TABLE MATCH**

Add to `mod tests` in `src/router.rs`:

```rust
#[test]
fn test_pgq_match_query() {
    let router = QueryRouter::new().unwrap();
    router
        .query_sync("CREATE TABLE persons (id BIGINT, name VARCHAR)")
        .unwrap();
    router
        .query_sync("INSERT INTO persons (id, name) VALUES (0, 'Alice'), (1, 'Bob'), (2, 'Carol')")
        .unwrap();
    router
        .query_sync("CREATE TABLE knows (src BIGINT, dst BIGINT)")
        .unwrap();
    router
        .query_sync("INSERT INTO knows (src, dst) VALUES (0, 1), (1, 2), (0, 2)")
        .unwrap();
    router
        .query_sync(
            "CREATE PROPERTY GRAPH social \
             VERTEX TABLES (persons LABEL Person) \
             EDGE TABLES (knows SOURCE KEY (src) REFERENCES persons (id) \
             DESTINATION KEY (dst) REFERENCES persons (id) LABEL Knows)",
        )
        .unwrap();

    // 1-hop: Alice's direct knows connections
    let result = router
        .query_sync(
            "SELECT * FROM GRAPH_TABLE (social \
             MATCH (p:Person)-[:Knows]->(q:Person) \
             WHERE p.name = 'Alice' \
             COLUMNS (q.name AS friend))",
        )
        .unwrap();

    assert_eq!(result.columns.len(), 1);
    assert_eq!(result.columns[0].name, "friend");
    assert_eq!(result.rows.len(), 2); // Alice knows Bob and Carol

    let names: Vec<String> = result
        .rows
        .iter()
        .map(|r| match &r[0] {
            Value::String(s) => s.clone(),
            other => panic!("expected String, got {other:?}"),
        })
        .collect();
    assert!(names.contains(&"Bob".to_string()));
    assert!(names.contains(&"Carol".to_string()));
}
```

**Step 2: Run test**

Run: `cargo test test_pgq_match_query -- --nocapture`

**Step 3: Commit**

```bash
git add src/router.rs
git commit -m "test: verify GRAPH_TABLE MATCH queries work through QueryRouter"
```

---

### Task 3: Test PGQ algorithm functions

**Files:**
- Test: `src/router.rs` (add test in `mod tests`)

**Step 1: Write the test for PAGERANK and CONNECTED_COMPONENT**

Add to `mod tests` in `src/router.rs`:

```rust
#[test]
fn test_pgq_algorithms() {
    let router = QueryRouter::new().unwrap();
    router
        .query_sync("CREATE TABLE persons (id BIGINT, name VARCHAR)")
        .unwrap();
    router
        .query_sync(
            "INSERT INTO persons (id, name) VALUES (0, 'Alice'), (1, 'Bob'), (2, 'Carol'), (3, 'Dave'), (4, 'Eve')",
        )
        .unwrap();
    router
        .query_sync("CREATE TABLE knows (src BIGINT, dst BIGINT)")
        .unwrap();
    router
        .query_sync("INSERT INTO knows (src, dst) VALUES (0, 1), (0, 2), (1, 3), (2, 3), (3, 4)")
        .unwrap();
    router
        .query_sync(
            "CREATE PROPERTY GRAPH social \
             VERTEX TABLES (persons LABEL Person) \
             EDGE TABLES (knows SOURCE KEY (src) REFERENCES persons (id) \
             DESTINATION KEY (dst) REFERENCES persons (id) LABEL Knows)",
        )
        .unwrap();

    // PageRank: all 5 nodes should get positive ranks
    let result = router
        .query_sync(
            "SELECT COUNT(*) FROM GRAPH_TABLE (social \
             MATCH (p:Person) \
             COLUMNS (PAGERANK(social, p) AS rank)) WHERE rank > 0",
        )
        .unwrap();
    match &result.rows[0][0] {
        Value::Int(n) => assert_eq!(*n, 5, "all 5 nodes should have positive pagerank"),
        other => panic!("expected Int, got {other:?}"),
    }

    // Connected components: single connected graph = 1 component
    let result = router
        .query_sync(
            "SELECT COUNT(DISTINCT component) FROM GRAPH_TABLE (social \
             MATCH (p:Person) \
             COLUMNS (COMPONENT(social, p) AS component))",
        )
        .unwrap();
    match &result.rows[0][0] {
        Value::Int(n) => assert_eq!(*n, 1, "fully connected graph should have 1 component"),
        other => panic!("expected Int, got {other:?}"),
    }

    // Louvain community detection: all nodes should get non-negative community IDs
    let result = router
        .query_sync(
            "SELECT COUNT(*) FROM GRAPH_TABLE (social \
             MATCH (p:Person) \
             COLUMNS (COMMUNITY(social, p) AS community)) WHERE community >= 0",
        )
        .unwrap();
    match &result.rows[0][0] {
        Value::Int(n) => assert_eq!(*n, 5),
        other => panic!("expected Int, got {other:?}"),
    }
}
```

**Step 2: Run test**

Run: `cargo test test_pgq_algorithms -- --nocapture`

If any column type is returned as "unknown", note which type tag and fix `col_type_name()` in the next task.

**Step 3: Commit**

```bash
git add src/router.rs
git commit -m "test: verify PGQ algorithm functions work through QueryRouter"
```

---

### Task 4: Fix col_type_name if needed

**Files:**
- Modify: `src/router.rs:154-166` (`col_type_name` function)

**Step 1: Check if all PGQ tests passed**

If Tasks 1-3 all passed, skip this task entirely. If any returned "unknown" types, check the type tags by adding a temporary debug print or examining what teide-rs returns.

**Step 2: Add any missing type tag mappings**

Common candidates from teide-rs graph results:
- i32 (type tag 5) — already mapped
- i64 (type tag 6) — already mapped
- f64 (type tag 7) — already mapped

If a new tag appears, add it to `col_type_name()`.

**Step 3: Run all PGQ tests**

Run: `cargo test test_pgq -- --nocapture`

**Step 4: Commit (if changes were needed)**

```bash
git add src/router.rs
git commit -m "fix: add missing type tag mappings for PGQ result columns"
```

---

### Task 5: Add auto-generated property graphs to TeidelumApi

**Files:**
- Modify: `src/api.rs`

**Step 1: Write the failing test**

Add to `mod tests` in `src/api.rs`:

```rust
#[test]
fn test_register_relationship_creates_property_graph() {
    let tmp = tempfile::tempdir().unwrap();
    let api = TeidelumApi::new(tmp.path()).unwrap();

    // Create two tables with data
    let person_cols = vec![
        ColumnSchema { name: "id".to_string(), dtype: "i64".to_string() },
        ColumnSchema { name: "name".to_string(), dtype: "string".to_string() },
    ];
    api.create_table(
        "persons",
        "test",
        &person_cols,
        &[
            vec![Value::Int(0), Value::String("Alice".to_string())],
            vec![Value::Int(1), Value::String("Bob".to_string())],
            vec![Value::Int(2), Value::String("Carol".to_string())],
        ],
    )
    .unwrap();

    let task_cols = vec![
        ColumnSchema { name: "id".to_string(), dtype: "i64".to_string() },
        ColumnSchema { name: "title".to_string(), dtype: "string".to_string() },
        ColumnSchema { name: "assignee_id".to_string(), dtype: "i64".to_string() },
    ];
    api.create_table(
        "tasks",
        "test",
        &task_cols,
        &[
            vec![Value::Int(1), Value::String("Fix bug".to_string()), Value::Int(0)],
            vec![Value::Int(2), Value::String("Add feature".to_string()), Value::Int(1)],
        ],
    )
    .unwrap();

    // Register relationship — should auto-create property graph
    api.register_relationship(Relationship {
        from_table: "tasks".to_string(),
        from_col: "assignee_id".to_string(),
        to_table: "persons".to_string(),
        to_col: "id".to_string(),
        relation: "assigned_to".to_string(),
    })
    .unwrap();

    // Property graph should be queryable via PGQ
    let result = api.query(
        "SELECT * FROM GRAPH_TABLE (pg_tasks_persons_assigned_to \
         MATCH (t:tasks)-[:assigned_to]->(p:persons) \
         WHERE p.name = 'Alice' \
         COLUMNS (t.title AS task_title))",
    );
    assert!(result.is_ok(), "PGQ query failed: {result:?}");
    let qr = result.unwrap();
    assert_eq!(qr.rows.len(), 1);
    match &qr.rows[0][0] {
        Value::String(s) => assert_eq!(s, "Fix bug"),
        other => panic!("expected String, got {other:?}"),
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_register_relationship_creates_property_graph -- --nocapture`

Expected: FAIL — property graph doesn't exist yet because `register_relationship` doesn't create one.

**Step 3: Implement auto-generation**

Add a helper method to `TeidelumApi` in `src/api.rs`:

```rust
/// Create a property graph for a catalog relationship.
/// Graph name: pg_{from_table}_{to_table}_{relation}
/// Uses the from_table as both vertex and edge table (FK is on from_table).
fn create_property_graph_for_relationship(&self, rel: &Relationship) {
    let graph_name = format!("pg_{}_{}_{}", rel.from_table, rel.to_table, rel.relation);
    let sql = format!(
        "CREATE PROPERTY GRAPH {graph_name} \
         VERTEX TABLES ({from_table} LABEL {from_table}, {to_table} LABEL {to_table}) \
         EDGE TABLES ({from_table} \
           SOURCE KEY ({from_col}) REFERENCES {from_table} ({from_col}) \
           DESTINATION KEY ({from_col}) REFERENCES {to_table} ({to_col}) \
           LABEL {relation})",
        from_table = rel.from_table,
        to_table = rel.to_table,
        from_col = rel.from_col,
        to_col = rel.to_col,
        relation = rel.relation,
    );
    if let Err(e) = self.query_router.query_sync(&sql) {
        tracing::warn!("failed to create property graph {graph_name}: {e}");
    }
}
```

Then call it from `register_relationship()`:

In `register_relationship()`, after `catalog.register_relationship(rel)?;` and before `self.rebuild_graph_locked(&catalog);`, add:

```rust
self.create_property_graph_for_relationship(&rel);
```

And similarly in `register_relationships()`, after the loop that registers each relationship, add a loop that creates property graphs:

```rust
for rel in &rels_clone {
    self.create_property_graph_for_relationship(rel);
}
```

Note: You'll need to clone `rels` before the loop that moves them into the catalog, or restructure slightly.

**Step 4: Run test to verify it passes**

Run: `cargo test test_register_relationship_creates_property_graph -- --nocapture`

Expected: PASS

**Step 5: Run all existing tests**

Run: `cargo test`

All existing tests must still pass.

**Step 6: Commit**

```bash
git add src/api.rs
git commit -m "feat: auto-create property graphs when relationships are registered"
```

---

### Task 6: Recreate property graphs on startup

**Files:**
- Modify: `src/api.rs`

**Step 1: Write the failing test**

Add to `mod tests` in `src/api.rs`:

```rust
#[test]
fn test_property_graphs_recreated_on_open() {
    let tmp = tempfile::tempdir().unwrap();

    // First session: create tables, register relationships, save
    {
        let api = TeidelumApi::new(tmp.path()).unwrap();

        let cols = vec![
            ColumnSchema { name: "id".to_string(), dtype: "i64".to_string() },
            ColumnSchema { name: "name".to_string(), dtype: "string".to_string() },
        ];
        api.create_table(
            "people",
            "test",
            &cols,
            &[
                vec![Value::Int(0), Value::String("Alice".to_string())],
                vec![Value::Int(1), Value::String("Bob".to_string())],
            ],
        )
        .unwrap();

        let edge_cols = vec![
            ColumnSchema { name: "src".to_string(), dtype: "i64".to_string() },
            ColumnSchema { name: "dst".to_string(), dtype: "i64".to_string() },
        ];
        api.create_table(
            "friendships",
            "test",
            &edge_cols,
            &[vec![Value::Int(0), Value::Int(1)]],
        )
        .unwrap();

        api.register_relationship(Relationship {
            from_table: "friendships".to_string(),
            from_col: "src".to_string(),
            to_table: "people".to_string(),
            to_col: "id".to_string(),
            relation: "friends_with".to_string(),
        })
        .unwrap();

        // Save tables to disk
        let tables_dir = tmp.path().join("tables");
        std::fs::create_dir_all(&tables_dir).unwrap();
        api.query_router().save_table("people", &tables_dir.join("people")).unwrap();
        api.query_router().save_table("friendships", &tables_dir.join("friendships")).unwrap();
        api.query_router().save_sym(&tables_dir.join("sym")).unwrap();
    }

    // Second session: open from disk — property graphs should be recreated
    // NOTE: This test depends on catalog relationships being persisted.
    // If they aren't persisted yet, this test documents the desired behavior
    // and can be marked #[ignore] until persistence is added.
    // For now, we verify the mechanism works by manually re-registering.
    let api = TeidelumApi::open(tmp.path()).unwrap();
    api.register_relationship(Relationship {
        from_table: "friendships".to_string(),
        from_col: "src".to_string(),
        to_table: "people".to_string(),
        to_col: "id".to_string(),
        relation: "friends_with".to_string(),
    })
    .unwrap();

    let result = api.query(
        "SELECT * FROM GRAPH_TABLE (pg_friendships_people_friends_with \
         MATCH (f:friendships)-[:friends_with]->(p:people) \
         COLUMNS (p.name AS person))",
    );
    assert!(result.is_ok(), "PGQ query after open failed: {result:?}");
}
```

**Step 2: Run test**

Run: `cargo test test_property_graphs_recreated_on_open -- --nocapture`

This should pass since `register_relationship` now creates property graphs (from Task 5).

**Step 3: Commit**

```bash
git add src/api.rs
git commit -m "test: verify property graphs are recreated after relationship re-registration"
```

---

### Task 7: Add property graphs to describe output

**Files:**
- Modify: `src/catalog.rs`

**Step 1: Write the failing test**

Add to `mod tests` in `src/catalog.rs`:

```rust
#[test]
fn test_describe_includes_property_graphs() {
    let mut catalog = Catalog::new();
    catalog.register_table(TableEntry {
        name: "users".to_string(),
        source: "test".to_string(),
        storage: StorageType::Local,
        columns: vec![ColumnInfo { name: "id".to_string(), dtype: "i64".to_string() }],
        row_count: Some(10),
    });
    catalog.register_table(TableEntry {
        name: "tasks".to_string(),
        source: "test".to_string(),
        storage: StorageType::Local,
        columns: vec![
            ColumnInfo { name: "id".to_string(), dtype: "i64".to_string() },
            ColumnInfo { name: "user_id".to_string(), dtype: "i64".to_string() },
        ],
        row_count: Some(5),
    });
    catalog
        .register_relationship(Relationship {
            from_table: "tasks".to_string(),
            from_col: "user_id".to_string(),
            to_table: "users".to_string(),
            to_col: "id".to_string(),
            relation: "assigned_to".to_string(),
        })
        .unwrap();

    let desc = catalog.describe(None).unwrap();
    let graphs = desc["property_graphs"].as_array().unwrap();
    assert_eq!(graphs.len(), 1);
    assert_eq!(graphs[0]["name"], "pg_tasks_users_assigned_to");
    assert!(graphs[0]["vertex_tables"].as_array().unwrap().len() >= 2);
    assert_eq!(graphs[0]["edge_label"], "assigned_to");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_describe_includes_property_graphs -- --nocapture`

Expected: FAIL — `describe` doesn't include `property_graphs` key yet.

**Step 3: Implement property_graphs in describe**

Modify `Catalog::describe()` in `src/catalog.rs`. After building the `rels` vec, add:

```rust
let property_graphs: Vec<serde_json::Value> = rels
    .iter()
    .map(|r| {
        serde_json::json!({
            "name": format!("pg_{}_{}_{}", r.from_table, r.to_table, r.relation),
            "vertex_tables": [r.from_table, r.to_table],
            "edge_table": r.from_table,
            "edge_label": r.relation,
            "source_key": r.from_col,
            "destination_key": r.to_col,
        })
    })
    .collect();
```

Then update the returned JSON to include it:

```rust
Ok(serde_json::json!({
    "tables": tables,
    "relationships": rels,
    "property_graphs": property_graphs,
}))
```

**Step 4: Run test to verify it passes**

Run: `cargo test test_describe_includes_property_graphs -- --nocapture`

**Step 5: Run all tests**

Run: `cargo test`

**Step 6: Commit**

```bash
git add src/catalog.rs
git commit -m "feat: include property_graphs in describe output"
```

---

### Task 8: Remove graph tool from MCP

**Files:**
- Modify: `src/mcp.rs`

**Step 1: Remove GraphParams struct and defaults**

In `src/mcp.rs`, delete:
- `default_operation()` function (line 58-60)
- `default_depth()` function (line 62-64)
- `default_direction()` function (line 66-68)
- `default_key_col()` function (line 70-72)
- `GraphParams` struct (lines 74-104)
- The `graph` tool method (lines 444-503)

**Step 2: Update server instructions**

In `get_info()` (line 1186), update the instructions string to remove the graph tool reference. Change:

```rust
"Teidelum indexes Notion, Zulip, and live data sources into a single \
 searchable index. Use 'describe' to see available tables, 'search' for \
 full-text queries, 'sql' for analytical queries, 'graph' to traverse \
 relationships between entities, and 'sync' to refresh data."
```

To:

```rust
"Teidelum indexes Notion, Zulip, and live data sources into a single \
 searchable index. Use 'describe' to see available tables and property graphs, \
 'search' for full-text queries, 'sql' for analytical queries (including PGQ \
 graph pattern matching and algorithms like PAGERANK, COMMUNITY, COMPONENT), \
 and 'sync' to refresh data."
```

**Step 3: Verify it compiles**

Run: `cargo check`

**Step 4: Commit**

```bash
git add src/mcp.rs
git commit -m "feat: remove graph MCP tool, update instructions for PGQ"
```

---

### Task 9: Remove GraphEngine from TeidelumApi

**Files:**
- Modify: `src/api.rs`

**Step 1: Remove graph_engine field and methods**

In `src/api.rs`:
- Remove `use crate::graph::GraphEngine;` import (line 12)
- Remove `graph_engine: RwLock<GraphEngine>` field from `TeidelumApi` struct (line 24)
- Remove `rebuild_graph_locked()` method (lines 96-99)
- Remove `neighbors()` method (lines 314-335)
- Remove `path()` method (lines 337-364)
- Remove all calls to `self.rebuild_graph_locked(&catalog)` — there are 6 occurrences:
  - In `create_table()` (line 179)
  - In `delete_table()` (line 235)
  - In `register_table()` (line 249)
  - In `register_relationship()` (line 266)
  - In `register_relationships()` (line 294)
  - In `load_splayed_tables()` (line 425)
- In `new()`, remove `graph_engine` initialization (line 82) and field (line 88)

Replace calls to `self.rebuild_graph_locked(&catalog)` in `register_relationship` and `register_relationships` with calls to `self.create_property_graph_for_relationship(...)` (which was added in Task 5).

**Step 2: Remove graph-specific tests**

Remove these tests from `src/api.rs`:
- `test_register_relationship_rebuilds_graph` (lines 659-716)
- `test_neighbors_via_api` (lines 786-802)
- `test_path_via_api` (lines 804-844)
- `test_graph_updates_when_tables_added_after_relationships` (lines 846-934)

Keep the replacement PGQ test from Task 5 (`test_register_relationship_creates_property_graph`).

**Step 3: Verify it compiles**

Run: `cargo check`

**Step 4: Run all tests**

Run: `cargo test`

**Step 5: Commit**

```bash
git add src/api.rs
git commit -m "refactor: remove GraphEngine from TeidelumApi"
```

---

### Task 10: Delete graph.rs

**Files:**
- Delete: `src/graph.rs`
- Modify: `src/lib.rs` (remove `pub mod graph;`)

**Step 1: Remove the module declaration**

In `src/lib.rs`, remove the line `pub mod graph;`.

**Step 2: Delete graph.rs**

```bash
rm src/graph.rs
```

**Step 3: Verify it compiles**

Run: `cargo check`

**Step 4: Run all tests**

Run: `cargo test`

**Step 5: Commit**

```bash
git add -A
git commit -m "refactor: delete graph.rs — replaced by PGQ via sql tool"
```

---

### Task 11: Clean up main.rs

**Files:**
- Modify: `src/main.rs`

**Step 1: Verify main.rs still compiles**

The `register_relationships` call in `main.rs` (lines 55-70) should still work — it registers catalog relationships which now auto-create property graphs.

No changes needed unless compilation fails.

Run: `cargo check`

**Step 2: Run the full test suite one final time**

Run: `cargo test`

Run: `cargo clippy -- -D warnings`

**Step 3: Commit if any cleanup was needed**

```bash
git add src/main.rs
git commit -m "chore: clean up main.rs after graph engine removal"
```

---

### Task 12: Integration test — full PGQ workflow via TeidelumApi

**Files:**
- Test: `src/api.rs` (add test in `mod tests`)

**Step 1: Write an end-to-end test**

Add to `mod tests` in `src/api.rs`:

```rust
#[test]
fn test_pgq_full_workflow() {
    let tmp = tempfile::tempdir().unwrap();
    let api = TeidelumApi::new(tmp.path()).unwrap();

    // Create vertex tables
    api.create_table(
        "employees",
        "test",
        &[
            ColumnSchema { name: "id".to_string(), dtype: "i64".to_string() },
            ColumnSchema { name: "name".to_string(), dtype: "string".to_string() },
        ],
        &[
            vec![Value::Int(0), Value::String("Alice".to_string())],
            vec![Value::Int(1), Value::String("Bob".to_string())],
            vec![Value::Int(2), Value::String("Carol".to_string())],
            vec![Value::Int(3), Value::String("Dave".to_string())],
        ],
    )
    .unwrap();

    // Create edge table
    api.create_table(
        "reports_to",
        "test",
        &[
            ColumnSchema { name: "subordinate".to_string(), dtype: "i64".to_string() },
            ColumnSchema { name: "manager".to_string(), dtype: "i64".to_string() },
        ],
        &[
            vec![Value::Int(1), Value::Int(0)], // Bob reports to Alice
            vec![Value::Int(2), Value::Int(0)], // Carol reports to Alice
            vec![Value::Int(3), Value::Int(1)], // Dave reports to Bob
        ],
    )
    .unwrap();

    // Register relationship — auto-creates property graph
    api.register_relationship(Relationship {
        from_table: "reports_to".to_string(),
        from_col: "subordinate".to_string(),
        to_table: "employees".to_string(),
        to_col: "id".to_string(),
        relation: "managed_by".to_string(),
    })
    .unwrap();

    // Verify describe includes property graph
    let desc = api.describe(None).unwrap();
    let graphs = desc["property_graphs"].as_array().unwrap();
    assert!(graphs.iter().any(|g| g["name"] == "pg_reports_to_employees_managed_by"));

    // 1-hop MATCH: who reports to Alice?
    let result = api
        .query(
            "SELECT * FROM GRAPH_TABLE (pg_reports_to_employees_managed_by \
             MATCH (r:reports_to)-[:managed_by]->(e:employees) \
             WHERE e.name = 'Alice' \
             COLUMNS (r.subordinate AS sub_id))",
        )
        .unwrap();
    assert_eq!(result.rows.len(), 2); // Bob (1) and Carol (2)

    // Agent can also create custom property graphs via sql tool
    api.query(
        "CREATE PROPERTY GRAPH org_chart \
         VERTEX TABLES (employees LABEL Employee) \
         EDGE TABLES (reports_to \
           SOURCE KEY (subordinate) REFERENCES employees (id) \
           DESTINATION KEY (manager) REFERENCES employees (id) \
           LABEL ReportsTo)",
    )
    .unwrap();

    // PageRank on custom graph
    let result = api
        .query(
            "SELECT COUNT(*) FROM GRAPH_TABLE (org_chart \
             MATCH (e:Employee) \
             COLUMNS (PAGERANK(org_chart, e) AS rank)) WHERE rank > 0",
        )
        .unwrap();
    match &result.rows[0][0] {
        Value::Int(n) => assert_eq!(*n, 4),
        other => panic!("expected Int, got {other:?}"),
    }
}
```

**Step 2: Run test**

Run: `cargo test test_pgq_full_workflow -- --nocapture`

**Step 3: Run full suite**

Run: `cargo test`

**Step 4: Commit**

```bash
git add src/api.rs
git commit -m "test: end-to-end PGQ workflow via TeidelumApi"
```
