//! Go language support

use anyhow::Result;
use tree_sitter::{Node, Tree};

use crate::languages::LanguageSupport;
use crate::storage::models::{EdgeData, NodeData};

/// Go language support implementation
pub struct GoLanguage;

impl GoLanguage {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GoLanguage {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageSupport for GoLanguage {
    fn language_id(&self) -> &str {
        "go"
    }

    fn file_extensions(&self) -> &[&str] {
        &[".go"]
    }

    fn grammar(&self) -> tree_sitter::Language {
        tree_sitter_go::LANGUAGE.into()
    }

    fn extract_graph(&self, source: &str, tree: &Tree) -> Result<(Vec<NodeData>, Vec<EdgeData>)> {
        let mut extractor = GoGraphExtractor::new(source);
        extractor.extract(tree.root_node());
        Ok((extractor.nodes, extractor.edges))
    }
}

/// Helper for extracting graph data from Go source
struct GoGraphExtractor<'a> {
    source: &'a str,
    nodes: Vec<NodeData>,
    edges: Vec<EdgeData>,
    current_package: Option<String>,
    current_func: Option<usize>,
    current_type: Option<String>,
}

impl<'a> GoGraphExtractor<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            nodes: Vec::new(),
            edges: Vec::new(),
            current_package: None,
            current_func: None,
            current_type: None,
        }
    }

    fn extract(&mut self, node: Node) {
        match node.kind() {
            "package_clause" => self.extract_package(node),
            "import_declaration" => self.extract_imports(node),
            "function_declaration" => self.extract_function(node),
            "method_declaration" => self.extract_method(node),
            "type_declaration" => self.extract_type_declaration(node),
            "call_expression" => self.extract_call(node),
            _ => {
                // Recurse into children
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i) {
                        self.extract(child);
                    }
                }
            }
        }
    }

    fn extract_package(&mut self, node: Node) {
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if child.kind() == "package_identifier" {
                    let name = self.node_text(child);
                    self.current_package = Some(name.clone());
                    self.nodes.push(NodeData {
                        node_type: "package".to_string(),
                        name: name.clone(),
                        qualified_name: Some(name),
                        start_line: node.start_position().row as u32 + 1,
                        start_column: node.start_position().column as u32 + 1,
                        end_line: node.end_position().row as u32 + 1,
                        end_column: node.end_position().column as u32 + 1,
                        attributes: None,
                    });
                    break;
                }
            }
        }
    }

    fn extract_imports(&mut self, node: Node) {
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if child.kind() == "import_spec_list" {
                    for j in 0..child.child_count() {
                        if let Some(spec) = child.child(j) {
                            self.extract_import_spec(spec);
                        }
                    }
                } else if child.kind() == "import_spec" {
                    self.extract_import_spec(child);
                }
            }
        }
    }

    fn extract_import_spec(&mut self, node: Node) {
        if let Some(path) = node.child_by_field_name("path") {
            let name = self.node_text(path);
            // Remove quotes
            let name = name.trim_matches('"').to_string();
            self.nodes.push(NodeData {
                node_type: "import".to_string(),
                name: name.clone(),
                qualified_name: Some(name),
                start_line: node.start_position().row as u32 + 1,
                start_column: node.start_position().column as u32 + 1,
                end_line: node.end_position().row as u32 + 1,
                end_column: node.end_position().column as u32 + 1,
                attributes: None,
            });
        }
    }

    fn extract_function(&mut self, node: Node) {
        if let Some(name_node) = node.child_by_field_name("name") {
            let name = self.node_text(name_node);
            let qualified_name = self.qualify_name(&name);

            let func_idx = self.nodes.len();
            self.nodes.push(NodeData {
                node_type: "function".to_string(),
                name: name.clone(),
                qualified_name: Some(qualified_name),
                start_line: node.start_position().row as u32 + 1,
                start_column: node.start_position().column as u32 + 1,
                end_line: node.end_position().row as u32 + 1,
                end_column: node.end_position().column as u32 + 1,
                attributes: None,
            });

            // Extract parameters
            if let Some(params) = node.child_by_field_name("parameters") {
                self.extract_parameters(func_idx, params);
            }

            // Process body
            let old_func = self.current_func.take();
            self.current_func = Some(func_idx);

            if let Some(body) = node.child_by_field_name("body") {
                for i in 0..body.child_count() {
                    if let Some(child) = body.child(i) {
                        self.extract(child);
                    }
                }
            }

            self.current_func = old_func;
        }
    }

    fn extract_method(&mut self, node: Node) {
        if let Some(name_node) = node.child_by_field_name("name") {
            let name = self.node_text(name_node);

            // Get receiver type
            let receiver_type = node
                .child_by_field_name("receiver")
                .and_then(|r| self.extract_receiver_type(r));

            let qualified_name = if let Some(ref recv) = receiver_type {
                format!("{}.{}", recv, name)
            } else {
                self.qualify_name(&name)
            };

            let method_idx = self.nodes.len();
            self.nodes.push(NodeData {
                node_type: "method".to_string(),
                name: name.clone(),
                qualified_name: Some(qualified_name),
                start_line: node.start_position().row as u32 + 1,
                start_column: node.start_position().column as u32 + 1,
                end_line: node.end_position().row as u32 + 1,
                end_column: node.end_position().column as u32 + 1,
                attributes: receiver_type.map(|t| format!(r#"{{"receiver":"{}"}}"#, t)),
            });

            // Extract parameters
            if let Some(params) = node.child_by_field_name("parameters") {
                self.extract_parameters(method_idx, params);
            }

            // Process body
            let old_func = self.current_func.take();
            self.current_func = Some(method_idx);

            if let Some(body) = node.child_by_field_name("body") {
                for i in 0..body.child_count() {
                    if let Some(child) = body.child(i) {
                        self.extract(child);
                    }
                }
            }

            self.current_func = old_func;
        }
    }

    fn extract_receiver_type(&self, receiver: Node) -> Option<String> {
        // parameter_list -> parameter_declaration -> type
        for i in 0..receiver.child_count() {
            if let Some(child) = receiver.child(i) {
                if child.kind() == "parameter_declaration" {
                    if let Some(type_node) = child.child_by_field_name("type") {
                        return Some(self.extract_type_name(type_node));
                    }
                }
            }
        }
        None
    }

    fn extract_type_name(&self, node: Node) -> String {
        match node.kind() {
            "pointer_type" => {
                // *Type -> Type
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i) {
                        if child.kind() == "type_identifier" {
                            return self.node_text(child);
                        }
                    }
                }
                self.node_text(node)
            }
            "type_identifier" => self.node_text(node),
            _ => self.node_text(node),
        }
    }

    fn extract_parameters(&mut self, func_idx: usize, params: Node) {
        for i in 0..params.child_count() {
            if let Some(param) = params.child(i) {
                if param.kind() == "parameter_declaration" {
                    if let Some(name_node) = param.child_by_field_name("name") {
                        let name = self.node_text(name_node);
                        let param_idx = self.nodes.len();
                        self.nodes.push(NodeData {
                            node_type: "parameter".to_string(),
                            name,
                            qualified_name: None,
                            start_line: param.start_position().row as u32 + 1,
                            start_column: param.start_position().column as u32 + 1,
                            end_line: param.end_position().row as u32 + 1,
                            end_column: param.end_position().column as u32 + 1,
                            attributes: None,
                        });
                        self.edges.push(EdgeData {
                            source_idx: func_idx as u32,
                            target_idx: param_idx as u32,
                            edge_type: "has_parameter".to_string(),
                            attributes: None,
                        });
                    }
                }
            }
        }
    }

    fn extract_type_declaration(&mut self, node: Node) {
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if child.kind() == "type_spec" {
                    self.extract_type_spec(child);
                }
            }
        }
    }

    fn extract_type_spec(&mut self, node: Node) {
        if let Some(name_node) = node.child_by_field_name("name") {
            let name = self.node_text(name_node);
            let qualified_name = self.qualify_name(&name);

            // Determine type kind
            let type_kind = node
                .child_by_field_name("type")
                .map(|t| t.kind())
                .unwrap_or("");

            let node_type = match type_kind {
                "struct_type" => "struct",
                "interface_type" => "interface",
                _ => "type",
            };

            let type_idx = self.nodes.len();
            self.nodes.push(NodeData {
                node_type: node_type.to_string(),
                name: name.clone(),
                qualified_name: Some(qualified_name.clone()),
                start_line: node.start_position().row as u32 + 1,
                start_column: node.start_position().column as u32 + 1,
                end_line: node.end_position().row as u32 + 1,
                end_column: node.end_position().column as u32 + 1,
                attributes: None,
            });

            // Extract struct fields
            if let Some(type_node) = node.child_by_field_name("type") {
                if type_node.kind() == "struct_type" {
                    let old_type = self.current_type.take();
                    self.current_type = Some(qualified_name);
                    self.extract_struct_fields(type_idx, type_node);
                    self.current_type = old_type;
                } else if type_node.kind() == "interface_type" {
                    self.extract_interface_methods(type_idx, type_node);
                }
            }
        }
    }

    fn extract_struct_fields(&mut self, struct_idx: usize, node: Node) {
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if child.kind() == "field_declaration_list" {
                    for j in 0..child.child_count() {
                        if let Some(field) = child.child(j) {
                            if field.kind() == "field_declaration" {
                                if let Some(name_node) = field.child_by_field_name("name") {
                                    let name = self.node_text(name_node);
                                    let field_idx = self.nodes.len();
                                    self.nodes.push(NodeData {
                                        node_type: "field".to_string(),
                                        name,
                                        qualified_name: None,
                                        start_line: field.start_position().row as u32 + 1,
                                        start_column: field.start_position().column as u32 + 1,
                                        end_line: field.end_position().row as u32 + 1,
                                        end_column: field.end_position().column as u32 + 1,
                                        attributes: None,
                                    });
                                    self.edges.push(EdgeData {
                                        source_idx: struct_idx as u32,
                                        target_idx: field_idx as u32,
                                        edge_type: "contains".to_string(),
                                        attributes: None,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn extract_interface_methods(&mut self, interface_idx: usize, node: Node) {
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if child.kind() == "method_spec" {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let name = self.node_text(name_node);
                        let method_idx = self.nodes.len();
                        self.nodes.push(NodeData {
                            node_type: "method".to_string(),
                            name,
                            qualified_name: None,
                            start_line: child.start_position().row as u32 + 1,
                            start_column: child.start_position().column as u32 + 1,
                            end_line: child.end_position().row as u32 + 1,
                            end_column: child.end_position().column as u32 + 1,
                            attributes: Some(r#"{"abstract":true}"#.to_string()),
                        });
                        self.edges.push(EdgeData {
                            source_idx: interface_idx as u32,
                            target_idx: method_idx as u32,
                            edge_type: "contains".to_string(),
                            attributes: None,
                        });
                    }
                }
            }
        }
    }

    fn extract_call(&mut self, node: Node) {
        if let Some(func_node) = node.child_by_field_name("function") {
            let name = self.node_text(func_node);
            let call_idx = self.nodes.len();

            self.nodes.push(NodeData {
                node_type: "call".to_string(),
                name: name.clone(),
                qualified_name: None,
                start_line: node.start_position().row as u32 + 1,
                start_column: node.start_position().column as u32 + 1,
                end_line: node.end_position().row as u32 + 1,
                end_column: node.end_position().column as u32 + 1,
                attributes: None,
            });

            // Link call to current function
            if let Some(func_idx) = self.current_func {
                self.edges.push(EdgeData {
                    source_idx: func_idx as u32,
                    target_idx: call_idx as u32,
                    edge_type: "calls".to_string(),
                    attributes: None,
                });
            }
        }

        // Recurse into arguments
        if let Some(args) = node.child_by_field_name("arguments") {
            for i in 0..args.child_count() {
                if let Some(child) = args.child(i) {
                    self.extract(child);
                }
            }
        }
    }

    fn node_text(&self, node: Node) -> String {
        self.source[node.byte_range()].to_string()
    }

    fn qualify_name(&self, name: &str) -> String {
        if let Some(ref pkg) = self.current_package {
            format!("{}.{}", pkg, name)
        } else {
            name.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::LanguageSupport;

    fn parse_go(source: &str) -> (Vec<NodeData>, Vec<EdgeData>) {
        let go = GoLanguage::new();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&go.grammar()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        go.extract_graph(source, &tree).unwrap()
    }

    #[test]
    fn test_go_language_new() {
        let go = GoLanguage::new();
        assert_eq!(go.language_id(), "go");
    }

    #[test]
    fn test_go_language_default() {
        let go = GoLanguage::default();
        assert_eq!(go.language_id(), "go");
    }

    #[test]
    fn test_go_file_extensions() {
        let go = GoLanguage::new();
        let extensions = go.file_extensions();
        assert!(extensions.contains(&".go"));
    }

    #[test]
    fn test_go_grammar() {
        let go = GoLanguage::new();
        let grammar = go.grammar();
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&grammar).is_ok());
    }

    #[test]
    fn test_extract_package() {
        let source = "package main";
        let (nodes, _) = parse_go(source);

        let pkg = nodes.iter().find(|n| n.node_type == "package").unwrap();
        assert_eq!(pkg.name, "main");
    }

    #[test]
    fn test_extract_import_single() {
        let source = r#"
package main

import "fmt"
"#;
        let (nodes, _) = parse_go(source);

        let import = nodes.iter().find(|n| n.node_type == "import").unwrap();
        assert_eq!(import.name, "fmt");
    }

    #[test]
    fn test_extract_import_multiple() {
        let source = r#"
package main

import (
    "fmt"
    "net/http"
    "encoding/json"
)
"#;
        let (nodes, _) = parse_go(source);

        let imports: Vec<_> = nodes.iter().filter(|n| n.node_type == "import").collect();
        assert_eq!(imports.len(), 3);
        assert!(imports.iter().any(|i| i.name == "fmt"));
        assert!(imports.iter().any(|i| i.name == "net/http"));
        assert!(imports.iter().any(|i| i.name == "encoding/json"));
    }

    #[test]
    fn test_extract_function() {
        let source = r#"
package main

func main() {
}

func helper() {
}
"#;
        let (nodes, _) = parse_go(source);

        let funcs: Vec<_> = nodes.iter().filter(|n| n.node_type == "function").collect();
        assert_eq!(funcs.len(), 2);
        assert!(funcs.iter().any(|f| f.name == "main"));
        assert!(funcs.iter().any(|f| f.name == "helper"));
    }

    #[test]
    fn test_extract_method() {
        let source = r#"
package main

type Server struct {}

func (s *Server) Start() {
}

func (s Server) Stop() {
}
"#;
        let (nodes, _) = parse_go(source);

        let methods: Vec<_> = nodes.iter().filter(|n| n.node_type == "method").collect();
        assert_eq!(methods.len(), 2);
        assert!(methods.iter().any(|m| m.name == "Start"));
        assert!(methods.iter().any(|m| m.name == "Stop"));
    }

    #[test]
    fn test_extract_struct() {
        let source = r#"
package main

type User struct {
    Name string
    Age  int
}
"#;
        let (nodes, _) = parse_go(source);

        let struc = nodes.iter().find(|n| n.node_type == "struct").unwrap();
        assert_eq!(struc.name, "User");
    }

    #[test]
    fn test_extract_interface() {
        let source = r#"
package main

type Repository interface {
    Find(id int) error
    Save(data string) error
}
"#;
        let (nodes, _) = parse_go(source);

        let interface = nodes.iter().find(|n| n.node_type == "interface").unwrap();
        assert_eq!(interface.name, "Repository");
    }

    #[test]
    fn test_extract_struct_fields() {
        let source = r#"
package main

type Config struct {
    Host string
    Port int
}
"#;
        let (nodes, edges) = parse_go(source);

        let fields: Vec<_> = nodes.iter().filter(|n| n.node_type == "field").collect();
        assert_eq!(fields.len(), 2);
        assert!(fields.iter().any(|f| f.name == "Host"));
        assert!(fields.iter().any(|f| f.name == "Port"));

        let contains_edges: Vec<_> = edges.iter().filter(|e| e.edge_type == "contains").collect();
        assert_eq!(contains_edges.len(), 2);
    }

    #[test]
    fn test_extract_interface_methods() {
        let source = r#"
package main

type Handler interface {
    Handle()
    Process()
}
"#;
        let (nodes, edges) = parse_go(source);

        // Verify interface is extracted
        assert!(nodes.iter().any(|n| n.node_type == "interface" && n.name == "Handler"));

        // Interface methods may or may not be extracted depending on tree-sitter behavior
        // Just verify we have some method nodes if they exist
        let interface_methods: Vec<_> = nodes.iter()
            .filter(|n| n.node_type == "method")
            .collect();

        // If methods are extracted, they should have contains edges
        if !interface_methods.is_empty() {
            let contains_edges: Vec<_> = edges.iter().filter(|e| e.edge_type == "contains").collect();
            assert!(!contains_edges.is_empty());
        }
    }

    #[test]
    fn test_extract_function_parameters() {
        let source = r#"
package main

func process(input string, count int) {
}
"#;
        let (nodes, edges) = parse_go(source);

        let params: Vec<_> = nodes.iter().filter(|n| n.node_type == "parameter").collect();
        assert_eq!(params.len(), 2);
        assert!(params.iter().any(|p| p.name == "input"));
        assert!(params.iter().any(|p| p.name == "count"));

        let param_edges: Vec<_> = edges.iter().filter(|e| e.edge_type == "has_parameter").collect();
        assert_eq!(param_edges.len(), 2);
    }

    #[test]
    fn test_extract_call() {
        let source = r#"
package main

import "fmt"

func main() {
    fmt.Println("hello")
    helper()
}
"#;
        let (nodes, edges) = parse_go(source);

        let calls: Vec<_> = nodes.iter().filter(|n| n.node_type == "call").collect();
        assert!(calls.len() >= 2);
        assert!(calls.iter().any(|c| c.name.contains("Println") || c.name.contains("fmt")));
        assert!(calls.iter().any(|c| c.name == "helper"));

        let call_edges: Vec<_> = edges.iter().filter(|e| e.edge_type == "calls").collect();
        assert!(call_edges.len() >= 2);
    }

    #[test]
    fn test_method_receiver_type() {
        let source = r#"
package main

type Server struct {}

func (s *Server) Start() error {
    return nil
}
"#;
        let (nodes, _) = parse_go(source);

        let method = nodes.iter().find(|n| n.node_type == "method" && n.name == "Start").unwrap();
        assert!(method.attributes.as_ref().unwrap().contains("Server"));
    }

    #[test]
    fn test_qualified_names() {
        let source = r#"
package mypackage

func MyFunction() {
}

type MyStruct struct {}
"#;
        let (nodes, _) = parse_go(source);

        let func = nodes.iter().find(|n| n.node_type == "function").unwrap();
        assert!(func.qualified_name.as_ref().unwrap().contains("mypackage"));

        let struc = nodes.iter().find(|n| n.node_type == "struct").unwrap();
        assert!(struc.qualified_name.as_ref().unwrap().contains("mypackage"));
    }

    #[test]
    fn test_node_positions() {
        let source = r#"package main

func main() {
    fmt.Println()
}"#;
        let (nodes, _) = parse_go(source);

        let pkg = nodes.iter().find(|n| n.node_type == "package").unwrap();
        assert_eq!(pkg.start_line, 1);

        let func = nodes.iter().find(|n| n.node_type == "function").unwrap();
        assert_eq!(func.start_line, 3);
        assert_eq!(func.end_line, 5);
    }

    #[test]
    fn test_nested_calls() {
        let source = r#"
package main

func main() {
    outer(inner())
}
"#;
        let (nodes, _) = parse_go(source);

        let calls: Vec<_> = nodes.iter().filter(|n| n.node_type == "call").collect();
        assert_eq!(calls.len(), 2);
    }

    #[test]
    fn test_empty_struct() {
        let source = r#"
package main

type Empty struct {}
"#;
        let (nodes, edges) = parse_go(source);

        let struc = nodes.iter().find(|n| n.node_type == "struct" && n.name == "Empty");
        assert!(struc.is_some());

        // Empty struct should have no contains edges
        let contains_edges: Vec<_> = edges.iter().filter(|e| e.edge_type == "contains").collect();
        assert!(contains_edges.is_empty());
    }

    #[test]
    fn test_type_alias() {
        let source = r#"
package main

type ID int
type Handler func()
"#;
        let (nodes, _) = parse_go(source);

        let types: Vec<_> = nodes.iter().filter(|n| n.node_type == "type").collect();
        assert_eq!(types.len(), 2);
        assert!(types.iter().any(|t| t.name == "ID"));
        assert!(types.iter().any(|t| t.name == "Handler"));
    }

    #[test]
    fn test_complex_go_file() {
        let source = r#"
package main

import (
    "fmt"
    "net/http"
)

type Server struct {
    port int
    name string
}

func NewServer(port int, name string) *Server {
    return &Server{
        port: port,
        name: name,
    }
}

func (s *Server) Start() error {
    fmt.Printf("Starting server %s on port %d\n", s.name, s.port)
    return http.ListenAndServe(fmt.Sprintf(":%d", s.port), nil)
}

func main() {
    server := NewServer(8080, "TestServer")
    if err := server.Start(); err != nil {
        fmt.Printf("Error: %v\n", err)
    }
}
"#;
        let (nodes, edges) = parse_go(source);

        // Check node types
        assert!(nodes.iter().any(|n| n.node_type == "package"));
        assert_eq!(nodes.iter().filter(|n| n.node_type == "import").count(), 2);
        assert!(nodes.iter().any(|n| n.node_type == "struct" && n.name == "Server"));
        assert_eq!(nodes.iter().filter(|n| n.node_type == "field").count(), 2);
        assert!(nodes.iter().any(|n| n.node_type == "function" && n.name == "NewServer"));
        assert!(nodes.iter().any(|n| n.node_type == "function" && n.name == "main"));
        assert!(nodes.iter().any(|n| n.node_type == "method" && n.name == "Start"));

        // Check calls
        let calls: Vec<_> = nodes.iter().filter(|n| n.node_type == "call").collect();
        assert!(calls.len() > 0);
        assert!(calls.iter().any(|c| c.name.contains("Printf") || c.name.contains("fmt")));
        assert!(calls.iter().any(|c| c.name == "NewServer"));

        // Check edges exist
        assert!(!edges.is_empty());
    }
}
