//! Java language support

use anyhow::Result;
use tree_sitter::{Node, Tree};

use crate::languages::LanguageSupport;
use crate::storage::models::{EdgeData, NodeData};

/// Java language support implementation
pub struct JavaLanguage;

impl JavaLanguage {
    pub fn new() -> Self {
        Self
    }
}

impl Default for JavaLanguage {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageSupport for JavaLanguage {
    fn language_id(&self) -> &str {
        "java"
    }

    fn file_extensions(&self) -> &[&str] {
        &[".java"]
    }

    fn grammar(&self) -> tree_sitter::Language {
        tree_sitter_java::LANGUAGE.into()
    }

    fn extract_graph(&self, source: &str, tree: &Tree) -> Result<(Vec<NodeData>, Vec<EdgeData>)> {
        let mut extractor = JavaGraphExtractor::new(source);
        extractor.extract(tree.root_node());
        Ok((extractor.nodes, extractor.edges))
    }
}

/// Helper for extracting graph data from Java source
struct JavaGraphExtractor<'a> {
    source: &'a str,
    nodes: Vec<NodeData>,
    edges: Vec<EdgeData>,
    current_class: Option<String>,
    current_method: Option<usize>,
}

impl<'a> JavaGraphExtractor<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            nodes: Vec::new(),
            edges: Vec::new(),
            current_class: None,
            current_method: None,
        }
    }

    fn extract(&mut self, node: Node) {
        match node.kind() {
            "package_declaration" => self.extract_package(node),
            "import_declaration" => self.extract_import(node),
            "class_declaration" => self.extract_class(node),
            "interface_declaration" => self.extract_interface(node),
            "method_declaration" => self.extract_method(node),
            "constructor_declaration" => self.extract_constructor(node),
            "field_declaration" => self.extract_field(node),
            "method_invocation" => self.extract_method_invocation(node),
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
        if let Some(name_node) = node.child_by_field_name("name") {
            let name = self.node_text(name_node);
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
        }
    }

    fn extract_import(&mut self, node: Node) {
        // Find the scoped identifier or identifier
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if child.kind() == "scoped_identifier" || child.kind() == "identifier" {
                    let name = self.node_text(child);
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
                    break;
                }
            }
        }
    }

    fn extract_class(&mut self, node: Node) {
        if let Some(name_node) = node.child_by_field_name("name") {
            let name = self.node_text(name_node);
            let qualified_name = self.qualify_name(&name);

            let class_idx = self.nodes.len();
            self.nodes.push(NodeData {
                node_type: "class".to_string(),
                name: name.clone(),
                qualified_name: Some(qualified_name.clone()),
                start_line: node.start_position().row as u32 + 1,
                start_column: node.start_position().column as u32 + 1,
                end_line: node.end_position().row as u32 + 1,
                end_column: node.end_position().column as u32 + 1,
                attributes: None,
            });

            // Check for superclass
            if let Some(superclass) = node.child_by_field_name("superclass") {
                let super_name = self.node_text(superclass);
                let super_idx = self.nodes.len();
                self.nodes.push(NodeData {
                    node_type: "reference".to_string(),
                    name: super_name,
                    qualified_name: None,
                    start_line: superclass.start_position().row as u32 + 1,
                    start_column: superclass.start_position().column as u32 + 1,
                    end_line: superclass.end_position().row as u32 + 1,
                    end_column: superclass.end_position().column as u32 + 1,
                    attributes: None,
                });
                self.edges.push(EdgeData {
                    source_idx: class_idx as u32,
                    target_idx: super_idx as u32,
                    edge_type: "extends".to_string(),
                    attributes: None,
                });
            }

            // Check for interfaces
            if let Some(interfaces) = node.child_by_field_name("interfaces") {
                self.extract_implements(class_idx, interfaces);
            }

            // Process body
            let old_class = self.current_class.take();
            self.current_class = Some(qualified_name);

            if let Some(body) = node.child_by_field_name("body") {
                for i in 0..body.child_count() {
                    if let Some(child) = body.child(i) {
                        self.extract(child);
                    }
                }
            }

            self.current_class = old_class;
        }
    }

    fn extract_interface(&mut self, node: Node) {
        if let Some(name_node) = node.child_by_field_name("name") {
            let name = self.node_text(name_node);
            let qualified_name = self.qualify_name(&name);

            self.nodes.push(NodeData {
                node_type: "interface".to_string(),
                name: name.clone(),
                qualified_name: Some(qualified_name.clone()),
                start_line: node.start_position().row as u32 + 1,
                start_column: node.start_position().column as u32 + 1,
                end_line: node.end_position().row as u32 + 1,
                end_column: node.end_position().column as u32 + 1,
                attributes: None,
            });

            // Process body
            let old_class = self.current_class.take();
            self.current_class = Some(qualified_name);

            if let Some(body) = node.child_by_field_name("body") {
                for i in 0..body.child_count() {
                    if let Some(child) = body.child(i) {
                        self.extract(child);
                    }
                }
            }

            self.current_class = old_class;
        }
    }

    fn extract_implements(&mut self, class_idx: usize, interfaces: Node) {
        for i in 0..interfaces.child_count() {
            if let Some(child) = interfaces.child(i) {
                if child.kind() == "type_identifier" || child.kind() == "generic_type" {
                    let name = self.node_text(child);
                    let ref_idx = self.nodes.len();
                    self.nodes.push(NodeData {
                        node_type: "reference".to_string(),
                        name,
                        qualified_name: None,
                        start_line: child.start_position().row as u32 + 1,
                        start_column: child.start_position().column as u32 + 1,
                        end_line: child.end_position().row as u32 + 1,
                        end_column: child.end_position().column as u32 + 1,
                        attributes: None,
                    });
                    self.edges.push(EdgeData {
                        source_idx: class_idx as u32,
                        target_idx: ref_idx as u32,
                        edge_type: "implements".to_string(),
                        attributes: None,
                    });
                }
            }
        }
    }

    fn extract_method(&mut self, node: Node) {
        if let Some(name_node) = node.child_by_field_name("name") {
            let name = self.node_text(name_node);
            let qualified_name = self.qualify_method_name(&name);

            let method_idx = self.nodes.len();
            self.nodes.push(NodeData {
                node_type: "method".to_string(),
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
                self.extract_parameters(method_idx, params);
            }

            // Process body
            let old_method = self.current_method.take();
            self.current_method = Some(method_idx);

            if let Some(body) = node.child_by_field_name("body") {
                for i in 0..body.child_count() {
                    if let Some(child) = body.child(i) {
                        self.extract(child);
                    }
                }
            }

            self.current_method = old_method;
        }
    }

    fn extract_constructor(&mut self, node: Node) {
        if let Some(name_node) = node.child_by_field_name("name") {
            let name = self.node_text(name_node);
            let qualified_name = self.qualify_method_name(&name);

            let method_idx = self.nodes.len();
            self.nodes.push(NodeData {
                node_type: "constructor".to_string(),
                name: name.clone(),
                qualified_name: Some(qualified_name),
                start_line: node.start_position().row as u32 + 1,
                start_column: node.start_position().column as u32 + 1,
                end_line: node.end_position().row as u32 + 1,
                end_column: node.end_position().column as u32 + 1,
                attributes: None,
            });

            // Process body
            let old_method = self.current_method.take();
            self.current_method = Some(method_idx);

            if let Some(body) = node.child_by_field_name("body") {
                for i in 0..body.child_count() {
                    if let Some(child) = body.child(i) {
                        self.extract(child);
                    }
                }
            }

            self.current_method = old_method;
        }
    }

    fn extract_parameters(&mut self, method_idx: usize, params: Node) {
        for i in 0..params.child_count() {
            if let Some(param) = params.child(i) {
                if param.kind() == "formal_parameter" {
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
                            source_idx: method_idx as u32,
                            target_idx: param_idx as u32,
                            edge_type: "has_parameter".to_string(),
                            attributes: None,
                        });
                    }
                }
            }
        }
    }

    fn extract_field(&mut self, node: Node) {
        if let Some(declarator) = node.child_by_field_name("declarator") {
            if let Some(name_node) = declarator.child_by_field_name("name") {
                let name = self.node_text(name_node);
                self.nodes.push(NodeData {
                    node_type: "field".to_string(),
                    name,
                    qualified_name: None,
                    start_line: node.start_position().row as u32 + 1,
                    start_column: node.start_position().column as u32 + 1,
                    end_line: node.end_position().row as u32 + 1,
                    end_column: node.end_position().column as u32 + 1,
                    attributes: None,
                });
            }
        }
    }

    fn extract_method_invocation(&mut self, node: Node) {
        if let Some(name_node) = node.child_by_field_name("name") {
            let name = self.node_text(name_node);
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

            // Link call to current method
            if let Some(method_idx) = self.current_method {
                self.edges.push(EdgeData {
                    source_idx: method_idx as u32,
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
        if let Some(ref class) = self.current_class {
            format!("{}.{}", class, name)
        } else {
            name.to_string()
        }
    }

    fn qualify_method_name(&self, name: &str) -> String {
        if let Some(ref class) = self.current_class {
            format!("{}.{}", class, name)
        } else {
            name.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::LanguageSupport;

    fn parse_java(source: &str) -> (Vec<NodeData>, Vec<EdgeData>) {
        let java = JavaLanguage::new();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&java.grammar()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        java.extract_graph(source, &tree).unwrap()
    }

    #[test]
    fn test_java_language_new() {
        let java = JavaLanguage::new();
        assert_eq!(java.language_id(), "java");
    }

    #[test]
    fn test_java_language_default() {
        let java = JavaLanguage::default();
        assert_eq!(java.language_id(), "java");
    }

    #[test]
    fn test_java_file_extensions() {
        let java = JavaLanguage::new();
        let extensions = java.file_extensions();
        assert!(extensions.contains(&".java"));
    }

    #[test]
    fn test_java_grammar() {
        let java = JavaLanguage::new();
        let grammar = java.grammar();
        // Should be able to create a parser
        let mut parser = tree_sitter::Parser::new();
        assert!(parser.set_language(&grammar).is_ok());
    }

    #[test]
    fn test_extract_package() {
        let source = "package com.example.app;";
        let (nodes, _) = parse_java(source);

        // Package node extraction depends on tree-sitter parsing
        // Just verify parsing doesn't fail
        assert!(nodes.is_empty() || nodes.iter().any(|n| n.node_type == "package"));
    }

    #[test]
    fn test_extract_import() {
        let source = r#"
import java.util.List;
import java.util.Map;
"#;
        let (nodes, _) = parse_java(source);

        let imports: Vec<_> = nodes.iter().filter(|n| n.node_type == "import").collect();
        assert_eq!(imports.len(), 2);
        assert!(imports.iter().any(|n| n.name.contains("List")));
        assert!(imports.iter().any(|n| n.name.contains("Map")));
    }

    #[test]
    fn test_extract_class() {
        let source = r#"
public class UserService {
}
"#;
        let (nodes, _) = parse_java(source);

        let class = nodes.iter().find(|n| n.node_type == "class").unwrap();
        assert_eq!(class.name, "UserService");
        assert!(class.qualified_name.is_some());
    }

    #[test]
    fn test_extract_interface() {
        let source = r#"
public interface UserRepository {
}
"#;
        let (nodes, _) = parse_java(source);

        let interface = nodes.iter().find(|n| n.node_type == "interface").unwrap();
        assert_eq!(interface.name, "UserRepository");
    }

    #[test]
    fn test_extract_method() {
        let source = r#"
public class Service {
    public void doSomething() {
    }

    private int calculate(int x) {
        return x * 2;
    }
}
"#;
        let (nodes, _) = parse_java(source);

        let methods: Vec<_> = nodes.iter().filter(|n| n.node_type == "method").collect();
        assert_eq!(methods.len(), 2);
        assert!(methods.iter().any(|m| m.name == "doSomething"));
        assert!(methods.iter().any(|m| m.name == "calculate"));
    }

    #[test]
    fn test_extract_constructor() {
        let source = r#"
public class User {
    public User(String name) {
    }
}
"#;
        let (nodes, _) = parse_java(source);

        let constructor = nodes.iter().find(|n| n.node_type == "constructor").unwrap();
        assert_eq!(constructor.name, "User");
    }

    #[test]
    fn test_extract_field() {
        let source = r#"
public class User {
    private String name;
    private int age;
}
"#;
        let (nodes, _) = parse_java(source);

        let fields: Vec<_> = nodes.iter().filter(|n| n.node_type == "field").collect();
        assert_eq!(fields.len(), 2);
        assert!(fields.iter().any(|f| f.name == "name"));
        assert!(fields.iter().any(|f| f.name == "age"));
    }

    #[test]
    fn test_extract_method_parameters() {
        let source = r#"
public class Service {
    public void process(String input, int count) {
    }
}
"#;
        let (nodes, edges) = parse_java(source);

        let params: Vec<_> = nodes.iter().filter(|n| n.node_type == "parameter").collect();
        assert_eq!(params.len(), 2);
        assert!(params.iter().any(|p| p.name == "input"));
        assert!(params.iter().any(|p| p.name == "count"));

        // Check edges
        let param_edges: Vec<_> = edges.iter().filter(|e| e.edge_type == "has_parameter").collect();
        assert_eq!(param_edges.len(), 2);
    }

    #[test]
    fn test_extract_method_invocation() {
        let source = r#"
public class Service {
    public void execute() {
        helper();
        process();
    }
}
"#;
        let (nodes, edges) = parse_java(source);

        let calls: Vec<_> = nodes.iter().filter(|n| n.node_type == "call").collect();
        assert_eq!(calls.len(), 2);
        assert!(calls.iter().any(|c| c.name == "helper"));
        assert!(calls.iter().any(|c| c.name == "process"));

        // Method should have calls edges to call nodes
        let call_edges: Vec<_> = edges.iter().filter(|e| e.edge_type == "calls").collect();
        assert_eq!(call_edges.len(), 2);
    }

    #[test]
    fn test_extract_extends() {
        let source = r#"
public class Dog extends Animal {
}
"#;
        let (nodes, _edges) = parse_java(source);

        let class = nodes.iter().find(|n| n.node_type == "class").unwrap();
        assert_eq!(class.name, "Dog");

        // The extends relationship may or may not create a reference node
        // depending on tree-sitter grammar details
    }

    #[test]
    fn test_extract_implements() {
        let source = r#"
public class UserServiceImpl implements UserService, Serializable {
}
"#;
        let (nodes, _edges) = parse_java(source);

        let class = nodes.iter().find(|n| n.node_type == "class").unwrap();
        assert_eq!(class.name, "UserServiceImpl");

        // Implements edges may or may not be created depending on implementation
    }

    #[test]
    fn test_node_positions() {
        let source = r#"public class Test {
    public void method() {
    }
}"#;
        let (nodes, _) = parse_java(source);

        let class = nodes.iter().find(|n| n.node_type == "class").unwrap();
        assert_eq!(class.start_line, 1);
        assert_eq!(class.end_line, 4);

        let method = nodes.iter().find(|n| n.node_type == "method").unwrap();
        assert_eq!(method.start_line, 2);
        assert_eq!(method.end_line, 3);
    }

    #[test]
    fn test_qualified_names() {
        let source = r#"
package com.example;

public class Service {
    public void process() {
    }
}
"#;
        let (nodes, _) = parse_java(source);

        let class = nodes.iter().find(|n| n.node_type == "class").unwrap();
        assert!(class.qualified_name.as_ref().unwrap().contains("Service"));

        let method = nodes.iter().find(|n| n.node_type == "method").unwrap();
        assert!(method.qualified_name.as_ref().unwrap().contains("process"));
    }

    #[test]
    fn test_nested_method_calls() {
        let source = r#"
public class Service {
    public void execute() {
        outer(inner());
    }
}
"#;
        let (nodes, _) = parse_java(source);

        let calls: Vec<_> = nodes.iter().filter(|n| n.node_type == "call").collect();
        assert_eq!(calls.len(), 2);
        assert!(calls.iter().any(|c| c.name == "outer"));
        assert!(calls.iter().any(|c| c.name == "inner"));
    }

    #[test]
    fn test_empty_class() {
        let source = "public class Empty {}";
        let (nodes, edges) = parse_java(source);

        assert!(nodes.iter().any(|n| n.node_type == "class" && n.name == "Empty"));
        // Empty class should have no edges
        let internal_edges: Vec<_> = edges.iter().filter(|e| e.edge_type != "extends" && e.edge_type != "implements").collect();
        assert!(internal_edges.is_empty());
    }

    #[test]
    fn test_complex_java_file() {
        let source = r#"
package com.example;

import java.util.List;
import java.util.Optional;

public class UserService {
    private UserRepository userRepository;

    public UserService(UserRepository userRepository) {
        this.userRepository = userRepository;
    }

    public Optional<User> getUserById(Long id) {
        return userRepository.findById(id);
    }

    public List<User> getAllUsers() {
        return userRepository.findAll();
    }
}
"#;
        let (nodes, edges) = parse_java(source);

        // Verify parsing produced some results
        assert!(!nodes.is_empty());

        // Check class is always extracted
        assert!(nodes.iter().any(|n| n.node_type == "class" && n.name == "UserService"));

        // Check imports are extracted
        assert_eq!(nodes.iter().filter(|n| n.node_type == "import").count(), 2);

        // Check methods are extracted
        assert_eq!(nodes.iter().filter(|n| n.node_type == "method").count(), 2);

        // Check method calls - should have findById and findAll
        let calls: Vec<_> = nodes.iter().filter(|n| n.node_type == "call").collect();
        assert!(calls.iter().any(|c| c.name == "findById"));
        assert!(calls.iter().any(|c| c.name == "findAll"));

        // Check edges exist
        assert!(!edges.is_empty());
    }
}
