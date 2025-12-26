//! HTTP server for the CodeGraph service

mod handlers;
mod routes;

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::storage::Database;

/// Shared application state
pub struct AppState {
    pub db_path: PathBuf,
    pub db: Mutex<Database>,
}

/// Run the HTTP server
pub async fn run_server(host: &str, port: u16, db_path: &Path) -> Result<()> {
    // Initialize database
    let db = Database::open(db_path)?;
    db.init_schema()?;

    let state = Arc::new(AppState {
        db_path: db_path.to_path_buf(),
        db: Mutex::new(db),
    });

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build router
    let app = Router::new()
        .merge(routes::api_routes())
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    // Start server
    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
    info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
