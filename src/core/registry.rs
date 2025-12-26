//! Language registry for managing supported programming languages

use std::collections::HashMap;
use std::sync::Arc;

use crate::languages::LanguageSupport;

/// Registry for managing language support plugins
pub struct LanguageRegistry {
    languages: HashMap<String, Arc<dyn LanguageSupport>>,
    extension_map: HashMap<String, String>,
}

impl LanguageRegistry {
    /// Create a new empty registry
    pub fn empty() -> Self {
        Self {
            languages: HashMap::new(),
            extension_map: HashMap::new(),
        }
    }

    /// Register a language support plugin
    pub fn register(&mut self, language: Arc<dyn LanguageSupport>) {
        let id = language.language_id().to_string();

        // Map extensions to language ID
        for ext in language.file_extensions() {
            self.extension_map.insert(ext.to_string(), id.clone());
        }

        self.languages.insert(id, language);
    }

    /// Get language support by ID
    pub fn get(&self, language_id: &str) -> Option<&Arc<dyn LanguageSupport>> {
        self.languages.get(language_id)
    }

    /// Get language support by file extension
    pub fn get_by_extension(&self, extension: &str) -> Option<&Arc<dyn LanguageSupport>> {
        let ext = if extension.starts_with('.') {
            extension.to_string()
        } else {
            format!(".{}", extension)
        };

        self.extension_map
            .get(&ext)
            .and_then(|id| self.languages.get(id))
    }

    /// List all supported language IDs
    pub fn language_ids(&self) -> Vec<&str> {
        self.languages.keys().map(|s| s.as_str()).collect()
    }

    /// List all registered languages
    pub fn list_languages(&self) -> Vec<&Arc<dyn LanguageSupport>> {
        self.languages.values().collect()
    }

    /// Check if a file extension is supported
    pub fn is_supported(&self, extension: &str) -> bool {
        self.get_by_extension(extension).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_registry() {
        let registry = LanguageRegistry::empty();
        assert!(registry.language_ids().is_empty());
    }
}
