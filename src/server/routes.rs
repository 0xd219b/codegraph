//! API route definitions

use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};

use super::handlers;
use super::AppState;

/// Create API routes
pub fn api_routes() -> Router<Arc<AppState>> {
    Router::new()
        // Health check
        .route("/api/v1/health", get(handlers::health_check))
        // Project management
        .route("/api/v1/projects", get(handlers::list_projects))
        .route("/api/v1/projects", post(handlers::create_project))
        .route("/api/v1/projects/:id", get(handlers::get_project))
        .route("/api/v1/projects/:id/status", get(handlers::get_project_status))
        .route("/api/v1/projects/:id/parse", post(handlers::parse_project))
        // Query endpoints
        .route("/api/v1/projects/:id/definition", get(handlers::find_definition))
        .route("/api/v1/projects/:id/references", get(handlers::find_references))
        .route("/api/v1/projects/:id/callgraph", get(handlers::get_callgraph))
        .route("/api/v1/projects/:id/symbols", get(handlers::search_symbols))
        // Languages
        .route("/api/v1/languages", get(handlers::list_languages))
}
