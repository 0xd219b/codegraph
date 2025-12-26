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
