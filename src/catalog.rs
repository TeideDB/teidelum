use std::collections::HashSet;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

/// Where a table's data is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageType {
    Local,
    Remote,
}

/// Metadata about a registered table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableEntry {
    pub name: String,
    pub source: String,
    pub storage: StorageType,
    pub columns: Vec<ColumnInfo>,
    pub row_count: Option<u64>,
}

/// Column name and type string.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub dtype: String,
}

/// A foreign key relationship between tables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub from_table: String,
    pub from_col: String,
    pub to_table: String,
    pub to_col: String,
    pub relation: String,
}

/// Validate that a string is a safe SQL identifier.
/// Must start with a letter or underscore, then alphanumeric or underscores.
pub fn is_valid_identifier(s: &str) -> bool {
    match s.chars().next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {
            s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        }
        _ => false,
    }
}

/// The metadata catalog tracks all available tables, their schemas,
/// storage type (local vs remote), and foreign key relationships.
///
/// The query router uses the catalog to decide whether to dispatch
/// a query to libteide (local) or a connector (remote).
#[derive(Clone)]
pub struct Catalog {
    tables: Vec<TableEntry>,
    relationships: Vec<Relationship>,
}

impl Catalog {
    pub fn new() -> Self {
        Self {
            tables: Vec::new(),
            relationships: Vec::new(),
        }
    }

    pub fn register_table(&mut self, entry: TableEntry) {
        self.tables.retain(|t| t.name != entry.name);
        self.tables.push(entry);
    }

    /// Validate a batch of relationships against the current catalog state
    /// without mutating anything. Checks identifier validity and graph-name
    /// collisions both against existing relationships and within the batch.
    pub fn validate_relationships(&self, rels: &[Relationship]) -> Result<()> {
        for (i, rel) in rels.iter().enumerate() {
            for (label, val) in [
                ("from_table", &rel.from_table),
                ("from_col", &rel.from_col),
                ("to_table", &rel.to_table),
                ("to_col", &rel.to_col),
                ("relation", &rel.relation),
            ] {
                if !is_valid_identifier(val) {
                    bail!("invalid identifier in relationship {label}: '{val}'");
                }
            }

            // Check if this exact relationship already exists (would be a no-op skip)
            let is_exact_dup = self.relationships.iter().any(|r| {
                r.from_table == rel.from_table
                    && r.from_col == rel.from_col
                    && r.to_table == rel.to_table
                    && r.to_col == rel.to_col
                    && r.relation == rel.relation
            });
            if is_exact_dup {
                continue;
            }

            // Check graph-name collision against existing catalog relationships
            let name_collision = self.relationships.iter().any(|r| {
                r.from_table == rel.from_table
                    && r.to_table == rel.to_table
                    && r.relation == rel.relation
            });
            if name_collision {
                bail!(
                    "relationship '{}' between {}.{} -> {}.{} conflicts with an existing \
                     relationship using the same (from_table, to_table, relation) triple \
                     but different columns",
                    rel.relation,
                    rel.from_table,
                    rel.from_col,
                    rel.to_table,
                    rel.to_col,
                );
            }

            // Check graph-name collision within the batch itself
            for earlier in &rels[..i] {
                let same_graph_name = earlier.from_table == rel.from_table
                    && earlier.to_table == rel.to_table
                    && earlier.relation == rel.relation;
                let same_cols = earlier.from_col == rel.from_col && earlier.to_col == rel.to_col;
                if same_graph_name && !same_cols {
                    bail!(
                        "relationship '{}' between {}.{} -> {}.{} conflicts with another \
                         relationship in the same batch using the same (from_table, to_table, \
                         relation) triple but different columns",
                        rel.relation,
                        rel.from_table,
                        rel.from_col,
                        rel.to_table,
                        rel.to_col,
                    );
                }
            }
        }
        Ok(())
    }

    pub fn register_relationship(&mut self, rel: Relationship) -> Result<()> {
        for (label, val) in [
            ("from_table", &rel.from_table),
            ("from_col", &rel.from_col),
            ("to_table", &rel.to_table),
            ("to_col", &rel.to_col),
            ("relation", &rel.relation),
        ] {
            if !is_valid_identifier(val) {
                bail!("invalid identifier in relationship {label}: '{val}'");
            }
        }
        // Deduplicate: skip if an identical relationship already exists
        let exists = self.relationships.iter().any(|r| {
            r.from_table == rel.from_table
                && r.from_col == rel.from_col
                && r.to_table == rel.to_table
                && r.to_col == rel.to_col
                && r.relation == rel.relation
        });
        if !exists {
            // Reject relationships that would collide on property graph name
            // (same from_table, to_table, relation) but differ on columns,
            // since graph names are derived from those three fields only.
            let name_collision = self.relationships.iter().any(|r| {
                r.from_table == rel.from_table
                    && r.to_table == rel.to_table
                    && r.relation == rel.relation
            });
            if name_collision {
                bail!(
                    "relationship '{}' between {}.{} -> {}.{} conflicts with an existing \
                     relationship using the same (from_table, to_table, relation) triple \
                     but different columns",
                    rel.relation,
                    rel.from_table,
                    rel.from_col,
                    rel.to_table,
                    rel.to_col,
                );
            }
            self.relationships.push(rel);
        }
        Ok(())
    }

    /// Remove a table and any relationships referencing it. Returns true if the table existed.
    pub fn remove_table(&mut self, name: &str) -> bool {
        let before = self.tables.len();
        self.tables.retain(|t| t.name != name);
        if self.tables.len() == before {
            return false;
        }
        self.relationships
            .retain(|r| r.from_table != name && r.to_table != name);
        true
    }

    /// Remove a specific relationship from the catalog.
    pub fn remove_relationship(&mut self, rel: &Relationship) {
        self.relationships.retain(|r| {
            !(r.from_table == rel.from_table
                && r.from_col == rel.from_col
                && r.to_table == rel.to_table
                && r.to_col == rel.to_col
                && r.relation == rel.relation)
        });
    }

    pub fn lookup_table(&self, name: &str) -> Option<&TableEntry> {
        self.tables.iter().find(|t| t.name == name)
    }

    pub fn tables(&self) -> &[TableEntry] {
        &self.tables
    }

    pub fn relationships(&self) -> &[Relationship] {
        &self.relationships
    }

    /// Filter tables by source name.
    pub fn tables_by_source(&self, source: &str) -> Vec<&TableEntry> {
        self.tables.iter().filter(|t| t.source == source).collect()
    }

    /// Produce a JSON description of the catalog for the `describe` MCP tool.
    ///
    /// When `created_graphs` is provided, only graphs whose names appear in the
    /// set are advertised. This prevents reporting graphs whose DDL creation
    /// failed. Pass `None` to advertise all graphs that pass the locality check
    /// (used by catalog-only tests that don't execute DDL).
    pub fn describe(
        &self,
        source_filter: Option<&str>,
        created_graphs: Option<&HashSet<String>>,
    ) -> Result<serde_json::Value> {
        let tables: Vec<_> = match source_filter {
            Some(src) => self.tables_by_source(src),
            None => self.tables.iter().collect(),
        };

        let rels: Vec<_> = match source_filter {
            Some(_) => self
                .relationships
                .iter()
                .filter(|r| {
                    tables
                        .iter()
                        .any(|t| t.name == r.from_table || t.name == r.to_table)
                })
                .collect(),
            None => self.relationships.iter().collect(),
        };

        // Only advertise property graphs for relationships where both
        // tables are locally stored — remote/catalog-only tables cannot
        // participate in SQL property graphs.  Check locality against all
        // tables (not the source-filtered list) so cross-source local
        // relationships are correctly reported.
        // Collect relationship-based graphs with full metadata.
        let mut rel_graph_names = std::collections::HashSet::new();
        let mut property_graphs: Vec<serde_json::Value> = rels
            .iter()
            .filter(|r| {
                let graph_name = format!("pg_{}_{}_{}", r.from_table, r.to_table, r.relation);
                // If we have a created_graphs set, only include graphs that
                // were actually created. Otherwise fall back to the locality
                // heuristic (for catalog-only tests without DDL execution).
                if let Some(created) = created_graphs {
                    return created.contains(&graph_name);
                }
                let from_local = self
                    .tables
                    .iter()
                    .any(|t| t.name == r.from_table && t.storage == StorageType::Local);
                let to_local = self
                    .tables
                    .iter()
                    .any(|t| t.name == r.to_table && t.storage == StorageType::Local);
                from_local && to_local
            })
            .map(|r| {
                let graph_name = format!("pg_{}_{}_{}", r.from_table, r.to_table, r.relation);
                rel_graph_names.insert(graph_name.clone());
                let vertex_tables: Vec<&str> = if r.from_table == r.to_table {
                    vec![&r.from_table]
                } else {
                    vec![&r.from_table, &r.to_table]
                };
                serde_json::json!({
                    "name": graph_name,
                    "vertex_tables": vertex_tables,
                    "edge_table": r.from_table,
                    "edge_label": r.relation,
                    "fk_column": r.from_col,
                    "referenced_column": r.to_col,
                })
            })
            .collect();

        // Include custom graphs (created via direct DDL, not from catalog
        // relationships) so they are discoverable through describe().
        // Skip when a source filter is active — custom graphs have no source
        // metadata, so including them unconditionally would leak unrelated graphs.
        if source_filter.is_none() {
            if let Some(created) = created_graphs {
                for name in created {
                    if !rel_graph_names.contains(name) {
                        property_graphs.push(serde_json::json!({
                            "name": name,
                            "custom": true,
                        }));
                    }
                }
            }
        }

        Ok(serde_json::json!({
            "tables": tables,
            "relationships": rels,
            "property_graphs": property_graphs,
        }))
    }
}

impl Default for Catalog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_table() {
        let mut catalog = Catalog::new();
        catalog.register_table(TableEntry {
            name: "users".to_string(),
            source: "test".to_string(),
            storage: StorageType::Local,
            columns: vec![ColumnInfo {
                name: "id".to_string(),
                dtype: "i64".to_string(),
            }],
            row_count: Some(10),
        });
        catalog.register_table(TableEntry {
            name: "orders".to_string(),
            source: "test".to_string(),
            storage: StorageType::Local,
            columns: vec![ColumnInfo {
                name: "id".to_string(),
                dtype: "i64".to_string(),
            }],
            row_count: Some(5),
        });
        catalog
            .register_relationship(Relationship {
                from_table: "orders".to_string(),
                from_col: "user_id".to_string(),
                to_table: "users".to_string(),
                to_col: "id".to_string(),
                relation: "belongs_to".to_string(),
            })
            .unwrap();

        assert!(catalog.remove_table("users"));

        // Table gone
        assert!(catalog.lookup_table("users").is_none());
        assert_eq!(catalog.tables().len(), 1);
        // Relationships referencing "users" also removed
        assert!(catalog.relationships().is_empty());
    }

    #[test]
    fn test_remove_table_nonexistent() {
        let mut catalog = Catalog::new();
        assert!(!catalog.remove_table("ghost"));
    }

    #[test]
    fn test_register_and_lookup() {
        let mut catalog = Catalog::new();
        catalog.register_table(TableEntry {
            name: "users".to_string(),
            source: "test".to_string(),
            storage: StorageType::Local,
            columns: vec![ColumnInfo {
                name: "id".to_string(),
                dtype: "i64".to_string(),
            }],
            row_count: Some(10),
        });

        let entry = catalog.lookup_table("users").unwrap();
        assert_eq!(entry.name, "users");
        assert_eq!(entry.source, "test");
        assert_eq!(entry.columns.len(), 1);
        assert_eq!(entry.row_count, Some(10));
    }

    #[test]
    fn test_register_replaces_existing() {
        let mut catalog = Catalog::new();
        catalog.register_table(TableEntry {
            name: "users".to_string(),
            source: "old".to_string(),
            storage: StorageType::Local,
            columns: vec![],
            row_count: Some(5),
        });
        catalog.register_table(TableEntry {
            name: "users".to_string(),
            source: "new".to_string(),
            storage: StorageType::Local,
            columns: vec![ColumnInfo {
                name: "id".to_string(),
                dtype: "i64".to_string(),
            }],
            row_count: Some(20),
        });

        assert_eq!(catalog.tables().len(), 1);
        let entry = catalog.lookup_table("users").unwrap();
        assert_eq!(entry.source, "new");
        assert_eq!(entry.row_count, Some(20));
    }

    #[test]
    fn test_lookup_nonexistent() {
        let catalog = Catalog::new();
        assert!(catalog.lookup_table("ghost").is_none());
    }

    #[test]
    fn test_tables_by_source() {
        let mut catalog = Catalog::new();
        catalog.register_table(TableEntry {
            name: "a".to_string(),
            source: "notion".to_string(),
            storage: StorageType::Local,
            columns: vec![],
            row_count: None,
        });
        catalog.register_table(TableEntry {
            name: "b".to_string(),
            source: "zulip".to_string(),
            storage: StorageType::Local,
            columns: vec![],
            row_count: None,
        });
        catalog.register_table(TableEntry {
            name: "c".to_string(),
            source: "notion".to_string(),
            storage: StorageType::Local,
            columns: vec![],
            row_count: None,
        });

        let notion = catalog.tables_by_source("notion");
        assert_eq!(notion.len(), 2);
        assert!(notion.iter().all(|t| t.source == "notion"));
    }

    #[test]
    fn test_tables_by_source_empty() {
        let catalog = Catalog::new();
        assert!(catalog.tables_by_source("ghost").is_empty());
    }

    #[test]
    fn test_describe_json_structure() {
        let mut catalog = Catalog::new();
        catalog.register_table(TableEntry {
            name: "t".to_string(),
            source: "s".to_string(),
            storage: StorageType::Local,
            columns: vec![],
            row_count: None,
        });

        let desc = catalog.describe(None, None).unwrap();
        assert!(desc["tables"].is_array());
        assert!(desc["relationships"].is_array());
        assert_eq!(desc["tables"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_describe_includes_columns() {
        let mut catalog = Catalog::new();
        catalog.register_table(TableEntry {
            name: "t".to_string(),
            source: "s".to_string(),
            storage: StorageType::Local,
            columns: vec![
                ColumnInfo {
                    name: "id".to_string(),
                    dtype: "i64".to_string(),
                },
                ColumnInfo {
                    name: "name".to_string(),
                    dtype: "string".to_string(),
                },
            ],
            row_count: Some(42),
        });

        let desc = catalog.describe(None, None).unwrap();
        let table = &desc["tables"][0];
        let cols = table["columns"].as_array().unwrap();
        assert_eq!(cols.len(), 2);
        assert_eq!(cols[0]["name"], "id");
        assert_eq!(cols[0]["dtype"], "i64");
        assert_eq!(table["row_count"], 42);
    }

    #[test]
    fn test_describe_source_filter_relationships() {
        let mut catalog = Catalog::new();
        catalog.register_table(TableEntry {
            name: "users".to_string(),
            source: "notion".to_string(),
            storage: StorageType::Local,
            columns: vec![],
            row_count: None,
        });
        catalog.register_table(TableEntry {
            name: "tasks".to_string(),
            source: "zulip".to_string(),
            storage: StorageType::Local,
            columns: vec![],
            row_count: None,
        });
        catalog
            .register_relationship(Relationship {
                from_table: "tasks".to_string(),
                from_col: "owner".to_string(),
                to_table: "users".to_string(),
                to_col: "name".to_string(),
                relation: "owned_by".to_string(),
            })
            .unwrap();

        // Filter by "zulip" — should see tasks table and its relationship
        let desc = catalog.describe(Some("zulip"), None).unwrap();
        let tables = desc["tables"].as_array().unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0]["name"], "tasks");
        let rels = desc["relationships"].as_array().unwrap();
        assert_eq!(rels.len(), 1);

        // Filter by nonexistent — no tables, no rels
        let desc2 = catalog.describe(Some("ghost"), None).unwrap();
        assert!(desc2["tables"].as_array().unwrap().is_empty());
        assert!(desc2["relationships"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_register_relationship_valid() {
        let mut catalog = Catalog::new();
        catalog
            .register_relationship(Relationship {
                from_table: "tasks".to_string(),
                from_col: "owner".to_string(),
                to_table: "people".to_string(),
                to_col: "name".to_string(),
                relation: "owned_by".to_string(),
            })
            .unwrap();
        assert_eq!(catalog.relationships().len(), 1);
    }

    #[test]
    fn test_register_relationship_invalid_identifier() {
        let mut catalog = Catalog::new();

        let cases = vec![
            ("bad table!", "col", "t2", "col", "rel"),
            ("t1", "bad col!", "t2", "col", "rel"),
            ("t1", "col", "bad table!", "col", "rel"),
            ("t1", "col", "t2", "bad col!", "rel"),
            ("t1", "col", "t2", "col", "bad rel!"),
        ];
        for (ft, fc, tt, tc, r) in cases {
            let result = catalog.register_relationship(Relationship {
                from_table: ft.to_string(),
                from_col: fc.to_string(),
                to_table: tt.to_string(),
                to_col: tc.to_string(),
                relation: r.to_string(),
            });
            assert!(
                result.is_err(),
                "should reject invalid identifier in ({ft}, {fc}, {tt}, {tc}, {r})"
            );
        }
        assert!(catalog.relationships().is_empty());
    }

    #[test]
    fn test_is_valid_identifier_comprehensive() {
        // Valid
        assert!(is_valid_identifier("a"));
        assert!(is_valid_identifier("_a"));
        assert!(is_valid_identifier("table_name"));
        assert!(is_valid_identifier("col1"));
        assert!(is_valid_identifier("ABC"));
        assert!(is_valid_identifier("_"));

        // Invalid
        assert!(!is_valid_identifier(""));
        assert!(!is_valid_identifier("1col"));
        assert!(!is_valid_identifier("has space"));
        assert!(!is_valid_identifier("dot.name"));
        assert!(!is_valid_identifier("dash-name"));
        assert!(!is_valid_identifier("semi;colon"));
        assert!(!is_valid_identifier("'quoted'"));
    }

    #[test]
    fn test_remove_table_cascades_multiple_relationships() {
        let mut catalog = Catalog::new();
        catalog.register_table(TableEntry {
            name: "a".to_string(),
            source: "t".to_string(),
            storage: StorageType::Local,
            columns: vec![],
            row_count: None,
        });
        catalog.register_table(TableEntry {
            name: "b".to_string(),
            source: "t".to_string(),
            storage: StorageType::Local,
            columns: vec![],
            row_count: None,
        });
        catalog.register_table(TableEntry {
            name: "c".to_string(),
            source: "t".to_string(),
            storage: StorageType::Local,
            columns: vec![],
            row_count: None,
        });

        // a->b, c->a, b->c
        catalog
            .register_relationship(Relationship {
                from_table: "a".to_string(),
                from_col: "id".to_string(),
                to_table: "b".to_string(),
                to_col: "ref".to_string(),
                relation: "r1".to_string(),
            })
            .unwrap();
        catalog
            .register_relationship(Relationship {
                from_table: "c".to_string(),
                from_col: "id".to_string(),
                to_table: "a".to_string(),
                to_col: "ref".to_string(),
                relation: "r2".to_string(),
            })
            .unwrap();
        catalog
            .register_relationship(Relationship {
                from_table: "b".to_string(),
                from_col: "id".to_string(),
                to_table: "c".to_string(),
                to_col: "ref".to_string(),
                relation: "r3".to_string(),
            })
            .unwrap();

        // Remove "a" — should cascade r1 (a->b) and r2 (c->a), leave r3 (b->c)
        assert!(catalog.remove_table("a"));
        assert_eq!(catalog.tables().len(), 2);
        assert_eq!(catalog.relationships().len(), 1);
        assert_eq!(catalog.relationships()[0].relation, "r3");
    }

    #[test]
    fn test_describe_includes_property_graphs() {
        let mut catalog = Catalog::new();
        catalog.register_table(TableEntry {
            name: "users".to_string(),
            source: "test".to_string(),
            storage: StorageType::Local,
            columns: vec![ColumnInfo {
                name: "id".to_string(),
                dtype: "i64".to_string(),
            }],
            row_count: Some(10),
        });
        catalog.register_table(TableEntry {
            name: "tasks".to_string(),
            source: "test".to_string(),
            storage: StorageType::Local,
            columns: vec![
                ColumnInfo {
                    name: "id".to_string(),
                    dtype: "i64".to_string(),
                },
                ColumnInfo {
                    name: "user_id".to_string(),
                    dtype: "i64".to_string(),
                },
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

        let desc = catalog.describe(None, None).unwrap();
        let graphs = desc["property_graphs"].as_array().unwrap();
        assert_eq!(graphs.len(), 1);
        assert_eq!(graphs[0]["name"], "pg_tasks_users_assigned_to");
        assert!(graphs[0]["vertex_tables"].as_array().unwrap().len() >= 2);
        assert_eq!(graphs[0]["edge_label"], "assigned_to");
    }
}
