//! Storage layer for persisting code graph data

pub mod models;
pub mod sqlite;

pub use sqlite::Database;
