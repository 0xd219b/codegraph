//! CodeGraph - Multi-language code graph parsing library
//!
//! This library provides the core functionality for parsing source code
//! and building searchable code graphs.

pub mod core;
pub mod languages;
pub mod server;
pub mod storage;

pub use crate::core::config::Config;
pub use crate::core::graph::GraphBuilder;
pub use crate::core::parser::CodeParser;
pub use crate::core::query::QueryExecutor;
pub use crate::languages::LanguageRegistry;
pub use crate::storage::Database;
