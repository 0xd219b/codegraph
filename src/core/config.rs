//! Configuration management for CodeGraph

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main configuration for the CodeGraph service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Server configuration
    pub server: ServerConfig,

    /// Database configuration
    pub database: DatabaseConfig,

    /// Logging configuration
    pub logging: LoggingConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            database: DatabaseConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

/// HTTP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Host to bind to
    pub host: String,

    /// Port to listen on
    pub port: u16,

    /// Enable CORS
    pub cors_enabled: bool,

    /// Allowed origins for CORS
    pub cors_origins: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            cors_enabled: true,
            cors_origins: vec!["*".to_string()],
        }
    }
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Path to SQLite database file
    pub path: PathBuf,

    /// Connection pool size
    pub pool_size: u32,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("codegraph.db"),
            pool_size: 4,
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,

    /// Log format (pretty, json, compact)
    pub format: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "pretty".to_string(),
        }
    }
}

impl Config {
    /// Load configuration from a TOML file
    pub fn from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to a TOML file
    pub fn to_file(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 8080);
        assert!(config.server.cors_enabled);
        assert_eq!(config.database.path, PathBuf::from("codegraph.db"));
        assert_eq!(config.database.pool_size, 4);
        assert_eq!(config.logging.level, "info");
        assert_eq!(config.logging.format, "pretty");
    }

    #[test]
    fn test_default_server_config() {
        let config = ServerConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 8080);
        assert!(config.cors_enabled);
        assert_eq!(config.cors_origins, vec!["*".to_string()]);
    }

    #[test]
    fn test_default_database_config() {
        let config = DatabaseConfig::default();
        assert_eq!(config.path, PathBuf::from("codegraph.db"));
        assert_eq!(config.pool_size, 4);
    }

    #[test]
    fn test_default_logging_config() {
        let config = LoggingConfig::default();
        assert_eq!(config.level, "info");
        assert_eq!(config.format, "pretty");
    }

    #[test]
    fn test_config_serialize_deserialize() {
        let config = Config::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();

        assert_eq!(parsed.server.host, config.server.host);
        assert_eq!(parsed.server.port, config.server.port);
        assert_eq!(parsed.database.path, config.database.path);
        assert_eq!(parsed.logging.level, config.logging.level);
    }

    #[test]
    fn test_config_from_file_and_to_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let config = Config {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 9090,
                cors_enabled: false,
                cors_origins: vec!["https://example.com".to_string()],
            },
            database: DatabaseConfig {
                path: PathBuf::from("/tmp/test.db"),
                pool_size: 8,
            },
            logging: LoggingConfig {
                level: "debug".to_string(),
                format: "json".to_string(),
            },
        };

        config.to_file(&config_path).unwrap();
        let loaded = Config::from_file(&config_path).unwrap();

        assert_eq!(loaded.server.host, "0.0.0.0");
        assert_eq!(loaded.server.port, 9090);
        assert!(!loaded.server.cors_enabled);
        assert_eq!(loaded.database.pool_size, 8);
        assert_eq!(loaded.logging.level, "debug");
        assert_eq!(loaded.logging.format, "json");
    }

    #[test]
    fn test_config_from_file_not_found() {
        let result = Config::from_file(std::path::Path::new("/nonexistent/path/config.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_custom_cors_origins() {
        let config = ServerConfig {
            host: "localhost".to_string(),
            port: 3000,
            cors_enabled: true,
            cors_origins: vec![
                "http://localhost:3000".to_string(),
                "https://app.example.com".to_string(),
            ],
        };

        assert_eq!(config.cors_origins.len(), 2);
        assert!(config.cors_origins.contains(&"http://localhost:3000".to_string()));
    }
}
