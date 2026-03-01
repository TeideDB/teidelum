# Plan: Graph Layer

Add native graph traversal to the teide stack — FFI bindings and safe Rust wrappers in teide-rs, SQL-based graph engine and MCP tool in teidelum.

## Context

The C engine at `teide/` has a fully implemented graph layer:

- CSR storage (`csr.c/h`)
- Graph opcodes (`OP_EXPAND=80`, `OP_VAR_EXPAND=81`, `OP_SHORTEST_PATH=82`, `OP_WCO_JOIN=83`)
- Executor implementations (`exec.c`)
- Leapfrog Triejoin (`lftj.c/h`)
- Public API in `include/teide/td.h` (lines 866-891)

The vendor copy in `teide-rs/vendor/teide/` was outdated (commit `a72ce65` vs main repo `9b8f42b`) — it lacked all graph code. And teide-rs had no FFI bindings or Rust wrappers for the graph API.

## Phase 1: SQL-Based Graph Traversal [DONE]

Immediate graph capabilities using SQL-based FK traversal over catalog relationships. Suitable for demo-scale data without requiring the full CSR pipeline.

### Step 1: Update teide-rs vendor

Pull the latest teide C engine into `teide-rs/vendor/teide/`:

```bash
cd teide-rs/vendor/teide && git pull origin master
```

This brings in:

| File | Purpose |
|------|---------|
| `src/store/csr.c`, `csr.h` | CSR build/save/load/mmap |
| `src/ops/lftj.c`, `lftj.h` | Leapfrog Triejoin |
| `src/ops/fvec.h` | Factorized vector types |
| `include/teide/td.h` | Graph opcodes, `td_rel_t`, graph API |
| `src/ops/graph.c` | `td_expand`, `td_var_expand`, `td_shortest_path`, `td_wco_join` DAG builders |
| `src/ops/exec.c` | `exec_expand`, `exec_var_expand`, `exec_shortest_path`, `exec_wco_join` |

The `build.rs` already walks all `.c` files recursively, so new files compile automatically.

### Step 2: teide-rs FFI bindings

**File: `teide-rs/src/ffi.rs`**

Add graph FFI declarations:

- Direction constants: `TD_DIR_FWD=0`, `TD_DIR_REV=1`, `TD_DIR_BOTH=2`
- Graph opcodes: `OP_EXPAND=80`, `OP_VAR_EXPAND=81`, `OP_SHORTEST_PATH=82`, `OP_WCO_JOIN=83`
- Opaque `td_rel_t` struct (zero-sized, C-allocated/C-freed)
- `extern "C"` for CSR API: `td_rel_build`, `td_rel_from_edges`, `td_rel_save/load/mmap/free`
- `extern "C"` for graph DAG: `td_expand`, `td_var_expand`, `td_shortest_path`, `td_wco_join`
- `extern "C"` for table registration: `td_graph_add_table`, `td_scan_table`
- Update `td_graph_t` struct: add `tables: *mut *mut td_t` and `n_tables: u16` fields (size 48 → 64)

### Step 3: teide-rs safe Rust wrappers

**File: `teide-rs/src/engine.rs`**

Add `Rel` RAII type:

```rust
pub struct Rel { ptr: *mut ffi::td_rel_t }

impl Rel {
    pub fn build(from_table, fk_col, n_target_nodes, sort) -> Result<Self>;
    pub fn from_edges(edge_table, src_col, dst_col, n_src, n_dst, sort) -> Result<Self>;
    pub fn load(dir) -> Result<Self>;
    pub fn mmap(dir) -> Result<Self>;
    pub fn save(&self, dir) -> Result<()>;
    pub fn as_raw(&self) -> *mut ffi::td_rel_t;
}

impl Drop for Rel { fn drop(&mut self) { td_rel_free } }
```

Add graph methods on `Graph<'a>`:

```rust
impl Graph<'a> {
    pub fn add_table(&self, table: &Table) -> u16;
    pub fn scan_table(&self, table_idx: u16) -> Result<Column>;
    pub fn expand(&self, src_nodes: Column, rel: &Rel, direction: u8) -> Result<Column>;
    pub fn var_expand(&self, start, rel, direction, min_depth, max_depth, track_path) -> Result<Column>;
    pub fn shortest_path(&self, src, dst, rel, max_depth) -> Result<Column>;
    pub fn wco_join(&self, rels: &[&Rel], n_vars: u8) -> Result<Column>;
}
```

Re-exported from `lib.rs` via existing `pub use engine::*`.

### Step 4: teidelum GraphEngine

**New file: `teidelum/src/graph.rs`**

SQL-based graph traversal engine using catalog FK relationships:

```rust
pub struct GraphEngine {
    relationships: Vec<Relationship>,
}

impl GraphEngine {
    pub fn build_from_catalog(catalog: &Catalog) -> Self;
    pub fn neighbors(table, key_col, key_value, depth, direction, rel_types, router) -> Result<Value>;
    pub fn path(from_table, from_key_col, from_key, to_table, to_key_col, to_key, max_depth, direction, rel_types, router) -> Result<Value>;
}
```

Design: BFS traversal using SQL queries against FK relationships. For each hop, resolves neighbors via `SELECT ... WHERE fk_col = 'value'`. Supports forward, reverse, and bidirectional traversal with relationship type filtering. `neighbors` returns the full subgraph (nodes + edges) within the given depth, serving the role originally planned for a separate `subgraph` operation.

Internal helpers:

| Method | Purpose |
|--------|---------|
| `find_relationships()` | Filter catalog relationships by table, direction, type |
| `resolve_neighbors()` | SQL-based FK resolution (forward and reverse) |
| `infer_key_col()` | Heuristic for identifying key column in neighbor tables |
| `fetch_node_properties()` | `SELECT * FROM table WHERE key_col = 'value'` → JSON |

**File: `teidelum/src/router.rs`** — add helper:

```rust
pub fn query_column_values(&self, table: &str, col: &str) -> Result<Vec<Option<String>>>;
```

### Step 5: Graph MCP tool

**File: `teidelum/src/mcp.rs`**

Add `GraphParams`:

```rust
pub struct GraphParams {
    pub table: String,              // starting node table
    pub key: String,                // node identifier value
    pub key_col: String,            // key column (default: "name")
    pub operation: String,          // "neighbors" | "path"
    pub depth: usize,               // max hops (default: 2)
    pub direction: String,          // "forward" | "reverse" | "both"
    pub rel_types: Option<Vec<String>>,
    pub to_table: Option<String>,   // for "path"
    pub to_key: Option<String>,     // for "path"
    pub to_key_col: Option<String>, // for "path"
}
```

Add `graph_engine: Arc<GraphEngine>` to `Teidelum`, `#[tool] async fn graph()` dispatching to neighbors/path. Update `get_info()` instructions.

### Step 6: Wire up in main.rs

After `load_tables()`, register FK relationships and build GraphEngine:

```rust
catalog.register_relationship(Relationship {
    from_table: "project_tasks", from_col: "assignee",
    to_table: "team_members",   to_col: "name",
    relation: "assigned_to",
});
catalog.register_relationship(Relationship {
    from_table: "incidents",     from_col: "reporter",
    to_table: "team_members",    to_col: "name",
    relation: "reported_by",
});

let graph_engine = GraphEngine::build_from_catalog(&catalog);
let server = Teidelum::new(catalog, search_engine, query_router, graph_engine);
```

### Verification

```bash
cd teide-rs && cargo build && cargo test -- --test-threads=1
cd teidelum && cargo build && cargo test -- --test-threads=1
cargo clippy -- -D warnings  # both crates
cargo fmt --check             # both crates
```

Manual MCP test:

- `graph(table="team_members", key="Alice Chen", operation="neighbors")` → tasks + incidents
- `graph(table="project_tasks", key="Implement JWT token rotation", key_col="title", operation="neighbors", depth=2)` → task → assignee → related

### Results

- teide-rs: 81 tests passed
- teidelum: 2 tests passed
- Clippy clean, fmt clean

Commits:

- teide-rs: `ff85c9b` — Add graph FFI bindings and safe Rust wrappers
- teidelum: `1a04575` — Add graph engine with SQL-based FK traversal and MCP graph tool

## Phase 2: CSR-Based Graph Traversal [TODO]

When data scale justifies it, replace SQL-based traversal with native CSR:

1. `GraphEngine` builds `teide::Rel` from edge tables via `Rel::from_edges()`
2. Uses `Graph::expand()` / `Graph::var_expand()` / `Graph::shortest_path()` for traversal
3. Property fetching via row offset gather on columnar storage
4. Automatic CSR rebuild on sync via catalog FK change detection

Transparent to the MCP tool — same `GraphParams`, same JSON output.

## Phase 3: SQL+MATCH Planner [TODO]

Extend teide-rs SQL planner to recognize MATCH clauses and emit graph opcodes:

```sql
SELECT p.name
FROM task t
MATCH (t)-[:assigned_to]->(p:person)
WHERE t.status = 'open'
```

Planner detects pattern shape → acyclic chain (OP_EXPAND), variable-length (OP_VAR_EXPAND), cyclic (OP_WCO_JOIN). See GRAPH.md for full design.

## Phase 4: Factorized Processing + SIP [TODO]

- Factorized vectors (`td_fvec_t`) to avoid materializing cross-products
- ASP-Join for factorized inputs
- SIP optimizer pass for backward filter propagation through OP_EXPAND chains

See GRAPH.md for algorithm details and implementation plan.
