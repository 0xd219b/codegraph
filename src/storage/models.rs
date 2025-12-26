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
