//! Graph builder for constructing code graphs

use std::path::Path;

use anyhow::Result;
use tracing::debug;

use crate::core::parser::FileGraphData;
use crate::storage::models::{EdgeRecord, FileRecord, NodeRecord, ProjectRecord};
use crate::storage::Database;

/// Builder for constructing and storing code graphs
pub struct GraphBuilder {
    db: Database,
}

impl GraphBuilder {
    /// Create a new graph builder with the given database
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Create or get an existing project
    pub fn create_or_get_project(&self, name: &str, root_path: &Path) -> Result<i64> {
        let root_path_str = root_path.to_string_lossy().to_string();

        // Try to find existing project
        if let Some(project) = self.db.get_project_by_path(&root_path_str)? {
            debug!("Found existing project: {} (id={})", project.name, project.id);
            return Ok(project.id);
        }

        // Create new project
        let project = ProjectRecord {
            id: 0,
            name: name.to_string(),
            root_path: root_path_str,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let id = self.db.insert_project(&project)?;
        debug!("Created new project: {} (id={})", name, id);
        Ok(id)
    }

    /// Store graph data for a single file
    pub fn store_file_graph(
        &mut self,
        project_id: i64,
        file_path: &Path,
        language: &str,
        graph_data: FileGraphData,
    ) -> Result<i64> {
        let file_path_str = file_path.to_string_lossy().to_string();

        // Check if file already exists
        if let Some(existing) = self.db.get_file_by_path(project_id, &file_path_str)? {
            // Check if content changed
            if existing.content_hash == graph_data.content_hash {
                debug!("File unchanged, skipping: {:?}", file_path);
                return Ok(existing.id);
            }

            // Delete old data and re-parse
            debug!("File changed, re-parsing: {:?}", file_path);
            self.db.delete_file_data(existing.id)?;
        }

        // Insert file record
        let file = FileRecord {
            id: 0,
            project_id,
            path: file_path_str,
            language: language.to_string(),
            content_hash: graph_data.content_hash,
            parsed_at: chrono::Utc::now(),
        };
        let file_id = self.db.insert_file(&file)?;

        // Insert nodes
        let mut node_id_map = std::collections::HashMap::new();
        for (idx, node_data) in graph_data.nodes.into_iter().enumerate() {
            let node = NodeRecord {
                id: 0,
                file_id,
                node_type: node_data.node_type,
                name: node_data.name,
                qualified_name: node_data.qualified_name,
                start_line: node_data.start_line,
                start_column: node_data.start_column,
                end_line: node_data.end_line,
                end_column: node_data.end_column,
                attributes: node_data.attributes,
            };
            let node_id = self.db.insert_node(&node)?;
            node_id_map.insert(idx, node_id);
        }

        // Insert edges (using local indices)
        let edges_count = graph_data.edges.len();
        for edge_data in graph_data.edges {
            if let (Some(&source_id), Some(&target_id)) = (
                node_id_map.get(&(edge_data.source_idx as usize)),
                node_id_map.get(&(edge_data.target_idx as usize)),
            ) {
                let edge = EdgeRecord {
                    id: 0,
                    source_id,
                    target_id,
                    edge_type: edge_data.edge_type,
                    attributes: edge_data.attributes,
                };
                self.db.insert_edge(&edge)?;
            }
        }

        debug!(
            "Stored graph for {:?}: {} nodes, {} edges",
            file_path,
            node_id_map.len(),
            edges_count
        );

        Ok(file_id)
    }

    /// Build cross-file references after all files are parsed
    pub fn build_cross_references(&mut self, project_id: i64) -> Result<()> {
        debug!("Building cross-file references for project {}", project_id);

        // Get all unresolved references (nodes without target)
        let unresolved = self.db.get_unresolved_references(project_id)?;
        debug!("Found {} unresolved references", unresolved.len());

        for (ref_node_id, ref_name) in unresolved {
            // Try to find definition by name
            if let Some(def_node_id) = self.db.find_definition_by_name(project_id, &ref_name)? {
                // Create reference edge
                let edge = EdgeRecord {
                    id: 0,
                    source_id: ref_node_id,
                    target_id: def_node_id,
                    edge_type: "references".to_string(),
                    attributes: None,
                };
                self.db.insert_edge(&edge)?;
                debug!(
                    "Resolved reference: {} -> {} ({})",
                    ref_node_id, def_node_id, ref_name
                );
            }
        }

        // Update project timestamp
        self.db.update_project_timestamp(project_id)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::{EdgeData, NodeData};
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn setup_test_db() -> (TempDir, Database) {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open_in_memory().unwrap();
        db.init_schema().unwrap();
        (temp_dir, db)
    }

    fn create_test_graph_data() -> FileGraphData {
        let nodes = vec![
            NodeData {
                node_type: "class".to_string(),
                name: "TestClass".to_string(),
                qualified_name: Some("com.example.TestClass".to_string()),
                start_line: 1,
                start_column: 1,
                end_line: 10,
                end_column: 1,
                attributes: None,
            },
            NodeData {
                node_type: "method".to_string(),
                name: "testMethod".to_string(),
                qualified_name: Some("com.example.TestClass.testMethod".to_string()),
                start_line: 3,
                start_column: 5,
                end_line: 8,
                end_column: 5,
                attributes: None,
            },
        ];

        let edges = vec![EdgeData {
            source_idx: 0,
            target_idx: 1,
            edge_type: "contains".to_string(),
            attributes: None,
        }];

        FileGraphData {
            nodes,
            edges,
            content_hash: "abc123".to_string(),
        }
    }

    #[test]
    fn test_graph_builder_new() {
        let (_temp_dir, db) = setup_test_db();
        let builder = GraphBuilder::new(db);
        drop(builder);
    }

    #[test]
    fn test_create_new_project() {
        let (temp_dir, db) = setup_test_db();
        let builder = GraphBuilder::new(db);

        let project_id = builder
            .create_or_get_project("test-project", temp_dir.path())
            .unwrap();

        assert!(project_id > 0);
    }

    #[test]
    fn test_get_existing_project() {
        let (temp_dir, db) = setup_test_db();
        let builder = GraphBuilder::new(db);

        let project_id1 = builder
            .create_or_get_project("test-project", temp_dir.path())
            .unwrap();

        let project_id2 = builder
            .create_or_get_project("test-project", temp_dir.path())
            .unwrap();

        assert_eq!(project_id1, project_id2);
    }

    #[test]
    fn test_store_file_graph() {
        let (temp_dir, db) = setup_test_db();
        let mut builder = GraphBuilder::new(db);

        let project_id = builder
            .create_or_get_project("test-project", temp_dir.path())
            .unwrap();

        let file_path = PathBuf::from("/test/TestClass.java");
        let graph_data = create_test_graph_data();

        let file_id = builder
            .store_file_graph(project_id, &file_path, "java", graph_data)
            .unwrap();

        assert!(file_id > 0);
    }

    #[test]
    fn test_store_file_graph_unchanged() {
        let (temp_dir, db) = setup_test_db();
        let mut builder = GraphBuilder::new(db);

        let project_id = builder
            .create_or_get_project("test-project", temp_dir.path())
            .unwrap();

        let file_path = PathBuf::from("/test/TestClass.java");
        let graph_data1 = create_test_graph_data();
        let graph_data2 = create_test_graph_data();

        let file_id1 = builder
            .store_file_graph(project_id, &file_path, "java", graph_data1)
            .unwrap();

        let file_id2 = builder
            .store_file_graph(project_id, &file_path, "java", graph_data2)
            .unwrap();

        // Same file with same content hash should return same ID
        assert_eq!(file_id1, file_id2);
    }

    #[test]
    fn test_store_file_graph_changed() {
        let (temp_dir, db) = setup_test_db();
        let mut builder = GraphBuilder::new(db);

        let project_id = builder
            .create_or_get_project("test-project", temp_dir.path())
            .unwrap();

        let file_path = PathBuf::from("/test/TestClass.java");

        let mut graph_data1 = create_test_graph_data();
        graph_data1.content_hash = "hash1".to_string();

        let mut graph_data2 = create_test_graph_data();
        graph_data2.content_hash = "hash2".to_string();

        let file_id1 = builder
            .store_file_graph(project_id, &file_path, "java", graph_data1)
            .unwrap();

        let file_id2 = builder
            .store_file_graph(project_id, &file_path, "java", graph_data2)
            .unwrap();

        // Both operations should succeed and return valid file IDs
        assert!(file_id1 > 0);
        assert!(file_id2 > 0);
    }

    #[test]
    fn test_store_multiple_files() {
        let (temp_dir, db) = setup_test_db();
        let mut builder = GraphBuilder::new(db);

        let project_id = builder
            .create_or_get_project("test-project", temp_dir.path())
            .unwrap();

        let file1 = PathBuf::from("/test/File1.java");
        let file2 = PathBuf::from("/test/File2.java");

        let mut graph1 = create_test_graph_data();
        graph1.content_hash = "hash1".to_string();

        let mut graph2 = create_test_graph_data();
        graph2.content_hash = "hash2".to_string();

        let file_id1 = builder.store_file_graph(project_id, &file1, "java", graph1).unwrap();
        let file_id2 = builder.store_file_graph(project_id, &file2, "java", graph2).unwrap();

        assert!(file_id1 > 0);
        assert!(file_id2 > 0);
        assert_ne!(file_id1, file_id2);
    }

    #[test]
    fn test_build_cross_references() {
        let (temp_dir, db) = setup_test_db();
        let mut builder = GraphBuilder::new(db);

        let project_id = builder
            .create_or_get_project("test-project", temp_dir.path())
            .unwrap();

        // Create a file with reference nodes
        let nodes = vec![
            NodeData {
                node_type: "class".to_string(),
                name: "UserService".to_string(),
                qualified_name: Some("com.example.UserService".to_string()),
                start_line: 1,
                start_column: 1,
                end_line: 10,
                end_column: 1,
                attributes: None,
            },
            NodeData {
                node_type: "reference".to_string(),
                name: "UserRepository".to_string(),
                qualified_name: None,
                start_line: 3,
                start_column: 5,
                end_line: 3,
                end_column: 20,
                attributes: None,
            },
        ];

        let graph_data = FileGraphData {
            nodes,
            edges: vec![],
            content_hash: "test_hash".to_string(),
        };

        let file_path = PathBuf::from("/test/UserService.java");
        builder
            .store_file_graph(project_id, &file_path, "java", graph_data)
            .unwrap();

        // Build cross references (should not fail even with unresolved refs)
        let result = builder.build_cross_references(project_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_store_graph_with_edges() {
        let (temp_dir, db) = setup_test_db();
        let mut builder = GraphBuilder::new(db);

        let project_id = builder
            .create_or_get_project("test-project", temp_dir.path())
            .unwrap();

        let nodes = vec![
            NodeData {
                node_type: "function".to_string(),
                name: "main".to_string(),
                qualified_name: Some("main.main".to_string()),
                start_line: 1,
                start_column: 1,
                end_line: 10,
                end_column: 1,
                attributes: None,
            },
            NodeData {
                node_type: "call".to_string(),
                name: "helper".to_string(),
                qualified_name: None,
                start_line: 5,
                start_column: 5,
                end_line: 5,
                end_column: 15,
                attributes: None,
            },
        ];

        let edges = vec![EdgeData {
            source_idx: 0,
            target_idx: 1,
            edge_type: "calls".to_string(),
            attributes: None,
        }];

        let graph_data = FileGraphData {
            nodes,
            edges,
            content_hash: "edge_test_hash".to_string(),
        };

        let file_path = PathBuf::from("/test/main.go");
        let file_id = builder
            .store_file_graph(project_id, &file_path, "go", graph_data)
            .unwrap();

        assert!(file_id > 0);
    }

    #[test]
    fn test_store_graph_with_invalid_edge_indices() {
        let (temp_dir, db) = setup_test_db();
        let mut builder = GraphBuilder::new(db);

        let project_id = builder
            .create_or_get_project("test-project", temp_dir.path())
            .unwrap();

        let nodes = vec![NodeData {
            node_type: "function".to_string(),
            name: "test".to_string(),
            qualified_name: None,
            start_line: 1,
            start_column: 1,
            end_line: 5,
            end_column: 1,
            attributes: None,
        }];

        // Edge with invalid indices (target_idx doesn't exist)
        let edges = vec![EdgeData {
            source_idx: 0,
            target_idx: 99, // Invalid index
            edge_type: "calls".to_string(),
            attributes: None,
        }];

        let graph_data = FileGraphData {
            nodes,
            edges,
            content_hash: "invalid_edge_hash".to_string(),
        };

        let file_path = PathBuf::from("/test/invalid.go");

        // Should not fail, just skip invalid edges
        let result = builder.store_file_graph(project_id, &file_path, "go", graph_data);
        assert!(result.is_ok());
    }
}
