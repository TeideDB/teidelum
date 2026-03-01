use std::collections::{HashMap, HashSet};

use anyhow::{bail, Result};
use serde_json::json;

use crate::catalog::{Catalog, Relationship};
use crate::connector::Value;
use crate::router::QueryRouter;

/// Maximum BFS traversal depth to prevent unbounded query storms.
const MAX_DEPTH: usize = 10;

/// Escape a string value for use in SQL single-quoted literals.
/// Replaces `'` with `''` to prevent SQL injection.
fn escape_sql_value(s: &str) -> String {
    s.replace('\'', "''")
}

/// Validate that a string is a safe SQL identifier.
/// Must start with a letter or underscore, then alphanumeric or underscores.
fn is_valid_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {
            chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        }
        _ => false,
    }
}

/// Describes one edge in a graph traversal result.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Edge {
    pub from_table: String,
    pub from_key_col: String,
    pub from_key: String,
    pub to_table: String,
    pub to_key_col: String,
    pub to_key: String,
    pub relation: String,
}

/// Parent info for BFS path reconstruction: ((parent_table, parent_key_col, parent_key), relation_name).
type PathParent = Option<((String, String, String), String)>;

/// All column values for a target row, used for flexible target matching in path().
type TargetRow = HashMap<String, String>;

/// SQL-based graph traversal engine over catalog FK relationships.
///
/// Performs BFS traversal using SQL queries at each hop, resolving
/// neighbors via FK relationships registered in the catalog.
pub struct GraphEngine {
    /// Catalog snapshot for relationship lookups.
    relationships: Vec<Relationship>,
    /// Column names per table for identity column resolution.
    table_columns: HashMap<String, Vec<String>>,
}

impl GraphEngine {
    /// Build a GraphEngine from the catalog's registered relationships.
    pub fn build_from_catalog(catalog: &Catalog) -> Self {
        let table_columns: HashMap<String, Vec<String>> = catalog
            .tables()
            .iter()
            .map(|t| {
                (
                    t.name.clone(),
                    t.columns.iter().map(|c| c.name.clone()).collect(),
                )
            })
            .collect();
        GraphEngine {
            relationships: catalog.relationships().to_vec(),
            table_columns,
        }
    }

    /// Find neighbors of a node up to `depth` hops.
    #[allow(clippy::too_many_arguments)]
    pub fn neighbors(
        &self,
        table: &str,
        key_col: &str,
        key_value: &str,
        depth: usize,
        direction: &str,
        rel_types: Option<&[String]>,
        router: &QueryRouter,
    ) -> Result<serde_json::Value> {
        if !is_valid_identifier(table) {
            bail!("invalid table name: {table}");
        }
        if !is_valid_identifier(key_col) {
            bail!("invalid column name: {key_col}");
        }
        if !matches!(direction, "forward" | "reverse" | "both") {
            bail!("invalid direction: '{direction}'. Use 'forward', 'reverse', or 'both'");
        }
        let depth = depth.min(MAX_DEPTH);
        let mut visited: HashMap<(String, String, String), serde_json::Value> = HashMap::new();
        let mut edges: Vec<Edge> = Vec::new();
        let mut seen_edges: HashSet<(String, String, String, String, String, String, String)> =
            HashSet::new();
        let mut frontier: Vec<(String, String, String)> = vec![(
            table.to_string(),
            key_col.to_string(),
            key_value.to_string(),
        )];

        // Fetch and store the starting node
        let props = self
            .fetch_node_properties(table, key_col, key_value, router)
            .map_err(|_| {
                anyhow::anyhow!("starting node not found: {table}.{key_col}={key_value}")
            })?;
        visited.insert(
            (
                table.to_string(),
                key_col.to_string(),
                key_value.to_string(),
            ),
            props,
        );

        for _d in 0..depth {
            let mut next_frontier = Vec::new();

            for (tbl, kcol, kval) in &frontier {
                let rels = self.find_relationships(tbl, direction, rel_types);

                for rel in &rels {
                    let (neighbor_table, neighbor_col, source_col, is_forward) =
                        if rel.from_table == *tbl {
                            (&rel.to_table, &rel.to_col, &rel.from_col, true)
                        } else {
                            (&rel.from_table, &rel.from_col, &rel.to_col, false)
                        };

                    // SQL lookup: find neighbors via FK
                    let neighbor_values = self.resolve_neighbors(
                        tbl,
                        kcol,
                        kval,
                        source_col,
                        neighbor_table,
                        neighbor_col,
                        is_forward,
                        router,
                    )?;

                    for (id_col, nval) in &neighbor_values {
                        let key = (neighbor_table.clone(), id_col.clone(), nval.clone());

                        // Record edges unconditionally (dedup by full edge identity)
                        let edge = if is_forward {
                            Edge {
                                from_table: tbl.clone(),
                                from_key_col: kcol.clone(),
                                from_key: kval.clone(),
                                to_table: neighbor_table.clone(),
                                to_key_col: id_col.clone(),
                                to_key: nval.clone(),
                                relation: rel.relation.clone(),
                            }
                        } else {
                            Edge {
                                from_table: neighbor_table.clone(),
                                from_key_col: id_col.clone(),
                                from_key: nval.clone(),
                                to_table: tbl.clone(),
                                to_key_col: kcol.clone(),
                                to_key: kval.clone(),
                                relation: rel.relation.clone(),
                            }
                        };
                        let edge_key = (
                            edge.from_table.clone(),
                            edge.from_key_col.clone(),
                            edge.from_key.clone(),
                            edge.to_table.clone(),
                            edge.to_key_col.clone(),
                            edge.to_key.clone(),
                            edge.relation.clone(),
                        );
                        if seen_edges.insert(edge_key) {
                            edges.push(edge);
                        }

                        // Only add newly-discovered nodes to the frontier
                        if let std::collections::hash_map::Entry::Vacant(e) = visited.entry(key) {
                            let props = self
                                .fetch_node_properties(neighbor_table, id_col, nval, router)
                                .unwrap_or(serde_json::Value::Null);
                            e.insert(props);
                            next_frontier.push((
                                neighbor_table.clone(),
                                id_col.clone(),
                                nval.clone(),
                            ));
                        }
                    }
                }
            }

            if next_frontier.is_empty() {
                break;
            }
            frontier = next_frontier;
        }

        let nodes: Vec<serde_json::Value> = visited
            .into_iter()
            .map(|((tbl, kcol, key), props)| {
                json!({
                    "table": tbl,
                    "key_col": kcol,
                    "key": key,
                    "properties": props,
                })
            })
            .collect();

        Ok(json!({
            "nodes": nodes,
            "edges": edges,
        }))
    }

    /// Find shortest path between two nodes.
    #[allow(clippy::too_many_arguments)]
    pub fn path(
        &self,
        from_table: &str,
        from_key_col: &str,
        from_key: &str,
        to_table: &str,
        to_key_col: &str,
        to_key: &str,
        max_depth: usize,
        direction: &str,
        rel_types: Option<&[String]>,
        router: &QueryRouter,
    ) -> Result<serde_json::Value> {
        if !is_valid_identifier(from_table) {
            bail!("invalid table name: {from_table}");
        }
        if !is_valid_identifier(from_key_col) {
            bail!("invalid column name: {from_key_col}");
        }
        if !is_valid_identifier(to_table) {
            bail!("invalid table name: {to_table}");
        }
        if !is_valid_identifier(to_key_col) {
            bail!("invalid column name: {to_key_col}");
        }
        if !matches!(direction, "forward" | "reverse" | "both") {
            bail!("invalid direction: '{direction}'. Use 'forward', 'reverse', or 'both'");
        }
        let max_depth = max_depth.min(MAX_DEPTH);

        // Verify the source node exists before starting BFS
        self.fetch_node_properties(from_table, from_key_col, from_key, router)
            .map_err(|_| {
                anyhow::anyhow!("source node not found: {from_table}.{from_key_col}={from_key}")
            })?;

        // Pre-resolve the target row: BFS discovers nodes using an identity
        // column from resolve_neighbors (which may differ from to_key_col).
        // Fetch all column values of the target row so we can match against
        // whichever column BFS uses as identity for nodes in to_table.
        let target_row = self
            .fetch_target_row(to_table, to_key_col, to_key, router)
            .map_err(|_| {
                anyhow::anyhow!("target node not found: {to_table}.{to_key_col}={to_key}")
            })?;

        // Trivial case: source is target
        if from_table == to_table {
            if let Some(expected) = target_row.get(from_key_col) {
                if expected == from_key {
                    return Ok(json!({
                        "found": true,
                        "path": [{"table": from_table, "key_col": from_key_col, "key": from_key}],
                        "hops": 0,
                    }));
                }
            }
        }

        // BFS from source to destination
        let mut visited: HashMap<(String, String, String), PathParent> = HashMap::new();
        visited.insert(
            (
                from_table.to_string(),
                from_key_col.to_string(),
                from_key.to_string(),
            ),
            None,
        );

        let mut frontier: Vec<(String, String, String)> = vec![(
            from_table.to_string(),
            from_key_col.to_string(),
            from_key.to_string(),
        )];

        let mut found_key: Option<(String, String, String)> = None;

        for _d in 0..max_depth {
            let mut next_frontier = Vec::new();

            'frontier: for (tbl, kcol, kval) in &frontier {
                let rels = self.find_relationships(tbl, direction, rel_types);

                for rel in &rels {
                    let (neighbor_table, neighbor_col, source_col, is_forward) =
                        if rel.from_table == *tbl {
                            (&rel.to_table, &rel.to_col, &rel.from_col, true)
                        } else {
                            (&rel.from_table, &rel.from_col, &rel.to_col, false)
                        };

                    let neighbor_values = self.resolve_neighbors(
                        tbl,
                        kcol,
                        kval,
                        source_col,
                        neighbor_table,
                        neighbor_col,
                        is_forward,
                        router,
                    )?;

                    for (id_col, nval) in &neighbor_values {
                        let key = (neighbor_table.clone(), id_col.clone(), nval.clone());
                        if !visited.contains_key(&key) {
                            visited.insert(
                                key.clone(),
                                Some((
                                    (tbl.clone(), kcol.clone(), kval.clone()),
                                    rel.relation.clone(),
                                )),
                            );
                            next_frontier.push((
                                neighbor_table.clone(),
                                id_col.clone(),
                                nval.clone(),
                            ));

                            // Match target: check if this node is in the target
                            // table and its id_col value matches what the target
                            // row has for that column.
                            if *neighbor_table == to_table {
                                if let Some(expected) = target_row.get(id_col) {
                                    if expected == nval {
                                        found_key = Some(key);
                                        break 'frontier;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if found_key.is_some() || next_frontier.is_empty() {
                break;
            }
            frontier = next_frontier;
        }

        let Some(target) = found_key else {
            return Ok(json!({
                "found": false,
                "message": format!(
                    "no path from {from_table}.{from_key} to {to_table}.{to_key} within {max_depth} hops"
                ),
            }));
        };

        // Reconstruct path
        let mut path = Vec::new();
        let mut current = target;
        while let Some(Some((parent, relation))) = visited.get(&current) {
            path.push(json!({
                "table": current.0,
                "key_col": current.1,
                "key": current.2,
                "via_relation": relation,
            }));
            current = parent.clone();
        }
        // Add source node
        path.push(json!({
            "table": current.0,
            "key_col": current.1,
            "key": current.2,
        }));
        path.reverse();

        Ok(json!({
            "found": true,
            "path": path,
            "hops": path.len() - 1,
        }))
    }

    // ---- Internal helpers ----

    /// Find relationships involving a table, filtered by direction and type.
    fn find_relationships(
        &self,
        table: &str,
        direction: &str,
        rel_types: Option<&[String]>,
    ) -> Vec<&Relationship> {
        self.relationships
            .iter()
            .filter(|r| {
                let matches_table = match direction {
                    "forward" => r.from_table == table,
                    "reverse" => r.to_table == table,
                    _ => r.from_table == table || r.to_table == table,
                };
                let matches_type = rel_types
                    .map(|types| types.iter().any(|t| t == &r.relation))
                    .unwrap_or(true);
                matches_table && matches_type
            })
            .collect()
    }

    /// Resolve neighbor values via SQL, returning (id_col, id_value) pairs.
    ///
    /// If `is_forward`: we have a row in `source_table` where `key_col=key_value`,
    /// and `source_col` is the FK column. We need to find matching rows in
    /// `neighbor_table` where `neighbor_col` matches the FK value.
    /// Returns `neighbor_col` as the id column.
    ///
    /// If `!is_forward` (reverse): we look in `neighbor_table` for rows whose
    /// `neighbor_col` (FK) value matches our `key_value`, and return a
    /// distinguishing identity column for each matched row.
    #[allow(clippy::too_many_arguments)]
    fn resolve_neighbors(
        &self,
        source_table: &str,
        key_col: &str,
        key_value: &str,
        source_col: &str,
        neighbor_table: &str,
        neighbor_col: &str,
        is_forward: bool,
        router: &QueryRouter,
    ) -> Result<Vec<(String, String)>> {
        let escaped_key = escape_sql_value(key_value);
        if is_forward {
            // Forward: get FK value from source, then find matching rows in neighbor
            let sql = format!(
                "SELECT {source_col} FROM {source_table} WHERE {key_col} = '{escaped_key}'"
            );
            let result = router.query_sync(&sql)?;
            let id_col = neighbor_col.to_string();
            let mut neighbors = Vec::new();
            for row in &result.rows {
                match row.first() {
                    Some(Value::String(fk_val)) => {
                        neighbors.push((id_col.clone(), fk_val.clone()));
                    }
                    Some(Value::Int(i)) => {
                        neighbors.push((id_col.clone(), i.to_string()));
                    }
                    _ => {}
                }
            }
            Ok(neighbors)
        } else {
            // Reverse: find rows in neighbor_table where neighbor_col (FK) = key_value.
            // We need a unique identity column for each matched row.
            //
            // Strategy: check if any relationship references this table as a target
            // (to_table). The to_col of such a relationship is expected to be unique
            // (it's an FK target), so it's a reliable identity. Fall back to first
            // non-FK column if no such relationship exists.
            let id_col_name = self.find_identity_column(neighbor_table, neighbor_col);

            if !is_valid_identifier(&id_col_name) {
                bail!("invalid identity column name: {id_col_name}");
            }

            let sql = format!(
                "SELECT {id_col_name}, {neighbor_col} FROM {neighbor_table} WHERE {neighbor_col} = '{escaped_key}'"
            );
            let result = router.query_sync(&sql)?;

            Ok(result
                .rows
                .into_iter()
                .filter_map(|row| {
                    let val = row.first()?;
                    match val {
                        Value::String(s) => Some((id_col_name.clone(), s.clone())),
                        Value::Int(i) => Some((id_col_name.clone(), i.to_string())),
                        _ => None,
                    }
                })
                .collect())
        }
    }

    /// Find the best identity column for a table during reverse traversal.
    ///
    /// Prefers columns that are FK targets (to_col in a relationship pointing
    /// to this table), since FK targets are expected to be unique. Falls back
    /// to an "id" column from the table schema, then to the first non-FK
    /// column. Using the FK column itself as identity would collapse multiple
    /// rows sharing the same FK value into a single node.
    fn find_identity_column(&self, table: &str, fk_col: &str) -> String {
        // 1. Check if this table is a FK target in any relationship — the to_col
        //    is expected to be unique (it's what other tables reference).
        for rel in &self.relationships {
            if rel.to_table == table && rel.to_col != fk_col {
                return rel.to_col.clone();
            }
        }

        // 2. Check table schema for an "id" column (common primary key).
        if let Some(cols) = self.table_columns.get(table) {
            if cols.iter().any(|c| c == "id") && fk_col != "id" {
                return "id".to_string();
            }
            // 3. Fall back to first column that isn't the FK column.
            if let Some(col) = cols.iter().find(|c| c.as_str() != fk_col) {
                return col.clone();
            }
        }

        // Last resort: use the FK column (may collapse rows, but no better option).
        fk_col.to_string()
    }

    /// Fetch all column values of a target row for flexible BFS target matching.
    /// Returns a map of column_name -> string_value for the first matching row.
    fn fetch_target_row(
        &self,
        table: &str,
        key_col: &str,
        key_value: &str,
        router: &QueryRouter,
    ) -> Result<TargetRow> {
        let escaped_key = escape_sql_value(key_value);
        let sql = format!("SELECT * FROM {table} WHERE {key_col} = '{escaped_key}'");
        let result = router.query_sync(&sql)?;

        if result.rows.is_empty() {
            bail!("target row not found: {table}.{key_col}={key_value}");
        }

        let row = &result.rows[0];
        let mut target = HashMap::new();
        for (i, col) in result.columns.iter().enumerate() {
            let val = match row.get(i) {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Int(i)) => i.to_string(),
                Some(Value::Float(f)) => f.to_string(),
                Some(Value::Bool(b)) => b.to_string(),
                Some(Value::Null) | None => continue,
            };
            target.insert(col.name.clone(), val);
        }
        Ok(target)
    }

    /// Build a GraphEngine directly from a list of relationships.
    pub fn from_relationships(relationships: Vec<Relationship>) -> Self {
        GraphEngine {
            relationships,
            table_columns: HashMap::new(),
        }
    }

    /// Build a GraphEngine from relationships and table column metadata.
    pub fn from_relationships_with_columns(
        relationships: Vec<Relationship>,
        table_columns: HashMap<String, Vec<String>>,
    ) -> Self {
        GraphEngine {
            relationships,
            table_columns,
        }
    }

    /// Fetch all properties of a node as JSON.
    fn fetch_node_properties(
        &self,
        table: &str,
        key_col: &str,
        key_value: &str,
        router: &QueryRouter,
    ) -> Result<serde_json::Value> {
        let escaped_key = escape_sql_value(key_value);
        let sql = format!("SELECT * FROM {table} WHERE {key_col} = '{escaped_key}'");
        let result = router.query_sync(&sql)?;

        if result.rows.is_empty() {
            bail!("node not found: {table}.{key_col}={key_value}");
        }

        let row = &result.rows[0];
        let mut props = serde_json::Map::new();
        for (i, col) in result.columns.iter().enumerate() {
            let val = match row.get(i) {
                Some(Value::String(s)) => json!(s),
                Some(Value::Int(i)) => json!(i),
                Some(Value::Float(f)) => json!(f),
                Some(Value::Bool(b)) => json!(b),
                Some(Value::Null) | None => serde_json::Value::Null,
            };
            props.insert(col.name.clone(), val);
        }

        Ok(serde_json::Value::Object(props))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Unit tests for utility functions ----

    #[test]
    fn test_escape_sql_value_no_quotes() {
        assert_eq!(escape_sql_value("hello"), "hello");
    }

    #[test]
    fn test_escape_sql_value_single_quote() {
        assert_eq!(escape_sql_value("it's"), "it''s");
    }

    #[test]
    fn test_escape_sql_value_multiple_quotes() {
        assert_eq!(escape_sql_value("a'b'c"), "a''b''c");
    }

    #[test]
    fn test_escape_sql_value_empty() {
        assert_eq!(escape_sql_value(""), "");
    }

    #[test]
    fn test_is_valid_identifier_valid() {
        assert!(is_valid_identifier("team_members"));
        assert!(is_valid_identifier("name"));
        assert!(is_valid_identifier("col1"));
    }

    #[test]
    fn test_is_valid_identifier_empty() {
        assert!(!is_valid_identifier(""));
    }

    #[test]
    fn test_is_valid_identifier_with_spaces() {
        assert!(!is_valid_identifier("has space"));
    }

    #[test]
    fn test_is_valid_identifier_injection_attempt() {
        assert!(!is_valid_identifier("'; DROP TABLE x;--"));
        assert!(!is_valid_identifier("name; --"));
    }

    #[test]
    fn test_is_valid_identifier_special_chars() {
        assert!(!is_valid_identifier("has-hyphen"));
        assert!(!is_valid_identifier("has.dot"));
    }

    // ---- Unit tests for find_relationships ----

    fn test_relationships() -> Vec<Relationship> {
        vec![
            Relationship {
                from_table: "project_tasks".to_string(),
                from_col: "assignee".to_string(),
                to_table: "team_members".to_string(),
                to_col: "name".to_string(),
                relation: "assigned_to".to_string(),
            },
            Relationship {
                from_table: "incidents".to_string(),
                from_col: "reporter".to_string(),
                to_table: "team_members".to_string(),
                to_col: "name".to_string(),
                relation: "reported_by".to_string(),
            },
        ]
    }

    #[test]
    fn test_find_relationships_forward() {
        let engine = GraphEngine::from_relationships(test_relationships());
        let rels = engine.find_relationships("project_tasks", "forward", None);
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].relation, "assigned_to");
    }

    #[test]
    fn test_find_relationships_reverse() {
        let engine = GraphEngine::from_relationships(test_relationships());
        let rels = engine.find_relationships("team_members", "reverse", None);
        assert_eq!(rels.len(), 2);
    }

    #[test]
    fn test_find_relationships_both() {
        let engine = GraphEngine::from_relationships(test_relationships());
        let rels = engine.find_relationships("team_members", "both", None);
        assert_eq!(rels.len(), 2);
    }

    #[test]
    fn test_find_relationships_with_type_filter() {
        let engine = GraphEngine::from_relationships(test_relationships());
        let types = vec!["assigned_to".to_string()];
        let rels = engine.find_relationships("team_members", "both", Some(&types));
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].relation, "assigned_to");
    }

    #[test]
    fn test_find_relationships_no_match() {
        let engine = GraphEngine::from_relationships(test_relationships());
        let rels = engine.find_relationships("nonexistent_table", "forward", None);
        assert!(rels.is_empty());
    }

    // ---- Validation tests ----

    fn setup_demo_router() -> (QueryRouter, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        crate::demo::generate(tmp.path()).unwrap();

        let router = QueryRouter::new().unwrap();
        let tables_dir = tmp.path().join("tables");
        let sym_path = tables_dir.join("sym");

        for table_name in &["team_members", "project_tasks", "incidents"] {
            let table_dir = tables_dir.join(table_name);
            router
                .load_splayed(table_name, &table_dir, Some(&sym_path))
                .unwrap();
        }
        (router, tmp)
    }

    fn demo_table_columns() -> HashMap<String, Vec<String>> {
        let mut cols = HashMap::new();
        cols.insert(
            "team_members".to_string(),
            vec!["id", "name", "role", "department", "start_date"]
                .into_iter()
                .map(String::from)
                .collect(),
        );
        cols.insert(
            "project_tasks".to_string(),
            vec![
                "id",
                "title",
                "assignee",
                "status",
                "priority",
                "project",
                "created_at",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
        );
        cols.insert(
            "incidents".to_string(),
            vec![
                "id",
                "title",
                "severity",
                "reporter",
                "resolved",
                "duration_min",
                "created_at",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
        );
        cols
    }

    fn demo_engine() -> GraphEngine {
        GraphEngine::from_relationships_with_columns(test_relationships(), demo_table_columns())
    }

    #[test]
    fn test_neighbors_invalid_table() {
        let engine = GraphEngine::from_relationships(test_relationships());
        let (router, _tmp) = setup_demo_router();
        let result = engine.neighbors("bad table", "name", "Alice", 2, "both", None, &router);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("invalid table name"));
    }

    #[test]
    fn test_neighbors_invalid_column() {
        let engine = GraphEngine::from_relationships(test_relationships());
        let (router, _tmp) = setup_demo_router();
        let result = engine.neighbors("team_members", "bad col", "Alice", 2, "both", None, &router);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("invalid column name"));
    }

    #[test]
    fn test_neighbors_invalid_direction() {
        let engine = GraphEngine::from_relationships(test_relationships());
        let (router, _tmp) = setup_demo_router();
        let result = engine.neighbors(
            "team_members",
            "name",
            "Alice",
            2,
            "backwards",
            None,
            &router,
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("invalid direction"));
    }

    #[test]
    fn test_path_invalid_direction() {
        let engine = GraphEngine::from_relationships(test_relationships());
        let (router, _tmp) = setup_demo_router();
        let result = engine.path(
            "team_members",
            "name",
            "Alice Chen",
            "incidents",
            "reporter",
            "Alice Chen",
            5,
            "backwards",
            None,
            &router,
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("invalid direction"));
    }

    // ---- Integration tests with demo data ----

    #[test]
    fn test_neighbors_depth_1() {
        let engine = demo_engine();
        let (router, _tmp) = setup_demo_router();
        let result = engine
            .neighbors(
                "team_members",
                "name",
                "Alice Chen",
                1,
                "both",
                None,
                &router,
            )
            .unwrap();

        let nodes = result["nodes"].as_array().unwrap();
        let edges = result["edges"].as_array().unwrap();

        // Must include at least the starting node + some neighbors
        assert!(nodes.len() >= 2, "should find neighbors for Alice Chen");
        assert!(!edges.is_empty(), "should have edges");

        // Starting node should be present
        let has_alice = nodes.iter().any(|n| n["key"] == "Alice Chen");
        assert!(has_alice, "starting node should be in results");
    }

    #[test]
    fn test_neighbors_no_duplicate_edges() {
        let engine = demo_engine();
        let (router, _tmp) = setup_demo_router();
        let result = engine
            .neighbors(
                "team_members",
                "name",
                "Alice Chen",
                2,
                "both",
                None,
                &router,
            )
            .unwrap();

        let edges = result["edges"].as_array().unwrap();

        // Check for duplicate edges: same (from_table, from_key_col, from_key, to_table, to_key_col, to_key, relation)
        let mut seen = std::collections::HashSet::new();
        for edge in edges {
            let key = format!(
                "{}.{}={}->{}:{}={}:{}",
                edge["from_table"],
                edge["from_key_col"],
                edge["from_key"],
                edge["to_table"],
                edge["to_key_col"],
                edge["to_key"],
                edge["relation"]
            );
            assert!(seen.insert(key.clone()), "duplicate edge found: {key}");
        }
    }

    #[test]
    fn test_neighbors_nonexistent_start() {
        let engine = GraphEngine::from_relationships(test_relationships());
        let (router, _tmp) = setup_demo_router();
        let result = engine.neighbors(
            "team_members",
            "name",
            "Nonexistent Person",
            1,
            "both",
            None,
            &router,
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("starting node not found"));
    }

    #[test]
    fn test_neighbors_depth_clamped_to_max() {
        let engine = demo_engine();
        let (router, _tmp) = setup_demo_router();
        // depth=100 should be clamped to MAX_DEPTH (10) without error
        let result = engine.neighbors(
            "team_members",
            "name",
            "Alice Chen",
            100,
            "both",
            None,
            &router,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_path_found() {
        let engine = demo_engine();
        let (router, _tmp) = setup_demo_router();

        // Find a task assigned to Alice Chen via SQL to get a valid path target
        let result = router
            .query_sync("SELECT title FROM project_tasks WHERE assignee = 'Alice Chen' LIMIT 1")
            .unwrap();
        if result.rows.is_empty() {
            // Alice has no tasks in this random seed - skip
            return;
        }
        let task_title = match &result.rows[0][0] {
            Value::String(s) => s.clone(),
            _ => return,
        };

        // Path from task to Alice via assignee FK
        let result = engine
            .path(
                "project_tasks",
                "title",
                &task_title,
                "team_members",
                "name",
                "Alice Chen",
                5,
                "both",
                None,
                &router,
            )
            .unwrap();

        assert_eq!(result["found"], true);
        let path = result["path"].as_array().unwrap();
        assert!(path.len() >= 2, "path should have at least 2 nodes");
        assert_eq!(result["hops"], 1);
    }

    #[test]
    fn test_path_target_not_found() {
        let engine = GraphEngine::from_relationships(test_relationships());
        let (router, _tmp) = setup_demo_router();
        // Search for a path to a nonexistent target - should error at pre-resolve
        let result = engine.path(
            "team_members",
            "name",
            "Alice Chen",
            "team_members",
            "name",
            "Nonexistent Person",
            5,
            "both",
            None,
            &router,
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("target node not found"));
    }

    #[test]
    fn test_path_source_not_found() {
        let engine = GraphEngine::from_relationships(test_relationships());
        let (router, _tmp) = setup_demo_router();
        let result = engine.path(
            "team_members",
            "name",
            "Nonexistent Person",
            "team_members",
            "name",
            "Alice Chen",
            5,
            "both",
            None,
            &router,
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("source node not found"));
    }

    #[test]
    fn test_path_no_route() {
        // Test the BFS "no path found" code path (found: false)
        // Use depth=0 so BFS never expands, guaranteeing no path
        let engine = demo_engine();
        let (router, _tmp) = setup_demo_router();

        // Find a task assigned to Alice to use as target
        let result = router
            .query_sync("SELECT title FROM project_tasks WHERE assignee = 'Alice Chen' LIMIT 1")
            .unwrap();
        if result.rows.is_empty() {
            return;
        }
        let task_title = match &result.rows[0][0] {
            Value::String(s) => s.clone(),
            _ => return,
        };

        let result = engine
            .path(
                "team_members",
                "name",
                "Alice Chen",
                "project_tasks",
                "title",
                &task_title,
                0, // depth=0: BFS won't expand, so no path
                "both",
                None,
                &router,
            )
            .unwrap();

        assert_eq!(result["found"], false);
        assert!(result["message"].as_str().unwrap().contains("no path"));
    }

    #[test]
    fn test_path_self() {
        // Path from a node to itself should return a trivial 0-hop path
        let engine = demo_engine();
        let (router, _tmp) = setup_demo_router();
        let result = engine
            .path(
                "team_members",
                "name",
                "Alice Chen",
                "team_members",
                "name",
                "Alice Chen",
                5,
                "both",
                None,
                &router,
            )
            .unwrap();

        assert_eq!(result["found"], true);
        let path = result["path"].as_array().unwrap();
        assert_eq!(path.len(), 1);
        assert_eq!(result["hops"], 0);
        assert_eq!(path[0]["table"], "team_members");
        assert_eq!(path[0]["key_col"], "name");
        assert_eq!(path[0]["key"], "Alice Chen");
    }

    #[test]
    fn test_reverse_traversal_no_node_collapse() {
        // Regression: reverse traversal from team_members to project_tasks must
        // produce distinct nodes for each task, not collapse them by FK value.
        let engine = demo_engine();
        let (router, _tmp) = setup_demo_router();

        // Find any team member with 2+ tasks (avoids dependence on random assignment)
        let count_result = router.query_sync(
            "SELECT assignee FROM project_tasks GROUP BY assignee HAVING count(*) >= 2 LIMIT 1",
        );

        // If the SQL engine doesn't support GROUP BY/HAVING, fall back to scanning all members
        let member_name = if let Ok(ref res) = count_result {
            if let Some(row) = res.rows.first() {
                match &row[0] {
                    Value::String(s) => Some(s.clone()),
                    _ => None,
                }
            } else {
                None
            }
        } else {
            None
        };

        // Fallback: try each known member until we find one with 2+ tasks
        let member_name = member_name.unwrap_or_else(|| {
            let members = [
                "Alice Chen",
                "Bob Martinez",
                "Carol Wu",
                "Dave Johnson",
                "Eve Park",
                "Frank Liu",
                "Grace Kim",
                "Hank Wilson",
            ];
            for name in &members {
                let escaped = escape_sql_value(name);
                let res = router
                    .query_sync(&format!(
                        "SELECT id FROM project_tasks WHERE assignee = '{escaped}'"
                    ))
                    .unwrap();
                if res.rows.len() >= 2 {
                    return name.to_string();
                }
            }
            panic!(
                "no team member has 2+ tasks — demo RNG produced degenerate data; \
                 with 20 tasks and 8 assignees this should be extremely rare"
            );
        });

        let escaped = escape_sql_value(&member_name);
        let task_result = router
            .query_sync(&format!(
                "SELECT id FROM project_tasks WHERE assignee = '{escaped}'"
            ))
            .unwrap();
        let expected_task_count = task_result.rows.len();
        assert!(expected_task_count >= 2);

        let result = engine
            .neighbors(
                "team_members",
                "name",
                &member_name,
                1,
                "both",
                Some(&["assigned_to".to_string()]),
                &router,
            )
            .unwrap();

        let nodes = result["nodes"].as_array().unwrap();
        let task_nodes: Vec<_> = nodes
            .iter()
            .filter(|n| n["table"] == "project_tasks")
            .collect();

        assert_eq!(
            task_nodes.len(),
            expected_task_count,
            "each task should be a distinct node, got {} but expected {} for member {}",
            task_nodes.len(),
            expected_task_count,
            member_name,
        );
    }

    #[test]
    fn test_is_valid_identifier_digit_start() {
        // Identifiers starting with digits should be rejected
        assert!(!is_valid_identifier("1table"));
        assert!(!is_valid_identifier("123"));
        // But digits after the first character are fine
        assert!(is_valid_identifier("table1"));
        assert!(is_valid_identifier("_1"));
    }
}
