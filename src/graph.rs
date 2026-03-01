use std::collections::HashMap;

use anyhow::{bail, Result};
use serde_json::json;

use crate::catalog::{Catalog, Relationship};
use crate::connector::Value;
use crate::router::QueryRouter;

/// Escape a string value for use in SQL single-quoted literals.
/// Replaces `'` with `''` to prevent SQL injection.
fn escape_sql_value(s: &str) -> String {
    s.replace('\'', "''")
}

/// Describes one edge in a graph traversal result.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Edge {
    pub from_table: String,
    pub from_key: String,
    pub to_table: String,
    pub to_key: String,
    pub relation: String,
}

/// Parent info for BFS path reconstruction: ((parent_table, parent_key), relation_name).
type PathParent = Option<((String, String), String)>;

/// SQL-based graph traversal engine over catalog FK relationships.
///
/// Performs BFS traversal using SQL queries at each hop, resolving
/// neighbors via FK relationships registered in the catalog.
pub struct GraphEngine {
    /// Catalog snapshot for relationship lookups.
    relationships: Vec<Relationship>,
}

impl GraphEngine {
    /// Build a GraphEngine from the catalog's registered relationships.
    pub fn build_from_catalog(catalog: &Catalog) -> Self {
        GraphEngine {
            relationships: catalog.relationships().to_vec(),
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
        let mut visited: HashMap<(String, String), serde_json::Value> = HashMap::new();
        let mut edges: Vec<Edge> = Vec::new();
        let mut frontier: Vec<(String, String, String)> = vec![(
            table.to_string(),
            key_col.to_string(),
            key_value.to_string(),
        )];

        // Fetch and store the starting node
        if let Ok(props) = self.fetch_node_properties(table, key_col, key_value, router) {
            visited.insert((table.to_string(), key_value.to_string()), props);
        }

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

                    for nval in &neighbor_values {
                        // Add edge
                        if is_forward {
                            edges.push(Edge {
                                from_table: tbl.clone(),
                                from_key: kval.clone(),
                                to_table: neighbor_table.clone(),
                                to_key: nval.clone(),
                                relation: rel.relation.clone(),
                            });
                        } else {
                            edges.push(Edge {
                                from_table: neighbor_table.clone(),
                                from_key: nval.clone(),
                                to_table: tbl.clone(),
                                to_key: kval.clone(),
                                relation: rel.relation.clone(),
                            });
                        }

                        // Visit neighbor if not already seen
                        let key = (neighbor_table.clone(), nval.clone());
                        if let std::collections::hash_map::Entry::Vacant(e) = visited.entry(key) {
                            let id_col = neighbor_col.to_string();
                            if let Ok(props) =
                                self.fetch_node_properties(neighbor_table, &id_col, nval, router)
                            {
                                e.insert(props);
                                next_frontier.push((neighbor_table.clone(), id_col, nval.clone()));
                            }
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
            .map(|((tbl, key), props)| {
                json!({
                    "table": tbl,
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
        _to_key_col: &str,
        to_key: &str,
        max_depth: usize,
        router: &QueryRouter,
    ) -> Result<serde_json::Value> {
        // BFS from source to destination
        let mut visited: HashMap<(String, String), PathParent> = HashMap::new();
        visited.insert((from_table.to_string(), from_key.to_string()), None);

        let mut frontier: Vec<(String, String, String)> = vec![(
            from_table.to_string(),
            from_key_col.to_string(),
            from_key.to_string(),
        )];

        let target = (to_table.to_string(), to_key.to_string());
        let mut found = false;

        for _d in 0..max_depth {
            let mut next_frontier = Vec::new();

            for (tbl, kcol, kval) in &frontier {
                let rels = self.find_relationships(tbl, "both", None);

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

                    for nval in &neighbor_values {
                        let key = (neighbor_table.clone(), nval.clone());
                        if !visited.contains_key(&key) {
                            visited.insert(
                                key.clone(),
                                Some(((tbl.clone(), kval.clone()), rel.relation.clone())),
                            );
                            let id_col = neighbor_col.to_string();
                            next_frontier.push((neighbor_table.clone(), id_col, nval.clone()));

                            if key == target {
                                found = true;
                            }
                        }
                    }
                }
            }

            if found || next_frontier.is_empty() {
                break;
            }
            frontier = next_frontier;
        }

        if !found {
            return Ok(json!({
                "found": false,
                "message": format!(
                    "no path from {from_table}.{from_key} to {to_table}.{to_key} within {max_depth} hops"
                ),
            }));
        }

        // Reconstruct path
        let mut path = Vec::new();
        let mut current = target;
        while let Some(Some((parent, relation))) = visited.get(&current) {
            path.push(json!({
                "table": current.0,
                "key": current.1,
                "via_relation": relation,
            }));
            current = parent.clone();
        }
        // Add source node
        path.push(json!({
            "table": current.0,
            "key": current.1,
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

    /// Resolve neighbor values via SQL.
    ///
    /// If `is_forward`: we have a row in `source_table` where `key_col=key_value`,
    /// and `source_col` is the FK column. We need to find matching rows in
    /// `neighbor_table` where `neighbor_col` matches the FK value.
    ///
    /// If `!is_forward` (reverse): we look in `neighbor_table` for rows whose
    /// `neighbor_col` (FK) value matches our `key_value`.
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
    ) -> Result<Vec<String>> {
        let escaped_key = escape_sql_value(key_value);
        if is_forward {
            // Forward: get FK value from source, then find matching rows in neighbor
            let sql = format!(
                "SELECT {source_col} FROM {source_table} WHERE {key_col} = '{escaped_key}'"
            );
            let result = router.query_sync(&sql)?;
            let mut neighbors = Vec::new();
            for row in &result.rows {
                if let Some(Value::String(fk_val)) = row.first() {
                    // The FK value IS the key in the neighbor table
                    neighbors.push(fk_val.clone());
                }
            }
            Ok(neighbors)
        } else {
            // Reverse: find rows in neighbor_table where neighbor_col (FK) = key_value
            let sql = format!(
                "SELECT {neighbor_col} FROM {neighbor_table} WHERE {neighbor_col} = '{escaped_key}'"
            );
            let result = router.query_sync(&sql)?;
            Ok(result
                .rows
                .into_iter()
                .filter_map(|row| match row.into_iter().next() {
                    Some(Value::String(s)) => Some(s),
                    Some(Value::Int(i)) => Some(i.to_string()),
                    _ => None,
                })
                .collect())
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
            let val = match &row[i] {
                Value::String(s) => json!(s),
                Value::Int(i) => json!(i),
                Value::Float(f) => json!(f),
                Value::Bool(b) => json!(b),
                Value::Null => serde_json::Value::Null,
            };
            props.insert(col.name.clone(), val);
        }

        Ok(serde_json::Value::Object(props))
    }
}
