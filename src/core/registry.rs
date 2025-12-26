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
    use crate::languages::java::JavaLanguage;
    use crate::languages::go::GoLanguage;

    #[test]
    fn test_empty_registry() {
        let registry = LanguageRegistry::empty();
        assert!(registry.language_ids().is_empty());
    }

    #[test]
    fn test_register_language() {
        let mut registry = LanguageRegistry::empty();
        registry.register(Arc::new(JavaLanguage::new()));

        let ids = registry.language_ids();
        assert_eq!(ids.len(), 1);
        assert!(ids.contains(&"java"));
    }

    #[test]
    fn test_register_multiple_languages() {
        let mut registry = LanguageRegistry::empty();
        registry.register(Arc::new(JavaLanguage::new()));
        registry.register(Arc::new(GoLanguage::new()));

        let ids = registry.language_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"java"));
        assert!(ids.contains(&"go"));
    }

    #[test]
    fn test_get_by_id() {
        let mut registry = LanguageRegistry::empty();
        registry.register(Arc::new(JavaLanguage::new()));

        let lang = registry.get("java");
        assert!(lang.is_some());
        assert_eq!(lang.unwrap().language_id(), "java");
    }

    #[test]
    fn test_get_by_id_not_found() {
        let registry = LanguageRegistry::empty();
        let lang = registry.get("python");
        assert!(lang.is_none());
    }

    #[test]
    fn test_get_by_extension_with_dot() {
        let mut registry = LanguageRegistry::empty();
        registry.register(Arc::new(JavaLanguage::new()));

        let lang = registry.get_by_extension(".java");
        assert!(lang.is_some());
        assert_eq!(lang.unwrap().language_id(), "java");
    }

    #[test]
    fn test_get_by_extension_without_dot() {
        let mut registry = LanguageRegistry::empty();
        registry.register(Arc::new(GoLanguage::new()));

        let lang = registry.get_by_extension("go");
        assert!(lang.is_some());
        assert_eq!(lang.unwrap().language_id(), "go");
    }

    #[test]
    fn test_get_by_extension_not_found() {
        let mut registry = LanguageRegistry::empty();
        registry.register(Arc::new(JavaLanguage::new()));

        let lang = registry.get_by_extension(".py");
        assert!(lang.is_none());
    }

    #[test]
    fn test_is_supported() {
        let mut registry = LanguageRegistry::empty();
        registry.register(Arc::new(JavaLanguage::new()));

        assert!(registry.is_supported(".java"));
        assert!(registry.is_supported("java"));
        assert!(!registry.is_supported(".py"));
        assert!(!registry.is_supported("python"));
    }

    #[test]
    fn test_list_languages() {
        let mut registry = LanguageRegistry::empty();
        registry.register(Arc::new(JavaLanguage::new()));
        registry.register(Arc::new(GoLanguage::new()));

        let languages = registry.list_languages();
        assert_eq!(languages.len(), 2);
    }

    #[test]
    fn test_extension_map() {
        let mut registry = LanguageRegistry::empty();
        registry.register(Arc::new(JavaLanguage::new()));

        // Verify extension mapping works
        assert!(registry.extension_map.contains_key(".java"));
        assert_eq!(registry.extension_map.get(".java"), Some(&"java".to_string()));
    }

    #[test]
    fn test_language_extensions() {
        let java = JavaLanguage::new();
        assert!(java.file_extensions().contains(&".java"));

        let go = GoLanguage::new();
        assert!(go.file_extensions().contains(&".go"));
    }

    #[test]
    fn test_language_grammar() {
        let java = JavaLanguage::new();
        let _grammar = java.grammar();
        // Grammar should be valid (no panic)

        let go = GoLanguage::new();
        let _grammar = go.grammar();
        // Grammar should be valid (no panic)
    }
}
