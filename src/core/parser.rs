//! Code parser using tree-sitter

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::debug;
use walkdir::WalkDir;

use crate::languages::LanguageRegistry;
use crate::storage::models::{EdgeData, NodeData};

/// Parsed graph data from a single file
#[derive(Debug, Clone)]
pub struct FileGraphData {
    pub nodes: Vec<NodeData>,
    pub edges: Vec<EdgeData>,
    pub content_hash: String,
}

/// Code parser that uses tree-sitter for syntax analysis
pub struct CodeParser {
    registry: LanguageRegistry,
}

impl CodeParser {
    /// Create a new parser with the given language registry
    pub fn new(registry: LanguageRegistry) -> Self {
        Self { registry }
    }

    /// Collect all parseable files in a directory
    pub fn collect_files(
        &self,
        root: &Path,
        filter_languages: Option<&[String]>,
    ) -> Result<Vec<(PathBuf, String)>> {
        let mut files = Vec::new();

        for entry in WalkDir::new(root)
            .follow_links(true)
            .into_iter()
            .filter_entry(|e| !is_hidden(e))
        {
            let entry = entry?;
            if entry.file_type().is_file() {
                if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                    if let Some(lang) = self.registry.get_by_extension(ext) {
                        let lang_id = lang.language_id().to_string();

                        // Apply language filter if specified
                        if let Some(filters) = filter_languages {
                            if !filters.contains(&lang_id) {
                                continue;
                            }
                        }

                        files.push((entry.path().to_path_buf(), lang_id));
                    }
                }
            }
        }

        Ok(files)
    }

    /// Parse a single file and extract graph data
    pub fn parse_file(&self, path: &Path, language_id: &str) -> Result<FileGraphData> {
        // Read file as bytes first to handle non-UTF8 encodings
        let bytes = fs::read(path)
            .with_context(|| format!("Failed to read file: {:?}", path))?;

        // Convert to UTF-8, replacing invalid sequences with replacement character
        let content = String::from_utf8_lossy(&bytes).into_owned();

        let content_hash = compute_hash(&content);

        let lang = self
            .registry
            .get(language_id)
            .ok_or_else(|| anyhow::anyhow!("Unsupported language: {}", language_id))?;

        // Create tree-sitter parser
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&lang.grammar())
            .with_context(|| format!("Failed to set language: {}", language_id))?;

        // Parse the source code
        let tree = parser
            .parse(&content, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse file: {:?}", path))?;

        debug!("Parsed {:?}, root node: {:?}", path, tree.root_node().kind());

        // Extract graph data using language-specific rules
        let (nodes, edges) = lang.extract_graph(&content, &tree)?;

        Ok(FileGraphData {
            nodes,
            edges,
            content_hash,
        })
    }
}

/// Check if a directory entry is hidden
fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

/// Compute SHA-256 hash of content
fn compute_hash(content: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

// Add hex encoding dependency workaround
mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes
            .as_ref()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }
}
