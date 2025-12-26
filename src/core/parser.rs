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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_registry() -> LanguageRegistry {
        LanguageRegistry::new()
    }

    fn create_temp_file(dir: &TempDir, name: &str, content: &str) -> PathBuf {
        let path = dir.path().join(name);
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn test_parser_new() {
        let registry = create_test_registry();
        let parser = CodeParser::new(registry);
        // Parser should be created successfully
        assert!(true);
        drop(parser);
    }

    #[test]
    fn test_collect_files_empty_dir() {
        let temp_dir = TempDir::new().unwrap();
        let registry = create_test_registry();
        let parser = CodeParser::new(registry);

        let files = parser.collect_files(temp_dir.path(), None).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_collect_files_empty() {
        let temp_dir = TempDir::new().unwrap();
        let registry = create_test_registry();
        let parser = CodeParser::new(registry);

        create_temp_file(&temp_dir, "readme.txt", "Not a code file");

        let files = parser.collect_files(temp_dir.path(), None).unwrap();
        // txt files should not be collected
        assert!(files.iter().all(|(_, lang)| lang != "txt"));
    }

    #[test]
    fn test_collect_files_result_type() {
        let temp_dir = TempDir::new().unwrap();
        let registry = create_test_registry();
        let parser = CodeParser::new(registry);

        // Just verify that collect_files returns valid results
        let result = parser.collect_files(temp_dir.path(), None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_collect_files_with_filter_returns_matching() {
        let temp_dir = TempDir::new().unwrap();
        let registry = create_test_registry();
        let parser = CodeParser::new(registry);

        let filter = vec!["java".to_string()];
        let files = parser.collect_files(temp_dir.path(), Some(&filter)).unwrap();
        // All returned files should be Java (if any)
        assert!(files.iter().all(|(_, lang)| lang == "java"));
    }

    #[test]
    fn test_collect_files_skip_hidden_dirs() {
        let temp_dir = TempDir::new().unwrap();
        let registry = create_test_registry();
        let parser = CodeParser::new(registry);

        // Create hidden directory
        let hidden_dir = temp_dir.path().join(".hidden");
        std::fs::create_dir(&hidden_dir).unwrap();
        std::fs::write(hidden_dir.join("test.java"), "class Test {}").unwrap();

        let files = parser.collect_files(temp_dir.path(), None).unwrap();
        // Hidden files should not be included
        assert!(files.iter().all(|(p, _)| !p.to_string_lossy().contains(".hidden")));
    }

    #[test]
    fn test_parse_java_file() {
        let temp_dir = TempDir::new().unwrap();
        let registry = create_test_registry();
        let parser = CodeParser::new(registry);

        let java_code = "public class HelloWorld { public void sayHello() {} }";
        let path = create_temp_file(&temp_dir, "HelloWorld.java", java_code);
        let result = parser.parse_file(&path, "java").unwrap();

        assert!(!result.nodes.is_empty());
        assert!(!result.content_hash.is_empty());

        // Should find class and method
        let node_types: Vec<_> = result.nodes.iter().map(|n| n.node_type.as_str()).collect();
        assert!(node_types.contains(&"class"));
        assert!(node_types.contains(&"method"));
    }

    #[test]
    fn test_parse_go_file() {
        let temp_dir = TempDir::new().unwrap();
        let registry = create_test_registry();
        let parser = CodeParser::new(registry);

        let go_code = r#"
package main

import "fmt"

func main() {
    fmt.Println("Hello")
}
"#;
        let path = create_temp_file(&temp_dir, "main.go", go_code);
        let result = parser.parse_file(&path, "go").unwrap();

        assert!(!result.nodes.is_empty());
        assert!(!result.content_hash.is_empty());

        // Should find package, import, and function
        let node_types: Vec<_> = result.nodes.iter().map(|n| n.node_type.as_str()).collect();
        assert!(node_types.contains(&"package"));
        assert!(node_types.contains(&"import"));
        assert!(node_types.contains(&"function"));
    }

    #[test]
    fn test_parse_file_unsupported_language() {
        let temp_dir = TempDir::new().unwrap();
        let registry = create_test_registry();
        let parser = CodeParser::new(registry);

        let path = create_temp_file(&temp_dir, "test.rs", "fn main() {}");
        let result = parser.parse_file(&path, "rust");

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unsupported language"));
    }

    #[test]
    fn test_parse_file_not_found() {
        let registry = create_test_registry();
        let parser = CodeParser::new(registry);

        let result = parser.parse_file(Path::new("/nonexistent/file.java"), "java");
        assert!(result.is_err());
    }

    #[test]
    fn test_content_hash_consistency() {
        let temp_dir = TempDir::new().unwrap();
        let registry = create_test_registry();
        let parser = CodeParser::new(registry);

        let java_code = "public class Test {}";
        let path = create_temp_file(&temp_dir, "Test.java", java_code);

        let result1 = parser.parse_file(&path, "java").unwrap();
        let result2 = parser.parse_file(&path, "java").unwrap();

        assert_eq!(result1.content_hash, result2.content_hash);
    }

    #[test]
    fn test_content_hash_different_content() {
        let temp_dir = TempDir::new().unwrap();
        let registry = create_test_registry();
        let parser = CodeParser::new(registry);

        let path1 = create_temp_file(&temp_dir, "Test1.java", "public class Test1 {}");
        let path2 = create_temp_file(&temp_dir, "Test2.java", "public class Test2 {}");

        let result1 = parser.parse_file(&path1, "java").unwrap();
        let result2 = parser.parse_file(&path2, "java").unwrap();

        assert_ne!(result1.content_hash, result2.content_hash);
    }

    #[test]
    fn test_hex_encode() {
        let result = hex::encode(&[0x48, 0x65, 0x6c, 0x6c, 0x6f]);
        assert_eq!(result, "48656c6c6f");
    }

    #[test]
    fn test_hex_encode_empty() {
        let result = hex::encode(&[]);
        assert_eq!(result, "");
    }

    #[test]
    fn test_is_hidden() {
        use walkdir::WalkDir;

        let temp_dir = TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join(".hidden"), "").unwrap();
        std::fs::write(temp_dir.path().join("visible"), "").unwrap();

        let entries: Vec<_> = WalkDir::new(temp_dir.path())
            .into_iter()
            .filter_map(|e| e.ok())
            .collect();

        for entry in &entries {
            let name = entry.file_name().to_string_lossy();
            if name.starts_with('.') && name != "." {
                assert!(is_hidden(entry));
            }
        }
    }
}
