//! HTTP request handlers

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use super::AppState;
use crate::core::query::QueryExecutor;
use crate::languages::LanguageRegistry;
use crate::storage::models::ProjectRecord;
use crate::storage::Database;

// ==================== Response Types ====================

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}

#[derive(Serialize)]
pub struct ProjectResponse {
    pub project_id: i64,
    pub name: String,
    pub root_path: String,
    pub status: String,
}

#[derive(Serialize)]
pub struct LanguageInfo {
    pub id: String,
    pub extensions: Vec<String>,
}

// ==================== Request Types ====================

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub root_path: String,
    #[serde(default)]
    pub languages: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct ParseProjectRequest {
    #[serde(default)]
    pub incremental: bool,
    #[serde(default)]
    pub paths: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct DefinitionQuery {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Deserialize)]
pub struct ReferencesQuery {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Deserialize)]
pub struct CallgraphQuery {
    pub symbol: String,
    #[serde(default = "default_depth")]
    pub depth: u32,
    #[serde(default = "default_direction")]
    pub direction: String,
}

fn default_depth() -> u32 {
    1
}

fn default_direction() -> String {
    "both".to_string()
}

#[derive(Deserialize)]
pub struct SymbolsQuery {
    pub query: String,
    #[serde(rename = "type")]
    pub symbol_type: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    50
}

// ==================== Handlers ====================

/// Health check endpoint
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// List all projects
pub async fn list_projects(
    State(_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    // For now, return empty list - would need to add a list_projects method
    let projects: Vec<ProjectResponse> = vec![];
    Ok(Json(projects))
}

/// Create a new project
pub async fn create_project(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateProjectRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let db = state.db.lock().await;

    let project = ProjectRecord {
        id: 0,
        name: req.name.clone(),
        root_path: req.root_path.clone(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    match db.insert_project(&project) {
        Ok(id) => Ok((
            StatusCode::CREATED,
            Json(ProjectResponse {
                project_id: id,
                name: req.name,
                root_path: req.root_path,
                status: "created".to_string(),
            }),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "database_error".to_string(),
                message: e.to_string(),
            }),
        )),
    }
}

/// Get project details
pub async fn get_project(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let db = state.db.lock().await;

    match db.get_project_status(id) {
        Ok(Some(status)) => Ok(Json(status)),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "not_found".to_string(),
                message: format!("Project {} not found", id),
            }),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "database_error".to_string(),
                message: e.to_string(),
            }),
        )),
    }
}

/// Get project status
pub async fn get_project_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let db = state.db.lock().await;

    match db.get_project_status(id) {
        Ok(Some(status)) => Ok(Json(status)),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "not_found".to_string(),
                message: format!("Project {} not found", id),
            }),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "database_error".to_string(),
                message: e.to_string(),
            }),
        )),
    }
}

/// Parse a project
pub async fn parse_project(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(req): Json<ParseProjectRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    // This would need to spawn a background task for parsing
    // For now, return a placeholder response
    Ok(Json(serde_json::json!({
        "status": "parsing",
        "project_id": id,
        "incremental": req.incremental,
        "message": "Parsing started (not yet implemented as background task)"
    })))
}

/// Find symbol definition
pub async fn find_definition(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(query): Query<DefinitionQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let query_db = match Database::open(&state.db_path) {
        Ok(db) => db,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "database_error".to_string(),
                    message: e.to_string(),
                }),
            ));
        }
    };

    let executor = QueryExecutor::new(query_db);

    match executor.find_definition(id, &query.file, query.line, query.column) {
        Ok(result) => Ok(Json(result)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "query_error".to_string(),
                message: e.to_string(),
            }),
        )),
    }
}

/// Find all references
pub async fn find_references(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(query): Query<ReferencesQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let query_db = match Database::open(&state.db_path) {
        Ok(db) => db,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "database_error".to_string(),
                    message: e.to_string(),
                }),
            ));
        }
    };

    let executor = QueryExecutor::new(query_db);

    match executor.find_references(id, &query.file, query.line, query.column) {
        Ok(result) => Ok(Json(result)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "query_error".to_string(),
                message: e.to_string(),
            }),
        )),
    }
}

/// Get call graph
pub async fn get_callgraph(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(query): Query<CallgraphQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let query_db = match Database::open(&state.db_path) {
        Ok(db) => db,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "database_error".to_string(),
                    message: e.to_string(),
                }),
            ));
        }
    };

    let executor = QueryExecutor::new(query_db);

    match executor.get_callgraph(id, &query.symbol, query.depth, &query.direction) {
        Ok(result) => Ok(Json(result)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "query_error".to_string(),
                message: e.to_string(),
            }),
        )),
    }
}

/// Search symbols
pub async fn search_symbols(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(query): Query<SymbolsQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let query_db = match Database::open(&state.db_path) {
        Ok(db) => db,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "database_error".to_string(),
                    message: e.to_string(),
                }),
            ));
        }
    };

    let executor = QueryExecutor::new(query_db);

    match executor.search_symbols(id, &query.query, query.symbol_type.as_deref(), query.limit) {
        Ok(result) => Ok(Json(result)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "query_error".to_string(),
                message: e.to_string(),
            }),
        )),
    }
}

/// List supported languages
pub async fn list_languages() -> Json<Vec<LanguageInfo>> {
    let registry = LanguageRegistry::new();
    let languages: Vec<LanguageInfo> = registry
        .list_languages()
        .iter()
        .map(|l| LanguageInfo {
            id: l.language_id().to_string(),
            extensions: l.file_extensions().iter().map(|s| s.to_string()).collect(),
        })
        .collect();

    Json(languages)
}
