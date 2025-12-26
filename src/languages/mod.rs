//! Language support for code parsing
//!
//! This module provides the trait for language support plugins and
//! implementations for supported languages (Java, Go, etc.)

pub mod go;
pub mod java;

use std::sync::Arc;

use anyhow::Result;
use tree_sitter::Tree;

use crate::storage::models::{EdgeData, NodeData};

/// Trait for language support plugins
pub trait LanguageSupport: Send + Sync {
    /// Get the language identifier (e.g., "java", "go")
    fn language_id(&self) -> &str;

    /// Get supported file extensions (e.g., [".java"], [".go"])
    fn file_extensions(&self) -> &[&str];

    /// Get the tree-sitter grammar
    fn grammar(&self) -> tree_sitter::Language;

    /// Extract graph data from parsed source code
    fn extract_graph(&self, source: &str, tree: &Tree) -> Result<(Vec<NodeData>, Vec<EdgeData>)>;
}

/// Registry for managing language support plugins
pub struct LanguageRegistry {
    languages: Vec<Arc<dyn LanguageSupport>>,
}

impl LanguageRegistry {
    /// Create a new registry with default language support
    pub fn new() -> Self {
        let mut registry = Self {
            languages: Vec::new(),
        };

        // Register built-in languages
        registry.register(Arc::new(java::JavaLanguage::new()));
        registry.register(Arc::new(go::GoLanguage::new()));

        registry
    }

    /// Register a language support plugin
    pub fn register(&mut self, language: Arc<dyn LanguageSupport>) {
        self.languages.push(language);
    }

    /// Get language support by ID
    pub fn get(&self, language_id: &str) -> Option<&Arc<dyn LanguageSupport>> {
        self.languages.iter().find(|l| l.language_id() == language_id)
    }

    /// Get language support by file extension
    pub fn get_by_extension(&self, extension: &str) -> Option<&Arc<dyn LanguageSupport>> {
        let ext = if extension.starts_with('.') {
            extension.to_string()
        } else {
            format!(".{}", extension)
        };

        self.languages
            .iter()
            .find(|l| l.file_extensions().contains(&ext.as_str()))
    }

    /// List all supported languages
    pub fn list_languages(&self) -> &[Arc<dyn LanguageSupport>] {
        &self.languages
    }
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::new()
    }
}
