//! Data models for the code graph storage

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Project record in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRecord {
    pub id: i64,
    pub name: String,
    pub root_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// File record in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub id: i64,
    pub project_id: i64,
    pub path: String,
    pub language: String,
    pub content_hash: String,
    pub parsed_at: DateTime<Utc>,
}

/// Node record in the database (symbols: functions, classes, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeRecord {
    pub id: i64,
    pub file_id: i64,
    pub node_type: String,
    pub name: String,
    pub qualified_name: Option<String>,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub attributes: Option<String>,
}

/// Edge record in the database (relationships between nodes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeRecord {
    pub id: i64,
    pub source_id: i64,
    pub target_id: i64,
    pub edge_type: String,
    pub attributes: Option<String>,
}

/// Node data extracted from parsing (before storage)
#[derive(Debug, Clone)]
pub struct NodeData {
    pub node_type: String,
    pub name: String,
    pub qualified_name: Option<String>,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub attributes: Option<String>,
}

/// Edge data extracted from parsing (before storage)
#[derive(Debug, Clone)]
pub struct EdgeData {
    pub source_idx: u32,
    pub target_idx: u32,
    pub edge_type: String,
    pub attributes: Option<String>,
}

/// Project status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectStatus {
    pub project_id: i64,
    pub name: String,
    pub root_path: String,
    pub status: String,
    pub files_parsed: u32,
    pub nodes_count: u32,
    pub edges_count: u32,
    pub last_updated: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_record_serialize() {
        let project = ProjectRecord {
            id: 1,
            name: "test-project".to_string(),
            root_path: "/path/to/project".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let json = serde_json::to_string(&project).unwrap();
        assert!(json.contains("test-project"));
        assert!(json.contains("/path/to/project"));
    }

    #[test]
    fn test_project_record_deserialize() {
        let json = r#"{
            "id": 1,
            "name": "my-project",
            "root_path": "/home/user/project",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        }"#;

        let project: ProjectRecord = serde_json::from_str(json).unwrap();
        assert_eq!(project.id, 1);
        assert_eq!(project.name, "my-project");
        assert_eq!(project.root_path, "/home/user/project");
    }

    #[test]
    fn test_project_record_clone() {
        let project = ProjectRecord {
            id: 1,
            name: "test".to_string(),
            root_path: "/test".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let cloned = project.clone();
        assert_eq!(cloned.id, project.id);
        assert_eq!(cloned.name, project.name);
    }

    #[test]
    fn test_file_record_serialize() {
        let file = FileRecord {
            id: 1,
            project_id: 1,
            path: "/src/main.java".to_string(),
            language: "java".to_string(),
            content_hash: "abc123".to_string(),
            parsed_at: Utc::now(),
        };

        let json = serde_json::to_string(&file).unwrap();
        assert!(json.contains("/src/main.java"));
        assert!(json.contains("java"));
    }

    #[test]
    fn test_file_record_deserialize() {
        let json = r#"{
            "id": 1,
            "project_id": 1,
            "path": "/src/lib.go",
            "language": "go",
            "content_hash": "xyz789",
            "parsed_at": "2024-01-01T00:00:00Z"
        }"#;

        let file: FileRecord = serde_json::from_str(json).unwrap();
        assert_eq!(file.path, "/src/lib.go");
        assert_eq!(file.language, "go");
        assert_eq!(file.content_hash, "xyz789");
    }

    #[test]
    fn test_node_record_serialize() {
        let node = NodeRecord {
            id: 1,
            file_id: 1,
            node_type: "class".to_string(),
            name: "UserService".to_string(),
            qualified_name: Some("com.example.UserService".to_string()),
            start_line: 10,
            start_column: 1,
            end_line: 50,
            end_column: 1,
            attributes: Some(r#"{"public":true}"#.to_string()),
        };

        let json = serde_json::to_string(&node).unwrap();
        assert!(json.contains("UserService"));
        assert!(json.contains("class"));
        assert!(json.contains("com.example.UserService"));
    }

    #[test]
    fn test_node_record_deserialize() {
        let json = r#"{
            "id": 1,
            "file_id": 1,
            "node_type": "function",
            "name": "main",
            "qualified_name": "main.main",
            "start_line": 5,
            "start_column": 1,
            "end_line": 20,
            "end_column": 1,
            "attributes": null
        }"#;

        let node: NodeRecord = serde_json::from_str(json).unwrap();
        assert_eq!(node.name, "main");
        assert_eq!(node.node_type, "function");
        assert_eq!(node.start_line, 5);
        assert_eq!(node.end_line, 20);
    }

    #[test]
    fn test_node_record_optional_fields() {
        let node = NodeRecord {
            id: 1,
            file_id: 1,
            node_type: "variable".to_string(),
            name: "x".to_string(),
            qualified_name: None,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 5,
            attributes: None,
        };

        let json = serde_json::to_string(&node).unwrap();
        let parsed: NodeRecord = serde_json::from_str(&json).unwrap();

        assert!(parsed.qualified_name.is_none());
        assert!(parsed.attributes.is_none());
    }

    #[test]
    fn test_edge_record_serialize() {
        let edge = EdgeRecord {
            id: 1,
            source_id: 1,
            target_id: 2,
            edge_type: "calls".to_string(),
            attributes: Some(r#"{"async":true}"#.to_string()),
        };

        let json = serde_json::to_string(&edge).unwrap();
        assert!(json.contains("calls"));
        assert!(json.contains("async"));
    }

    #[test]
    fn test_edge_record_deserialize() {
        let json = r#"{
            "id": 1,
            "source_id": 10,
            "target_id": 20,
            "edge_type": "extends",
            "attributes": null
        }"#;

        let edge: EdgeRecord = serde_json::from_str(json).unwrap();
        assert_eq!(edge.source_id, 10);
        assert_eq!(edge.target_id, 20);
        assert_eq!(edge.edge_type, "extends");
    }

    #[test]
    fn test_node_data_clone() {
        let node = NodeData {
            node_type: "method".to_string(),
            name: "process".to_string(),
            qualified_name: Some("Service.process".to_string()),
            start_line: 10,
            start_column: 5,
            end_line: 25,
            end_column: 5,
            attributes: None,
        };

        let cloned = node.clone();
        assert_eq!(cloned.name, node.name);
        assert_eq!(cloned.node_type, node.node_type);
    }

    #[test]
    fn test_edge_data_clone() {
        let edge = EdgeData {
            source_idx: 0,
            target_idx: 1,
            edge_type: "contains".to_string(),
            attributes: Some(r#"{"count":1}"#.to_string()),
        };

        let cloned = edge.clone();
        assert_eq!(cloned.source_idx, edge.source_idx);
        assert_eq!(cloned.target_idx, edge.target_idx);
        assert_eq!(cloned.edge_type, edge.edge_type);
    }

    #[test]
    fn test_project_status_serialize() {
        let status = ProjectStatus {
            project_id: 1,
            name: "test-project".to_string(),
            root_path: "/test/path".to_string(),
            status: "ready".to_string(),
            files_parsed: 10,
            nodes_count: 100,
            edges_count: 50,
            last_updated: Utc::now(),
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("test-project"));
        assert!(json.contains("ready"));
        assert!(json.contains("100"));
    }

    #[test]
    fn test_project_status_deserialize() {
        let json = r#"{
            "project_id": 1,
            "name": "my-project",
            "root_path": "/home/project",
            "status": "parsing",
            "files_parsed": 5,
            "nodes_count": 50,
            "edges_count": 25,
            "last_updated": "2024-01-01T00:00:00Z"
        }"#;

        let status: ProjectStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status.name, "my-project");
        assert_eq!(status.status, "parsing");
        assert_eq!(status.files_parsed, 5);
        assert_eq!(status.nodes_count, 50);
    }

    #[test]
    fn test_node_data_debug() {
        let node = NodeData {
            node_type: "class".to_string(),
            name: "Test".to_string(),
            qualified_name: None,
            start_line: 1,
            start_column: 1,
            end_line: 10,
            end_column: 1,
            attributes: None,
        };

        let debug_str = format!("{:?}", node);
        assert!(debug_str.contains("Test"));
        assert!(debug_str.contains("class"));
    }

    #[test]
    fn test_edge_data_debug() {
        let edge = EdgeData {
            source_idx: 0,
            target_idx: 1,
            edge_type: "calls".to_string(),
            attributes: None,
        };

        let debug_str = format!("{:?}", edge);
        assert!(debug_str.contains("calls"));
        assert!(debug_str.contains("source_idx: 0"));
    }
}
