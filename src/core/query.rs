//! Query executor for code graph queries

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::storage::Database;

/// Result of a definition query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefinitionResult {
    pub found: bool,
    pub definition: Option<SymbolLocation>,
}

/// Result of a references query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferencesResult {
    pub count: usize,
    pub references: Vec<SymbolLocation>,
}

/// Result of a call graph query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallGraphResult {
    pub center: SymbolInfo,
    pub callers: Vec<SymbolInfo>,
    pub callees: Vec<SymbolInfo>,
}

/// Result of a symbol search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolSearchResult {
    pub count: usize,
    pub symbols: Vec<SymbolInfo>,
}

/// Location of a symbol in the source code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolLocation {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub node_type: String,
    pub name: String,
    pub qualified_name: Option<String>,
    pub context: Option<String>,
}

/// Information about a symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInfo {
    pub name: String,
    pub qualified_name: Option<String>,
    pub node_type: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
}

/// Query executor for the code graph
pub struct QueryExecutor {
    db: Database,
}

impl QueryExecutor {
    /// Create a new query executor
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Find the definition of a symbol at the given location
    pub fn find_definition(
        &self,
        project_id: i64,
        file: &str,
        line: u32,
        column: u32,
    ) -> Result<DefinitionResult> {
        // Find the node at the given position
        let node = self
            .db
            .find_node_at_position(project_id, file, line, column)?;

        match node {
            Some(n) => {
                // If this is a reference, find its target
                if let Some(target) = self.db.find_reference_target(n.id)? {
                    let file_info = self.db.get_file(target.file_id)?;
                    Ok(DefinitionResult {
                        found: true,
                        definition: Some(SymbolLocation {
                            file: file_info.map(|f| f.path).unwrap_or_default(),
                            line: target.start_line,
                            column: target.start_column,
                            node_type: target.node_type,
                            name: target.name,
                            qualified_name: target.qualified_name,
                            context: None,
                        }),
                    })
                } else {
                    // This might be the definition itself
                    let file_info = self.db.get_file(n.file_id)?;
                    Ok(DefinitionResult {
                        found: true,
                        definition: Some(SymbolLocation {
                            file: file_info.map(|f| f.path).unwrap_or_default(),
                            line: n.start_line,
                            column: n.start_column,
                            node_type: n.node_type,
                            name: n.name,
                            qualified_name: n.qualified_name,
                            context: None,
                        }),
                    })
                }
            }
            None => Ok(DefinitionResult {
                found: false,
                definition: None,
            }),
        }
    }

    /// Find all references to a symbol at the given location
    pub fn find_references(
        &self,
        project_id: i64,
        file: &str,
        line: u32,
        column: u32,
    ) -> Result<ReferencesResult> {
        let node = self
            .db
            .find_node_at_position(project_id, file, line, column)?;

        match node {
            Some(n) => {
                let refs = self.db.find_all_references(n.id)?;
                let mut references = Vec::new();

                for ref_node in refs {
                    let file_info = self.db.get_file(ref_node.file_id)?;
                    references.push(SymbolLocation {
                        file: file_info.map(|f| f.path).unwrap_or_default(),
                        line: ref_node.start_line,
                        column: ref_node.start_column,
                        node_type: ref_node.node_type,
                        name: ref_node.name,
                        qualified_name: ref_node.qualified_name,
                        context: None,
                    });
                }

                Ok(ReferencesResult {
                    count: references.len(),
                    references,
                })
            }
            None => Ok(ReferencesResult {
                count: 0,
                references: vec![],
            }),
        }
    }

    /// Get the call graph for a symbol
    pub fn get_callgraph(
        &self,
        project_id: i64,
        symbol: &str,
        depth: u32,
        direction: &str,
    ) -> Result<CallGraphResult> {
        let center_node = self.db.find_symbol_by_name(project_id, symbol)?;

        match center_node {
            Some(n) => {
                let file_info = self.db.get_file(n.file_id)?;
                let center = SymbolInfo {
                    name: n.name.clone(),
                    qualified_name: n.qualified_name.clone(),
                    node_type: n.node_type.clone(),
                    file: file_info.map(|f| f.path).unwrap_or_default(),
                    line: n.start_line,
                    column: n.start_column,
                };

                let callers = if direction == "callers" || direction == "both" {
                    self.collect_callers(n.id, depth)?
                } else {
                    vec![]
                };

                let callees = if direction == "callees" || direction == "both" {
                    self.collect_callees(n.id, depth)?
                } else {
                    vec![]
                };

                Ok(CallGraphResult {
                    center,
                    callers,
                    callees,
                })
            }
            None => Err(anyhow::anyhow!("Symbol not found: {}", symbol)),
        }
    }

    /// Search for symbols matching a query
    pub fn search_symbols(
        &self,
        project_id: i64,
        query: &str,
        symbol_type: Option<&str>,
        limit: u32,
    ) -> Result<SymbolSearchResult> {
        let nodes = self.db.search_symbols(project_id, query, symbol_type, limit)?;
        let mut symbols = Vec::new();

        for n in nodes {
            let file_info = self.db.get_file(n.file_id)?;
            symbols.push(SymbolInfo {
                name: n.name,
                qualified_name: n.qualified_name,
                node_type: n.node_type,
                file: file_info.map(|f| f.path).unwrap_or_default(),
                line: n.start_line,
                column: n.start_column,
            });
        }

        Ok(SymbolSearchResult {
            count: symbols.len(),
            symbols,
        })
    }

    fn collect_callers(&self, node_id: i64, depth: u32) -> Result<Vec<SymbolInfo>> {
        if depth == 0 {
            return Ok(vec![]);
        }

        let callers = self.db.find_callers(node_id)?;
        let mut result = Vec::new();

        for caller in callers {
            let file_info = self.db.get_file(caller.file_id)?;
            result.push(SymbolInfo {
                name: caller.name,
                qualified_name: caller.qualified_name,
                node_type: caller.node_type,
                file: file_info.map(|f| f.path).unwrap_or_default(),
                line: caller.start_line,
                column: caller.start_column,
            });
        }

        Ok(result)
    }

    fn collect_callees(&self, node_id: i64, depth: u32) -> Result<Vec<SymbolInfo>> {
        if depth == 0 {
            return Ok(vec![]);
        }

        let callees = self.db.find_callees(node_id)?;
        let mut result = Vec::new();

        for callee in callees {
            let file_info = self.db.get_file(callee.file_id)?;
            result.push(SymbolInfo {
                name: callee.name,
                qualified_name: callee.qualified_name,
                node_type: callee.node_type,
                file: file_info.map(|f| f.path).unwrap_or_default(),
                line: callee.start_line,
                column: callee.start_column,
            });
        }

        Ok(result)
    }
}

// Standalone functions for CLI usage (default project_id = 1)
pub fn find_definition(db_path: &Path, file: &Path, line: u32, column: u32) -> Result<DefinitionResult> {
    find_definition_with_project(db_path, 1, file, line, column)
}

pub fn find_references(db_path: &Path, file: &Path, line: u32, column: u32) -> Result<ReferencesResult> {
    find_references_with_project(db_path, 1, file, line, column)
}

pub fn get_callgraph(db_path: &Path, symbol: &str, depth: u32, direction: &str) -> Result<CallGraphResult> {
    get_callgraph_with_project(db_path, 1, symbol, depth, direction)
}

pub fn search_symbols(db_path: &Path, query: &str, symbol_type: Option<&str>, limit: u32) -> Result<SymbolSearchResult> {
    search_symbols_with_project(db_path, 1, query, symbol_type, limit)
}

// Standalone functions with explicit project_id
pub fn find_definition_with_project(
    db_path: &Path,
    project_id: i64,
    file: &Path,
    line: u32,
    column: u32,
) -> Result<DefinitionResult> {
    let db = Database::open(db_path)?;
    let executor = QueryExecutor::new(db);
    let file_str = file.to_string_lossy();
    executor.find_definition(project_id, &file_str, line, column)
}

pub fn find_references_with_project(
    db_path: &Path,
    project_id: i64,
    file: &Path,
    line: u32,
    column: u32,
) -> Result<ReferencesResult> {
    let db = Database::open(db_path)?;
    let executor = QueryExecutor::new(db);
    let file_str = file.to_string_lossy();
    executor.find_references(project_id, &file_str, line, column)
}

pub fn get_callgraph_with_project(
    db_path: &Path,
    project_id: i64,
    symbol: &str,
    depth: u32,
    direction: &str,
) -> Result<CallGraphResult> {
    let db = Database::open(db_path)?;
    let executor = QueryExecutor::new(db);
    executor.get_callgraph(project_id, symbol, depth, direction)
}

pub fn search_symbols_with_project(
    db_path: &Path,
    project_id: i64,
    query: &str,
    symbol_type: Option<&str>,
    limit: u32,
) -> Result<SymbolSearchResult> {
    let db = Database::open(db_path)?;
    let executor = QueryExecutor::new(db);
    executor.search_symbols(project_id, query, symbol_type, limit)
}

/// Find symbol definition by name
pub fn find_definition_by_symbol(
    db_path: &Path,
    project_id: i64,
    symbol: &str,
) -> Result<DefinitionResult> {
    let db = Database::open(db_path)?;

    // Search for the symbol definition (exclude call nodes)
    let nodes = db.search_symbols(project_id, symbol, None, 50)?;

    // Filter to definition types only (class, method, function, interface, struct, field)
    let definition_types = ["class", "method", "function", "interface", "struct", "field", "variable"];

    for node in nodes {
        if definition_types.contains(&node.node_type.as_str()) {
            // Check if name matches exactly or qualified_name matches
            let name_matches = node.name == symbol
                || node.qualified_name.as_ref().map(|q| q == symbol || q.ends_with(&format!(".{}", symbol))).unwrap_or(false);

            if name_matches {
                let file_info = db.get_file(node.file_id)?;
                return Ok(DefinitionResult {
                    found: true,
                    definition: Some(SymbolLocation {
                        file: file_info.map(|f| f.path).unwrap_or_default(),
                        line: node.start_line,
                        column: node.start_column,
                        node_type: node.node_type,
                        name: node.name,
                        qualified_name: node.qualified_name,
                        context: None,
                    }),
                });
            }
        }
    }

    Ok(DefinitionResult {
        found: false,
        definition: None,
    })
}

/// Find all references to a symbol by name (where the symbol is called/used)
pub fn find_references_by_symbol(
    db_path: &Path,
    project_id: i64,
    symbol: &str,
    limit: u32,
) -> Result<ReferencesResult> {
    let db = Database::open(db_path)?;

    // First find the symbol definition
    let target_node = db.find_symbol_by_name(project_id, symbol)?;

    match target_node {
        Some(node) => {
            // Find all callers (nodes that call this symbol)
            let callers = db.find_callers(node.id)?;

            let mut references = Vec::new();
            let mut count = 0;

            for caller in callers {
                if count >= limit {
                    break;
                }
                let file_info = db.get_file(caller.file_id)?;
                references.push(SymbolLocation {
                    file: file_info.map(|f| f.path).unwrap_or_default(),
                    line: caller.start_line,
                    column: caller.start_column,
                    node_type: caller.node_type,
                    name: caller.name,
                    qualified_name: caller.qualified_name,
                    context: None,
                });
                count += 1;
            }

            // Also find call nodes (method_invocation/call_expression) with matching name
            let call_nodes = db.search_symbols(project_id, symbol, Some("call"), limit)?;
            for call_node in call_nodes {
                if count >= limit {
                    break;
                }
                let file_info = db.get_file(call_node.file_id)?;
                references.push(SymbolLocation {
                    file: file_info.map(|f| f.path).unwrap_or_default(),
                    line: call_node.start_line,
                    column: call_node.start_column,
                    node_type: call_node.node_type,
                    name: call_node.name,
                    qualified_name: call_node.qualified_name,
                    context: None,
                });
                count += 1;
            }

            Ok(ReferencesResult {
                count: references.len(),
                references,
            })
        }
        None => {
            // Symbol definition not found, try searching for call nodes directly
            let call_nodes = db.search_symbols(project_id, symbol, Some("call"), limit)?;

            let mut references = Vec::new();
            for call_node in call_nodes {
                let file_info = db.get_file(call_node.file_id)?;
                references.push(SymbolLocation {
                    file: file_info.map(|f| f.path).unwrap_or_default(),
                    line: call_node.start_line,
                    column: call_node.start_column,
                    node_type: call_node.node_type,
                    name: call_node.name,
                    qualified_name: call_node.qualified_name,
                    context: None,
                });
            }

            Ok(ReferencesResult {
                count: references.len(),
                references,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::{EdgeRecord, FileRecord, NodeRecord, ProjectRecord};
    use crate::storage::Database;
    use tempfile::TempDir;

    fn setup_test_db() -> Database {
        let db = Database::open_in_memory().unwrap();
        db.init_schema().unwrap();
        db
    }

    fn create_test_project(db: &Database) -> i64 {
        let project = ProjectRecord {
            id: 0,
            name: "test-project".to_string(),
            root_path: "/test/project".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        db.insert_project(&project).unwrap()
    }

    fn create_test_file(db: &Database, project_id: i64, path: &str, language: &str) -> i64 {
        let file = FileRecord {
            id: 0,
            project_id,
            path: path.to_string(),
            language: language.to_string(),
            content_hash: "test_hash".to_string(),
            parsed_at: chrono::Utc::now(),
        };
        db.insert_file(&file).unwrap()
    }

    fn create_test_node(
        db: &Database,
        file_id: i64,
        node_type: &str,
        name: &str,
        qualified_name: Option<&str>,
        start_line: u32,
    ) -> i64 {
        let node = NodeRecord {
            id: 0,
            file_id,
            node_type: node_type.to_string(),
            name: name.to_string(),
            qualified_name: qualified_name.map(|s| s.to_string()),
            start_line,
            start_column: 1,
            end_line: start_line + 5,
            end_column: 1,
            attributes: None,
        };
        db.insert_node(&node).unwrap()
    }

    #[test]
    fn test_query_executor_new() {
        let db = setup_test_db();
        let executor = QueryExecutor::new(db);
        drop(executor);
    }

    #[test]
    fn test_find_definition_not_found() {
        let db = setup_test_db();
        let project_id = create_test_project(&db);
        let executor = QueryExecutor::new(db);

        let result = executor
            .find_definition(project_id, "/nonexistent/file.java", 10, 5)
            .unwrap();

        assert!(!result.found);
        assert!(result.definition.is_none());
    }

    #[test]
    fn test_find_definition_at_position() {
        let db = setup_test_db();
        let project_id = create_test_project(&db);
        let file_id = create_test_file(&db, project_id, "/test/UserService.java", "java");
        create_test_node(&db, file_id, "class", "UserService", Some("com.example.UserService"), 1);

        let executor = QueryExecutor::new(db);
        let result = executor
            .find_definition(project_id, "/test/UserService.java", 3, 1)
            .unwrap();

        assert!(result.found);
        assert!(result.definition.is_some());
        let def = result.definition.unwrap();
        assert_eq!(def.name, "UserService");
        assert_eq!(def.node_type, "class");
    }

    #[test]
    fn test_find_references_empty() {
        let db = setup_test_db();
        let project_id = create_test_project(&db);
        let executor = QueryExecutor::new(db);

        let result = executor
            .find_references(project_id, "/nonexistent/file.java", 10, 5)
            .unwrap();

        assert_eq!(result.count, 0);
        assert!(result.references.is_empty());
    }

    #[test]
    fn test_search_symbols() {
        let db = setup_test_db();
        let project_id = create_test_project(&db);
        let file_id = create_test_file(&db, project_id, "/test/Service.java", "java");

        create_test_node(&db, file_id, "class", "UserService", Some("com.example.UserService"), 1);
        create_test_node(&db, file_id, "method", "getUser", Some("com.example.UserService.getUser"), 10);
        create_test_node(&db, file_id, "method", "createUser", Some("com.example.UserService.createUser"), 20);

        let executor = QueryExecutor::new(db);
        let result = executor
            .search_symbols(project_id, "User", None, 10)
            .unwrap();

        assert!(result.count >= 2);
        assert!(result.symbols.iter().any(|s| s.name == "UserService"));
    }

    #[test]
    fn test_search_symbols_by_type() {
        let db = setup_test_db();
        let project_id = create_test_project(&db);
        let file_id = create_test_file(&db, project_id, "/test/Service.java", "java");

        create_test_node(&db, file_id, "class", "UserService", None, 1);
        create_test_node(&db, file_id, "method", "getUser", None, 10);

        let executor = QueryExecutor::new(db);

        // Search only for methods
        let result = executor
            .search_symbols(project_id, "User", Some("method"), 10)
            .unwrap();

        assert_eq!(result.count, 1);
        assert_eq!(result.symbols[0].name, "getUser");
        assert_eq!(result.symbols[0].node_type, "method");
    }

    #[test]
    fn test_search_symbols_with_limit() {
        let db = setup_test_db();
        let project_id = create_test_project(&db);
        let file_id = create_test_file(&db, project_id, "/test/Service.java", "java");

        for i in 0..10 {
            create_test_node(
                &db,
                file_id,
                "method",
                &format!("testMethod{}", i),
                None,
                (i * 10) as u32,
            );
        }

        let executor = QueryExecutor::new(db);
        let result = executor
            .search_symbols(project_id, "testMethod", None, 5)
            .unwrap();

        assert_eq!(result.count, 5);
    }

    #[test]
    fn test_get_callgraph_symbol_not_found() {
        let db = setup_test_db();
        let project_id = create_test_project(&db);
        let executor = QueryExecutor::new(db);

        let result = executor.get_callgraph(project_id, "nonExistentSymbol", 2, "both");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Symbol not found"));
    }

    #[test]
    fn test_get_callgraph_basic() {
        let db = setup_test_db();
        let project_id = create_test_project(&db);
        let file_id = create_test_file(&db, project_id, "/test/main.go", "go");

        let main_id = create_test_node(&db, file_id, "function", "main", Some("main.main"), 1);
        let helper_id = create_test_node(&db, file_id, "function", "helper", Some("main.helper"), 20);

        // main calls helper
        let edge = EdgeRecord {
            id: 0,
            source_id: main_id,
            target_id: helper_id,
            edge_type: "calls".to_string(),
            attributes: None,
        };
        db.insert_edge(&edge).unwrap();

        let executor = QueryExecutor::new(db);
        let result = executor
            .get_callgraph(project_id, "main", 1, "both")
            .unwrap();

        assert_eq!(result.center.name, "main");
        // The callees should include helper
        assert!(result.callees.iter().any(|c| c.name == "helper"));
    }

    #[test]
    fn test_callgraph_direction_callers() {
        let db = setup_test_db();
        let project_id = create_test_project(&db);
        let file_id = create_test_file(&db, project_id, "/test/main.go", "go");

        let main_id = create_test_node(&db, file_id, "function", "main", Some("main.main"), 1);
        let helper_id = create_test_node(&db, file_id, "function", "helper", Some("main.helper"), 20);

        let edge = EdgeRecord {
            id: 0,
            source_id: main_id,
            target_id: helper_id,
            edge_type: "calls".to_string(),
            attributes: None,
        };
        db.insert_edge(&edge).unwrap();

        let executor = QueryExecutor::new(db);
        let result = executor
            .get_callgraph(project_id, "helper", 1, "callers")
            .unwrap();

        assert_eq!(result.center.name, "helper");
        assert!(result.callees.is_empty());
    }

    #[test]
    fn test_callgraph_direction_callees() {
        let db = setup_test_db();
        let project_id = create_test_project(&db);
        let file_id = create_test_file(&db, project_id, "/test/main.go", "go");

        let main_id = create_test_node(&db, file_id, "function", "main", Some("main.main"), 1);
        let helper_id = create_test_node(&db, file_id, "function", "helper", Some("main.helper"), 20);

        let edge = EdgeRecord {
            id: 0,
            source_id: main_id,
            target_id: helper_id,
            edge_type: "calls".to_string(),
            attributes: None,
        };
        db.insert_edge(&edge).unwrap();

        let executor = QueryExecutor::new(db);
        let result = executor
            .get_callgraph(project_id, "main", 1, "callees")
            .unwrap();

        assert_eq!(result.center.name, "main");
        assert!(result.callers.is_empty());
        assert!(result.callees.iter().any(|c| c.name == "helper"));
    }

    #[test]
    fn test_callgraph_depth_zero() {
        let db = setup_test_db();
        let project_id = create_test_project(&db);
        let file_id = create_test_file(&db, project_id, "/test/main.go", "go");

        let main_id = create_test_node(&db, file_id, "function", "main", Some("main.main"), 1);
        let helper_id = create_test_node(&db, file_id, "function", "helper", Some("main.helper"), 20);

        let edge = EdgeRecord {
            id: 0,
            source_id: main_id,
            target_id: helper_id,
            edge_type: "calls".to_string(),
            attributes: None,
        };
        db.insert_edge(&edge).unwrap();

        let executor = QueryExecutor::new(db);
        let result = executor
            .get_callgraph(project_id, "main", 0, "both")
            .unwrap();

        assert_eq!(result.center.name, "main");
        assert!(result.callers.is_empty());
        assert!(result.callees.is_empty());
    }

    #[test]
    fn test_definition_result_serialization() {
        let result = DefinitionResult {
            found: true,
            definition: Some(SymbolLocation {
                file: "/test/file.java".to_string(),
                line: 10,
                column: 5,
                node_type: "class".to_string(),
                name: "TestClass".to_string(),
                qualified_name: Some("com.example.TestClass".to_string()),
                context: None,
            }),
        };

        let json = serde_json::to_string(&result).unwrap();
        let parsed: DefinitionResult = serde_json::from_str(&json).unwrap();

        assert!(parsed.found);
        assert_eq!(parsed.definition.unwrap().name, "TestClass");
    }

    #[test]
    fn test_references_result_serialization() {
        let result = ReferencesResult {
            count: 1,
            references: vec![SymbolLocation {
                file: "/test/file.java".to_string(),
                line: 10,
                column: 5,
                node_type: "reference".to_string(),
                name: "TestClass".to_string(),
                qualified_name: None,
                context: None,
            }],
        };

        let json = serde_json::to_string(&result).unwrap();
        let parsed: ReferencesResult = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.count, 1);
        assert_eq!(parsed.references.len(), 1);
    }

    #[test]
    fn test_callgraph_result_serialization() {
        let result = CallGraphResult {
            center: SymbolInfo {
                name: "main".to_string(),
                qualified_name: Some("main.main".to_string()),
                node_type: "function".to_string(),
                file: "/test/main.go".to_string(),
                line: 1,
                column: 1,
            },
            callers: vec![],
            callees: vec![SymbolInfo {
                name: "helper".to_string(),
                qualified_name: None,
                node_type: "function".to_string(),
                file: "/test/main.go".to_string(),
                line: 20,
                column: 1,
            }],
        };

        let json = serde_json::to_string(&result).unwrap();
        let parsed: CallGraphResult = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.center.name, "main");
        assert_eq!(parsed.callees.len(), 1);
    }

    #[test]
    fn test_symbol_search_result_serialization() {
        let result = SymbolSearchResult {
            count: 2,
            symbols: vec![
                SymbolInfo {
                    name: "func1".to_string(),
                    qualified_name: None,
                    node_type: "function".to_string(),
                    file: "/test.go".to_string(),
                    line: 1,
                    column: 1,
                },
                SymbolInfo {
                    name: "func2".to_string(),
                    qualified_name: None,
                    node_type: "function".to_string(),
                    file: "/test.go".to_string(),
                    line: 10,
                    column: 1,
                },
            ],
        };

        let json = serde_json::to_string(&result).unwrap();
        let parsed: SymbolSearchResult = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.count, 2);
        assert_eq!(parsed.symbols.len(), 2);
    }

    #[test]
    fn test_find_definition_by_symbol() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let db = Database::open(&db_path).unwrap();
        db.init_schema().unwrap();
        let project_id = create_test_project(&db);
        let file_id = create_test_file(&db, project_id, "/test/Service.java", "java");
        create_test_node(&db, file_id, "class", "UserService", Some("com.example.UserService"), 1);
        drop(db);

        let result = find_definition_by_symbol(&db_path, project_id, "UserService").unwrap();

        assert!(result.found);
        let def = result.definition.unwrap();
        assert_eq!(def.name, "UserService");
    }

    #[test]
    fn test_find_references_by_symbol() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let db = Database::open(&db_path).unwrap();
        db.init_schema().unwrap();
        let project_id = create_test_project(&db);
        let file_id = create_test_file(&db, project_id, "/test/Service.java", "java");

        let method_id = create_test_node(&db, file_id, "method", "getUser", Some("UserService.getUser"), 10);
        let call_id = create_test_node(&db, file_id, "call", "getUser", None, 30);

        // Create a calls edge
        let edge = EdgeRecord {
            id: 0,
            source_id: call_id,
            target_id: method_id,
            edge_type: "calls".to_string(),
            attributes: None,
        };
        db.insert_edge(&edge).unwrap();
        drop(db);

        let result = find_references_by_symbol(&db_path, project_id, "getUser", 10).unwrap();

        assert!(result.count >= 1);
    }

    #[test]
    fn test_standalone_find_definition() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let db = Database::open(&db_path).unwrap();
        db.init_schema().unwrap();
        let project_id = create_test_project(&db);
        let file_id = create_test_file(&db, project_id, "/test/file.java", "java");
        create_test_node(&db, file_id, "class", "Test", None, 1);
        drop(db);

        let result = find_definition(&db_path, Path::new("/test/file.java"), 3, 1).unwrap();
        assert!(result.found);
    }

    #[test]
    fn test_standalone_search_symbols() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let db = Database::open(&db_path).unwrap();
        db.init_schema().unwrap();
        let project_id = create_test_project(&db);
        let file_id = create_test_file(&db, project_id, "/test/file.java", "java");
        create_test_node(&db, file_id, "class", "TestClass", None, 1);
        drop(db);

        let result = search_symbols(&db_path, "Test", None, 10).unwrap();
        assert!(result.count > 0);
    }
}
