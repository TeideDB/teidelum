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

    pub fn register_relationship(&mut self, rel: Relationship) -> Result<()> {
        for (label, val) in [
            ("from_table", &rel.from_table),
            ("from_col", &rel.from_col),
            ("to_table", &rel.to_table),
            ("to_col", &rel.to_col),
            ("relation", &rel.relation),
        ] {
            let valid = match val.chars().next() {
                Some(c) if c.is_ascii_alphabetic() || c == '_' => {
                    val.chars().all(|c| c.is_alphanumeric() || c == '_')
                }
                _ => false,
            };
            if !valid {
                bail!("invalid identifier in relationship {label}: '{val}'");
            }
        }
        self.relationships.push(rel);
        Ok(())
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
    pub fn describe(&self, source_filter: Option<&str>) -> Result<serde_json::Value> {
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

        Ok(serde_json::json!({
            "tables": tables,
            "relationships": rels,
        }))
    }
}

impl Default for Catalog {
    fn default() -> Self {
        Self::new()
    }
}
