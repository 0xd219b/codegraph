//! Core engine for code graph parsing and querying

pub mod config;
pub mod graph;
pub mod parser;
pub mod query;
pub mod registry;

use std::path::Path;
use tracing::info;

use crate::languages::LanguageRegistry;
use crate::storage::Database;

/// Parse a project and build the code graph
pub async fn parse_project(
    db_path: &Path,
    project_name: &str,
    project_path: &Path,
    languages: Option<&[String]>,
) -> anyhow::Result<()> {
    let db = Database::open(db_path)?;
    db.init_schema()?;

    let registry = LanguageRegistry::new();
    let parser = parser::CodeParser::new(registry);
    let mut builder = graph::GraphBuilder::new(db);

    // Create or get project
    let project_id = builder.create_or_get_project(project_name, project_path)?;

    info!("Project ID: {}", project_id);

    // Collect files to parse
    let files = parser.collect_files(project_path, languages)?;
    info!("Found {} files to parse", files.len());

    // Parse each file
    for (file_path, language) in files {
        info!("Parsing {:?} as {}", file_path, language);
        match parser.parse_file(&file_path, &language) {
            Ok(graph_data) => {
                builder.store_file_graph(project_id, &file_path, &language, graph_data)?;
            }
            Err(e) => {
                tracing::warn!("Failed to parse {:?}: {}", file_path, e);
            }
        }
    }

    // Build cross-file references
    builder.build_cross_references(project_id)?;

    info!("Project parsing complete");
    Ok(())
}
