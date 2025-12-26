//! SQLite database implementation

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};

use super::models::{EdgeRecord, FileRecord, NodeRecord, ProjectRecord, ProjectStatus};

/// SQLite database wrapper
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open or create a database at the given path
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open database: {:?}", path))?;

        // Enable foreign keys
        conn.execute("PRAGMA foreign_keys = ON", [])?;

        Ok(Self { conn })
    }

    /// Open an in-memory database (for testing)
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute("PRAGMA foreign_keys = ON", [])?;
        Ok(Self { conn })
    }

    /// Initialize the database schema
    pub fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            -- Projects table
            CREATE TABLE IF NOT EXISTS projects (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                root_path TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            -- Files table
            CREATE TABLE IF NOT EXISTS files (
                id INTEGER PRIMARY KEY,
                project_id INTEGER NOT NULL,
                path TEXT NOT NULL,
                language TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                parsed_at TEXT NOT NULL,
                FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
                UNIQUE(project_id, path)
            );

            -- Nodes table (symbols)
            CREATE TABLE IF NOT EXISTS nodes (
                id INTEGER PRIMARY KEY,
                file_id INTEGER NOT NULL,
                node_type TEXT NOT NULL,
                name TEXT NOT NULL,
                qualified_name TEXT,
                start_line INTEGER NOT NULL,
                start_column INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                end_column INTEGER NOT NULL,
                attributes TEXT,
                FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
            );

            -- Edges table (relationships)
            CREATE TABLE IF NOT EXISTS edges (
                id INTEGER PRIMARY KEY,
                source_id INTEGER NOT NULL,
                target_id INTEGER NOT NULL,
                edge_type TEXT NOT NULL,
                attributes TEXT,
                FOREIGN KEY (source_id) REFERENCES nodes(id) ON DELETE CASCADE,
                FOREIGN KEY (target_id) REFERENCES nodes(id) ON DELETE CASCADE
            );

            -- Indexes
            CREATE INDEX IF NOT EXISTS idx_files_project ON files(project_id);
            CREATE INDEX IF NOT EXISTS idx_files_path ON files(path);
            CREATE INDEX IF NOT EXISTS idx_nodes_file ON nodes(file_id);
            CREATE INDEX IF NOT EXISTS idx_nodes_name ON nodes(name);
            CREATE INDEX IF NOT EXISTS idx_nodes_type ON nodes(node_type);
            CREATE INDEX IF NOT EXISTS idx_nodes_qualified ON nodes(qualified_name);
            CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source_id);
            CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target_id);
            CREATE INDEX IF NOT EXISTS idx_edges_type ON edges(edge_type);
            "#,
        )?;

        Ok(())
    }

    // ==================== Project Operations ====================

    /// Insert a new project
    pub fn insert_project(&self, project: &ProjectRecord) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO projects (name, root_path, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                project.name,
                project.root_path,
                project.created_at.to_rfc3339(),
                project.updated_at.to_rfc3339()
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get a project by its root path
    pub fn get_project_by_path(&self, root_path: &str) -> Result<Option<ProjectRecord>> {
        self.conn
            .query_row(
                "SELECT id, name, root_path, created_at, updated_at FROM projects WHERE root_path = ?1",
                params![root_path],
                |row| {
                    Ok(ProjectRecord {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        root_path: row.get(2)?,
                        created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                            .unwrap()
                            .with_timezone(&chrono::Utc),
                        updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                            .unwrap()
                            .with_timezone(&chrono::Utc),
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// Get a project by its name
    pub fn get_project_by_name(&self, name: &str) -> Result<Option<ProjectRecord>> {
        self.conn
            .query_row(
                "SELECT id, name, root_path, created_at, updated_at FROM projects WHERE name = ?1",
                params![name],
                |row| {
                    Ok(ProjectRecord {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        root_path: row.get(2)?,
                        created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                            .unwrap()
                            .with_timezone(&chrono::Utc),
                        updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                            .unwrap()
                            .with_timezone(&chrono::Utc),
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// List all projects
    pub fn list_projects(&self) -> Result<Vec<ProjectRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, root_path, created_at, updated_at FROM projects ORDER BY name"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ProjectRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                root_path: row.get(2)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                    .unwrap()
                    .with_timezone(&chrono::Utc),
                updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .unwrap()
                    .with_timezone(&chrono::Utc),
            })
        })?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// Update project timestamp
    pub fn update_project_timestamp(&self, project_id: i64) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE projects SET updated_at = ?1 WHERE id = ?2",
            params![now, project_id],
        )?;
        Ok(())
    }

    /// Get project status
    pub fn get_project_status(&self, project_id: i64) -> Result<Option<ProjectStatus>> {
        let project = self.conn.query_row(
            "SELECT id, name, root_path, created_at, updated_at FROM projects WHERE id = ?1",
            params![project_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(4)?,
                ))
            },
        ).optional()?;

        if let Some((id, name, root_path, updated_at)) = project {
            let files_count: u32 = self.conn.query_row(
                "SELECT COUNT(*) FROM files WHERE project_id = ?1",
                params![project_id],
                |row| row.get(0),
            )?;

            let nodes_count: u32 = self.conn.query_row(
                "SELECT COUNT(*) FROM nodes n JOIN files f ON n.file_id = f.id WHERE f.project_id = ?1",
                params![project_id],
                |row| row.get(0),
            )?;

            let edges_count: u32 = self.conn.query_row(
                "SELECT COUNT(*) FROM edges e JOIN nodes n ON e.source_id = n.id JOIN files f ON n.file_id = f.id WHERE f.project_id = ?1",
                params![project_id],
                |row| row.get(0),
            )?;

            Ok(Some(ProjectStatus {
                project_id: id,
                name,
                root_path,
                status: "ready".to_string(),
                files_parsed: files_count,
                nodes_count,
                edges_count,
                last_updated: chrono::DateTime::parse_from_rfc3339(&updated_at)
                    .unwrap()
                    .with_timezone(&chrono::Utc),
            }))
        } else {
            Ok(None)
        }
    }

    // ==================== File Operations ====================

    /// Insert a new file
    pub fn insert_file(&self, file: &FileRecord) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO files (project_id, path, language, content_hash, parsed_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                file.project_id,
                file.path,
                file.language,
                file.content_hash,
                file.parsed_at.to_rfc3339()
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get a file by project and path
    pub fn get_file_by_path(&self, project_id: i64, path: &str) -> Result<Option<FileRecord>> {
        self.conn
            .query_row(
                "SELECT id, project_id, path, language, content_hash, parsed_at FROM files WHERE project_id = ?1 AND path = ?2",
                params![project_id, path],
                |row| {
                    Ok(FileRecord {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        path: row.get(2)?,
                        language: row.get(3)?,
                        content_hash: row.get(4)?,
                        parsed_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                            .unwrap()
                            .with_timezone(&chrono::Utc),
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// Get a file by ID
    pub fn get_file(&self, file_id: i64) -> Result<Option<FileRecord>> {
        self.conn
            .query_row(
                "SELECT id, project_id, path, language, content_hash, parsed_at FROM files WHERE id = ?1",
                params![file_id],
                |row| {
                    Ok(FileRecord {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        path: row.get(2)?,
                        language: row.get(3)?,
                        content_hash: row.get(4)?,
                        parsed_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                            .unwrap()
                            .with_timezone(&chrono::Utc),
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// Delete all data for a file
    pub fn delete_file_data(&self, file_id: i64) -> Result<()> {
        // Edges will be deleted via CASCADE
        self.conn.execute(
            "DELETE FROM nodes WHERE file_id = ?1",
            params![file_id],
        )?;
        self.conn.execute(
            "DELETE FROM files WHERE id = ?1",
            params![file_id],
        )?;
        Ok(())
    }

    // ==================== Node Operations ====================

    /// Insert a new node
    pub fn insert_node(&self, node: &NodeRecord) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO nodes (file_id, node_type, name, qualified_name, start_line, start_column, end_line, end_column, attributes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                node.file_id,
                node.node_type,
                node.name,
                node.qualified_name,
                node.start_line,
                node.start_column,
                node.end_line,
                node.end_column,
                node.attributes
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Find a node at a specific position
    pub fn find_node_at_position(
        &self,
        project_id: i64,
        file_path: &str,
        line: u32,
        column: u32,
    ) -> Result<Option<NodeRecord>> {
        self.conn
            .query_row(
                r#"
                SELECT n.id, n.file_id, n.node_type, n.name, n.qualified_name,
                       n.start_line, n.start_column, n.end_line, n.end_column, n.attributes
                FROM nodes n
                JOIN files f ON n.file_id = f.id
                WHERE f.project_id = ?1
                  AND f.path = ?2
                  AND n.start_line <= ?3 AND n.end_line >= ?3
                  AND (n.start_line < ?3 OR n.start_column <= ?4)
                  AND (n.end_line > ?3 OR n.end_column >= ?4)
                ORDER BY (n.end_line - n.start_line), (n.end_column - n.start_column)
                LIMIT 1
                "#,
                params![project_id, file_path, line, column],
                |row| {
                    Ok(NodeRecord {
                        id: row.get(0)?,
                        file_id: row.get(1)?,
                        node_type: row.get(2)?,
                        name: row.get(3)?,
                        qualified_name: row.get(4)?,
                        start_line: row.get(5)?,
                        start_column: row.get(6)?,
                        end_line: row.get(7)?,
                        end_column: row.get(8)?,
                        attributes: row.get(9)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// Find symbol by name
    pub fn find_symbol_by_name(&self, project_id: i64, name: &str) -> Result<Option<NodeRecord>> {
        self.conn
            .query_row(
                r#"
                SELECT n.id, n.file_id, n.node_type, n.name, n.qualified_name,
                       n.start_line, n.start_column, n.end_line, n.end_column, n.attributes
                FROM nodes n
                JOIN files f ON n.file_id = f.id
                WHERE f.project_id = ?1 AND (n.name = ?2 OR n.qualified_name = ?2)
                LIMIT 1
                "#,
                params![project_id, name],
                |row| {
                    Ok(NodeRecord {
                        id: row.get(0)?,
                        file_id: row.get(1)?,
                        node_type: row.get(2)?,
                        name: row.get(3)?,
                        qualified_name: row.get(4)?,
                        start_line: row.get(5)?,
                        start_column: row.get(6)?,
                        end_line: row.get(7)?,
                        end_column: row.get(8)?,
                        attributes: row.get(9)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// Search symbols by name pattern
    pub fn search_symbols(
        &self,
        project_id: i64,
        query: &str,
        symbol_type: Option<&str>,
        limit: u32,
    ) -> Result<Vec<NodeRecord>> {
        let pattern = format!("%{}%", query);

        let row_mapper = |row: &rusqlite::Row| -> rusqlite::Result<NodeRecord> {
            Ok(NodeRecord {
                id: row.get(0)?,
                file_id: row.get(1)?,
                node_type: row.get(2)?,
                name: row.get(3)?,
                qualified_name: row.get(4)?,
                start_line: row.get(5)?,
                start_column: row.get(6)?,
                end_line: row.get(7)?,
                end_column: row.get(8)?,
                attributes: row.get(9)?,
            })
        };

        let mut result = Vec::new();

        if let Some(stype) = symbol_type {
            let mut stmt = self.conn.prepare(
                r#"
                SELECT n.id, n.file_id, n.node_type, n.name, n.qualified_name,
                       n.start_line, n.start_column, n.end_line, n.end_column, n.attributes
                FROM nodes n
                JOIN files f ON n.file_id = f.id
                WHERE f.project_id = ?1
                  AND n.node_type = ?2
                  AND (n.name LIKE ?3 OR n.qualified_name LIKE ?3)
                LIMIT ?4
                "#,
            )?;
            let rows = stmt.query_map(params![project_id, stype, pattern, limit], row_mapper)?;
            for row in rows {
                result.push(row?);
            }
        } else {
            let mut stmt = self.conn.prepare(
                r#"
                SELECT n.id, n.file_id, n.node_type, n.name, n.qualified_name,
                       n.start_line, n.start_column, n.end_line, n.end_column, n.attributes
                FROM nodes n
                JOIN files f ON n.file_id = f.id
                WHERE f.project_id = ?1
                  AND (n.name LIKE ?2 OR n.qualified_name LIKE ?2)
                LIMIT ?3
                "#,
            )?;
            let rows = stmt.query_map(params![project_id, pattern, limit], row_mapper)?;
            for row in rows {
                result.push(row?);
            }
        }

        Ok(result)
    }

    /// Get unresolved references (nodes that reference symbols not yet linked)
    pub fn get_unresolved_references(&self, project_id: i64) -> Result<Vec<(i64, String)>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT n.id, n.name
            FROM nodes n
            JOIN files f ON n.file_id = f.id
            LEFT JOIN edges e ON e.source_id = n.id AND e.edge_type = 'references'
            WHERE f.project_id = ?1
              AND n.node_type = 'reference'
              AND e.id IS NULL
            "#,
        )?;

        let rows = stmt.query_map(params![project_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// Find definition by name
    pub fn find_definition_by_name(&self, project_id: i64, name: &str) -> Result<Option<i64>> {
        self.conn
            .query_row(
                r#"
                SELECT n.id
                FROM nodes n
                JOIN files f ON n.file_id = f.id
                WHERE f.project_id = ?1
                  AND n.name = ?2
                  AND n.node_type IN ('function', 'method', 'class', 'interface', 'struct', 'variable')
                LIMIT 1
                "#,
                params![project_id, name],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    // ==================== Edge Operations ====================

    /// Insert a new edge
    pub fn insert_edge(&self, edge: &EdgeRecord) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO edges (source_id, target_id, edge_type, attributes) VALUES (?1, ?2, ?3, ?4)",
            params![edge.source_id, edge.target_id, edge.edge_type, edge.attributes],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Find the target of a reference
    pub fn find_reference_target(&self, node_id: i64) -> Result<Option<NodeRecord>> {
        self.conn
            .query_row(
                r#"
                SELECT n.id, n.file_id, n.node_type, n.name, n.qualified_name,
                       n.start_line, n.start_column, n.end_line, n.end_column, n.attributes
                FROM nodes n
                JOIN edges e ON e.target_id = n.id
                WHERE e.source_id = ?1 AND e.edge_type = 'references'
                LIMIT 1
                "#,
                params![node_id],
                |row| {
                    Ok(NodeRecord {
                        id: row.get(0)?,
                        file_id: row.get(1)?,
                        node_type: row.get(2)?,
                        name: row.get(3)?,
                        qualified_name: row.get(4)?,
                        start_line: row.get(5)?,
                        start_column: row.get(6)?,
                        end_line: row.get(7)?,
                        end_column: row.get(8)?,
                        attributes: row.get(9)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// Find all references to a node
    pub fn find_all_references(&self, node_id: i64) -> Result<Vec<NodeRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT n.id, n.file_id, n.node_type, n.name, n.qualified_name,
                   n.start_line, n.start_column, n.end_line, n.end_column, n.attributes
            FROM nodes n
            JOIN edges e ON e.source_id = n.id
            WHERE e.target_id = ?1 AND e.edge_type = 'references'
            "#,
        )?;

        let rows = stmt.query_map(params![node_id], |row| {
            Ok(NodeRecord {
                id: row.get(0)?,
                file_id: row.get(1)?,
                node_type: row.get(2)?,
                name: row.get(3)?,
                qualified_name: row.get(4)?,
                start_line: row.get(5)?,
                start_column: row.get(6)?,
                end_line: row.get(7)?,
                end_column: row.get(8)?,
                attributes: row.get(9)?,
            })
        })?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// Find callers of a function
    pub fn find_callers(&self, node_id: i64) -> Result<Vec<NodeRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT n.id, n.file_id, n.node_type, n.name, n.qualified_name,
                   n.start_line, n.start_column, n.end_line, n.end_column, n.attributes
            FROM nodes n
            JOIN edges e ON e.source_id = n.id
            WHERE e.target_id = ?1 AND e.edge_type = 'calls'
            "#,
        )?;

        let rows = stmt.query_map(params![node_id], |row| {
            Ok(NodeRecord {
                id: row.get(0)?,
                file_id: row.get(1)?,
                node_type: row.get(2)?,
                name: row.get(3)?,
                qualified_name: row.get(4)?,
                start_line: row.get(5)?,
                start_column: row.get(6)?,
                end_line: row.get(7)?,
                end_column: row.get(8)?,
                attributes: row.get(9)?,
            })
        })?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// Find callees of a function
    pub fn find_callees(&self, node_id: i64) -> Result<Vec<NodeRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT n.id, n.file_id, n.node_type, n.name, n.qualified_name,
                   n.start_line, n.start_column, n.end_line, n.end_column, n.attributes
            FROM nodes n
            JOIN edges e ON e.target_id = n.id
            WHERE e.source_id = ?1 AND e.edge_type = 'calls'
            "#,
        )?;

        let rows = stmt.query_map(params![node_id], |row| {
            Ok(NodeRecord {
                id: row.get(0)?,
                file_id: row.get(1)?,
                node_type: row.get(2)?,
                name: row.get(3)?,
                qualified_name: row.get(4)?,
                start_line: row.get(5)?,
                start_column: row.get(6)?,
                end_line: row.get(7)?,
                end_column: row.get(8)?,
                attributes: row.get(9)?,
            })
        })?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Database {
        let db = Database::open_in_memory().unwrap();
        db.init_schema().unwrap();
        db
    }

    fn create_project(db: &Database) -> i64 {
        let project = ProjectRecord {
            id: 0,
            name: "test-project".to_string(),
            root_path: "/test/path".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        db.insert_project(&project).unwrap()
    }

    fn create_file(db: &Database, project_id: i64) -> i64 {
        let file = FileRecord {
            id: 0,
            project_id,
            path: "/test/path/file.java".to_string(),
            language: "java".to_string(),
            content_hash: "abc123".to_string(),
            parsed_at: chrono::Utc::now(),
        };
        db.insert_file(&file).unwrap()
    }

    fn create_node(db: &Database, file_id: i64, node_type: &str, name: &str) -> i64 {
        let node = NodeRecord {
            id: 0,
            file_id,
            node_type: node_type.to_string(),
            name: name.to_string(),
            qualified_name: Some(format!("com.example.{}", name)),
            start_line: 1,
            start_column: 1,
            end_line: 10,
            end_column: 1,
            attributes: None,
        };
        db.insert_node(&node).unwrap()
    }

    #[test]
    fn test_init_schema() {
        let db = Database::open_in_memory().unwrap();
        db.init_schema().unwrap();
    }

    #[test]
    fn test_init_schema_idempotent() {
        let db = Database::open_in_memory().unwrap();
        db.init_schema().unwrap();
        db.init_schema().unwrap(); // Should not fail
    }

    #[test]
    fn test_insert_project() {
        let db = Database::open_in_memory().unwrap();
        db.init_schema().unwrap();

        let project = ProjectRecord {
            id: 0,
            name: "test".to_string(),
            root_path: "/test/path".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let id = db.insert_project(&project).unwrap();
        assert!(id > 0);
    }

    #[test]
    fn test_get_project_by_path() {
        let db = setup_db();
        let project_id = create_project(&db);

        let project = db.get_project_by_path("/test/path").unwrap();
        assert!(project.is_some());
        assert_eq!(project.unwrap().id, project_id);
    }

    #[test]
    fn test_get_project_by_path_not_found() {
        let db = setup_db();
        let project = db.get_project_by_path("/nonexistent").unwrap();
        assert!(project.is_none());
    }

    #[test]
    fn test_get_project_by_name() {
        let db = setup_db();
        create_project(&db);

        let project = db.get_project_by_name("test-project").unwrap();
        assert!(project.is_some());
        assert_eq!(project.unwrap().name, "test-project");
    }

    #[test]
    fn test_list_projects() {
        let db = setup_db();

        let project1 = ProjectRecord {
            id: 0,
            name: "project-a".to_string(),
            root_path: "/test/a".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let project2 = ProjectRecord {
            id: 0,
            name: "project-b".to_string(),
            root_path: "/test/b".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        db.insert_project(&project1).unwrap();
        db.insert_project(&project2).unwrap();

        let projects = db.list_projects().unwrap();
        assert_eq!(projects.len(), 2);
        // Should be sorted by name
        assert_eq!(projects[0].name, "project-a");
        assert_eq!(projects[1].name, "project-b");
    }

    #[test]
    fn test_update_project_timestamp() {
        let db = setup_db();
        let project_id = create_project(&db);

        // Wait a tiny bit to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(10));

        db.update_project_timestamp(project_id).unwrap();

        let project = db.get_project_by_path("/test/path").unwrap().unwrap();
        // updated_at should be recent (can't easily test exact value)
        assert!(project.updated_at >= project.created_at);
    }

    #[test]
    fn test_get_project_status() {
        let db = setup_db();
        let project_id = create_project(&db);
        let file_id = create_file(&db, project_id);
        create_node(&db, file_id, "class", "TestClass");

        let status = db.get_project_status(project_id).unwrap();
        assert!(status.is_some());
        let status = status.unwrap();
        assert_eq!(status.project_id, project_id);
        assert_eq!(status.files_parsed, 1);
        assert_eq!(status.nodes_count, 1);
        assert_eq!(status.status, "ready");
    }

    #[test]
    fn test_insert_file() {
        let db = setup_db();
        let project_id = create_project(&db);

        let file = FileRecord {
            id: 0,
            project_id,
            path: "/test/file.java".to_string(),
            language: "java".to_string(),
            content_hash: "hash123".to_string(),
            parsed_at: chrono::Utc::now(),
        };

        let file_id = db.insert_file(&file).unwrap();
        assert!(file_id > 0);
    }

    #[test]
    fn test_get_file_by_path() {
        let db = setup_db();
        let project_id = create_project(&db);
        let file_id = create_file(&db, project_id);

        let file = db.get_file_by_path(project_id, "/test/path/file.java").unwrap();
        assert!(file.is_some());
        assert_eq!(file.unwrap().id, file_id);
    }

    #[test]
    fn test_get_file() {
        let db = setup_db();
        let project_id = create_project(&db);
        let file_id = create_file(&db, project_id);

        let file = db.get_file(file_id).unwrap();
        assert!(file.is_some());
        assert_eq!(file.unwrap().path, "/test/path/file.java");
    }

    #[test]
    fn test_delete_file_data() {
        let db = setup_db();
        let project_id = create_project(&db);
        let file_id = create_file(&db, project_id);
        create_node(&db, file_id, "class", "Test");

        db.delete_file_data(file_id).unwrap();

        let file = db.get_file(file_id).unwrap();
        assert!(file.is_none());
    }

    #[test]
    fn test_insert_node() {
        let db = setup_db();
        let project_id = create_project(&db);
        let file_id = create_file(&db, project_id);

        let node = NodeRecord {
            id: 0,
            file_id,
            node_type: "function".to_string(),
            name: "myFunction".to_string(),
            qualified_name: Some("pkg.myFunction".to_string()),
            start_line: 5,
            start_column: 1,
            end_line: 10,
            end_column: 1,
            attributes: Some(r#"{"public":true}"#.to_string()),
        };

        let node_id = db.insert_node(&node).unwrap();
        assert!(node_id > 0);
    }

    #[test]
    fn test_find_node_at_position() {
        let db = setup_db();
        let project_id = create_project(&db);
        let file_id = create_file(&db, project_id);

        let node = NodeRecord {
            id: 0,
            file_id,
            node_type: "class".to_string(),
            name: "TestClass".to_string(),
            qualified_name: None,
            start_line: 1,
            start_column: 1,
            end_line: 20,
            end_column: 1,
            attributes: None,
        };
        db.insert_node(&node).unwrap();

        let found = db.find_node_at_position(project_id, "/test/path/file.java", 10, 5).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "TestClass");
    }

    #[test]
    fn test_find_node_at_position_not_found() {
        let db = setup_db();
        let project_id = create_project(&db);

        let found = db.find_node_at_position(project_id, "/nonexistent.java", 10, 5).unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_find_symbol_by_name() {
        let db = setup_db();
        let project_id = create_project(&db);
        let file_id = create_file(&db, project_id);
        create_node(&db, file_id, "class", "UserService");

        let found = db.find_symbol_by_name(project_id, "UserService").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "UserService");
    }

    #[test]
    fn test_find_symbol_by_qualified_name() {
        let db = setup_db();
        let project_id = create_project(&db);
        let file_id = create_file(&db, project_id);
        create_node(&db, file_id, "class", "UserService");

        let found = db.find_symbol_by_name(project_id, "com.example.UserService").unwrap();
        assert!(found.is_some());
    }

    #[test]
    fn test_search_symbols() {
        let db = setup_db();
        let project_id = create_project(&db);
        let file_id = create_file(&db, project_id);

        create_node(&db, file_id, "class", "UserService");
        create_node(&db, file_id, "class", "UserRepository");
        create_node(&db, file_id, "class", "OrderService");

        let results = db.search_symbols(project_id, "User", None, 10).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_symbols_by_type() {
        let db = setup_db();
        let project_id = create_project(&db);
        let file_id = create_file(&db, project_id);

        create_node(&db, file_id, "class", "UserService");
        create_node(&db, file_id, "method", "getUser");

        let results = db.search_symbols(project_id, "User", Some("method"), 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].node_type, "method");
    }

    #[test]
    fn test_search_symbols_with_limit() {
        let db = setup_db();
        let project_id = create_project(&db);
        let file_id = create_file(&db, project_id);

        for i in 0..10 {
            create_node(&db, file_id, "method", &format!("method{}", i));
        }

        let results = db.search_symbols(project_id, "method", None, 5).unwrap();
        assert_eq!(results.len(), 5);
    }

    #[test]
    fn test_insert_edge() {
        let db = setup_db();
        let project_id = create_project(&db);
        let file_id = create_file(&db, project_id);

        let node1_id = create_node(&db, file_id, "function", "caller");
        let node2_id = create_node(&db, file_id, "function", "callee");

        let edge = EdgeRecord {
            id: 0,
            source_id: node1_id,
            target_id: node2_id,
            edge_type: "calls".to_string(),
            attributes: None,
        };

        let edge_id = db.insert_edge(&edge).unwrap();
        assert!(edge_id > 0);
    }

    #[test]
    fn test_find_reference_target() {
        let db = setup_db();
        let project_id = create_project(&db);
        let file_id = create_file(&db, project_id);

        let source_id = create_node(&db, file_id, "reference", "UserService");
        let target_id = create_node(&db, file_id, "class", "UserService");

        let edge = EdgeRecord {
            id: 0,
            source_id,
            target_id,
            edge_type: "references".to_string(),
            attributes: None,
        };
        db.insert_edge(&edge).unwrap();

        let target = db.find_reference_target(source_id).unwrap();
        assert!(target.is_some());
        assert_eq!(target.unwrap().id, target_id);
    }

    #[test]
    fn test_find_all_references() {
        let db = setup_db();
        let project_id = create_project(&db);
        let file_id = create_file(&db, project_id);

        let target_id = create_node(&db, file_id, "class", "UserService");
        let ref1_id = create_node(&db, file_id, "reference", "ref1");
        let ref2_id = create_node(&db, file_id, "reference", "ref2");

        for ref_id in [ref1_id, ref2_id] {
            let edge = EdgeRecord {
                id: 0,
                source_id: ref_id,
                target_id,
                edge_type: "references".to_string(),
                attributes: None,
            };
            db.insert_edge(&edge).unwrap();
        }

        let refs = db.find_all_references(target_id).unwrap();
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn test_find_callers() {
        let db = setup_db();
        let project_id = create_project(&db);
        let file_id = create_file(&db, project_id);

        let callee_id = create_node(&db, file_id, "function", "helper");
        let caller1_id = create_node(&db, file_id, "function", "main");
        let caller2_id = create_node(&db, file_id, "function", "test");

        for caller_id in [caller1_id, caller2_id] {
            let edge = EdgeRecord {
                id: 0,
                source_id: caller_id,
                target_id: callee_id,
                edge_type: "calls".to_string(),
                attributes: None,
            };
            db.insert_edge(&edge).unwrap();
        }

        let callers = db.find_callers(callee_id).unwrap();
        assert_eq!(callers.len(), 2);
    }

    #[test]
    fn test_find_callees() {
        let db = setup_db();
        let project_id = create_project(&db);
        let file_id = create_file(&db, project_id);

        let caller_id = create_node(&db, file_id, "function", "main");
        let callee1_id = create_node(&db, file_id, "function", "helper1");
        let callee2_id = create_node(&db, file_id, "function", "helper2");

        for callee_id in [callee1_id, callee2_id] {
            let edge = EdgeRecord {
                id: 0,
                source_id: caller_id,
                target_id: callee_id,
                edge_type: "calls".to_string(),
                attributes: None,
            };
            db.insert_edge(&edge).unwrap();
        }

        let callees = db.find_callees(caller_id).unwrap();
        assert_eq!(callees.len(), 2);
    }

    #[test]
    fn test_get_unresolved_references() {
        let db = setup_db();
        let project_id = create_project(&db);
        let file_id = create_file(&db, project_id);

        // Create a reference node without a target edge
        let node = NodeRecord {
            id: 0,
            file_id,
            node_type: "reference".to_string(),
            name: "UnresolvedType".to_string(),
            qualified_name: None,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 15,
            attributes: None,
        };
        db.insert_node(&node).unwrap();

        let unresolved = db.get_unresolved_references(project_id).unwrap();
        assert_eq!(unresolved.len(), 1);
        assert_eq!(unresolved[0].1, "UnresolvedType");
    }

    #[test]
    fn test_find_definition_by_name() {
        let db = setup_db();
        let project_id = create_project(&db);
        let file_id = create_file(&db, project_id);

        create_node(&db, file_id, "function", "myFunction");
        create_node(&db, file_id, "call", "myFunction"); // This should not be found

        let def_id = db.find_definition_by_name(project_id, "myFunction").unwrap();
        assert!(def_id.is_some());
    }

    #[test]
    fn test_cascade_delete() {
        let db = setup_db();
        let project_id = create_project(&db);
        let file_id = create_file(&db, project_id);
        let node_id = create_node(&db, file_id, "class", "Test");

        // Verify node exists
        let found = db.find_symbol_by_name(project_id, "Test").unwrap();
        assert!(found.is_some());

        // Delete file should cascade delete nodes
        db.delete_file_data(file_id).unwrap();

        // Node should be gone
        let found = db.find_symbol_by_name(project_id, "Test").unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_open_file_database() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        {
            let db = Database::open(&db_path).unwrap();
            db.init_schema().unwrap();
            create_project(&db);
        }

        // Re-open the database
        let db = Database::open(&db_path).unwrap();
        let projects = db.list_projects().unwrap();
        assert_eq!(projects.len(), 1);
    }

    #[test]
    fn test_unique_project_path_constraint() {
        let db = setup_db();

        let project1 = ProjectRecord {
            id: 0,
            name: "project1".to_string(),
            root_path: "/same/path".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let project2 = ProjectRecord {
            id: 0,
            name: "project2".to_string(),
            root_path: "/same/path".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        db.insert_project(&project1).unwrap();
        let result = db.insert_project(&project2);
        assert!(result.is_err()); // Should fail due to unique constraint
    }

    #[test]
    fn test_unique_file_path_constraint() {
        let db = setup_db();
        let project_id = create_project(&db);

        let file1 = FileRecord {
            id: 0,
            project_id,
            path: "/same/file.java".to_string(),
            language: "java".to_string(),
            content_hash: "hash1".to_string(),
            parsed_at: chrono::Utc::now(),
        };

        let file2 = FileRecord {
            id: 0,
            project_id,
            path: "/same/file.java".to_string(),
            language: "java".to_string(),
            content_hash: "hash2".to_string(),
            parsed_at: chrono::Utc::now(),
        };

        db.insert_file(&file1).unwrap();
        let result = db.insert_file(&file2);
        assert!(result.is_err()); // Should fail due to unique constraint
    }
}
