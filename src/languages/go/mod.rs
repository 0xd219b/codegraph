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
