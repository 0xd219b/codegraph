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
