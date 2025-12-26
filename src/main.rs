//! CodeGraph - Multi-language code graph parsing service
//!
//! A command-line tool for parsing code repositories and building
//! searchable code graphs with support for multiple programming languages.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::info;

mod core;
mod languages;
mod server;
mod storage;

pub use crate::core::config::Config;

/// CodeGraph - Multi-language code graph parsing service
#[derive(Parser)]
#[command(name = "codegraph")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the HTTP server
    Start {
        /// Host to bind to
        #[arg(short = 'H', long, default_value = "127.0.0.1")]
        host: String,

        /// Port to listen on
        #[arg(short, long, default_value_t = 8080)]
        port: u16,

        /// Path to SQLite database file
        #[arg(short, long, default_value = "codegraph.db")]
        database: PathBuf,
    },

    /// Parse a project and build the code graph
    Parse {
        /// Path to the project root
        #[arg(short, long)]
        path: PathBuf,

        /// Project name (defaults to directory name)
        #[arg(short, long)]
        name: Option<String>,

        /// Languages to parse (auto-detect if not specified)
        #[arg(short, long)]
        languages: Option<Vec<String>>,

        /// Path to SQLite database file
        #[arg(short, long, default_value = "codegraph.db")]
        database: PathBuf,
    },

    /// Query the code graph
    Query {
        /// Path to SQLite database file
        #[arg(short, long, default_value = "codegraph.db")]
        database: PathBuf,

        /// Project name or ID to query
        #[arg(short, long)]
        project: Option<String>,

        #[command(subcommand)]
        query_type: QueryCommands,
    },

    /// List all projects
    Projects {
        /// Path to SQLite database file
        #[arg(short, long, default_value = "codegraph.db")]
        database: PathBuf,
    },

    /// List supported languages
    Languages,

    /// Show server status
    Status {
        /// Host to connect to
        #[arg(short = 'H', long, default_value = "127.0.0.1")]
        host: String,

        /// Port to connect to
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
    },
}

#[derive(Subcommand)]
enum QueryCommands {
    /// Find symbol definition by name
    Definition {
        /// Symbol name or qualified name to find definition for
        #[arg(short, long)]
        symbol: String,
    },

    /// Find all references to a symbol (where a symbol is used/called)
    References {
        /// Symbol name or qualified name to find references for
        #[arg(short, long)]
        symbol: String,

        /// Maximum number of results
        #[arg(short, long, default_value_t = 100)]
        limit: u32,
    },

    /// Get call graph for a symbol
    Callgraph {
        /// Symbol name or qualified name
        #[arg(short, long)]
        symbol: String,

        /// Depth of traversal
        #[arg(short, long, default_value_t = 1)]
        depth: u32,

        /// Direction: callers, callees, or both
        #[arg(long, default_value = "both")]
        direction: String,
    },

    /// Search for symbols
    Symbols {
        /// Search query
        #[arg(short, long)]
        query: String,

        /// Symbol type filter
        #[arg(short = 't', long)]
        symbol_type: Option<String>,

        /// Maximum number of results
        #[arg(short, long, default_value_t = 50)]
        limit: u32,
    },
}

fn init_logging(verbose: bool) {
    let filter = if verbose {
        "codegraph=debug,tower_http=debug"
    } else {
        "codegraph=info,tower_http=info"
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .init();
}

/// Resolve project name/id to project_id
fn resolve_project(db: &storage::Database, project: Option<&str>) -> anyhow::Result<i64> {
    match project {
        Some(p) => {
            // Try to parse as ID first
            if let Ok(id) = p.parse::<i64>() {
                return Ok(id);
            }
            // Otherwise, look up by name
            db.get_project_by_name(p)?
                .map(|proj| proj.id)
                .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", p))
        }
        None => {
            // Get the first/default project
            let projects = db.list_projects()?;
            if projects.is_empty() {
                anyhow::bail!("No projects found. Use 'codegraph parse' to create one.");
            }
            if projects.len() > 1 {
                eprintln!("Multiple projects found. Use --project to specify one:");
                for p in &projects {
                    eprintln!("  - {} (id={})", p.name, p.id);
                }
                anyhow::bail!("Please specify a project with --project <name|id>");
            }
            Ok(projects[0].id)
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    init_logging(cli.verbose);

    match cli.command {
        Commands::Start {
            host,
            port,
            database,
        } => {
            info!("Starting CodeGraph server on {}:{}", host, port);
            server::run_server(&host, port, &database).await?;
        }

        Commands::Parse {
            path,
            name,
            languages,
            database,
        } => {
            let project_name = name.unwrap_or_else(|| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unnamed")
                    .to_string()
            });

            info!("Parsing project '{}' at {:?}", project_name, path);
            core::parse_project(&database, &project_name, &path, languages.as_deref()).await?;
        }

        Commands::Query {
            database,
            project,
            query_type,
        } => {
            let db = storage::Database::open(&database)?;
            let project_id = resolve_project(&db, project.as_deref())?;

            match query_type {
                QueryCommands::Definition { symbol } => {
                    let result = core::query::find_definition_by_symbol(&database, project_id, &symbol)?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                QueryCommands::References { symbol, limit } => {
                    let result = core::query::find_references_by_symbol(&database, project_id, &symbol, limit)?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                QueryCommands::Callgraph {
                    symbol,
                    depth,
                    direction,
                } => {
                    let result = core::query::get_callgraph_with_project(&database, project_id, &symbol, depth, &direction)?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                QueryCommands::Symbols {
                    query,
                    symbol_type,
                    limit,
                } => {
                    let result =
                        core::query::search_symbols_with_project(&database, project_id, &query, symbol_type.as_deref(), limit)?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
            }
        }

        Commands::Projects { database } => {
            let db = storage::Database::open(&database)?;
            let projects = db.list_projects()?;

            if projects.is_empty() {
                println!("No projects found.");
            } else {
                println!("Projects:");
                for p in projects {
                    println!("  - {} (id={}, path={})", p.name, p.id, p.root_path);
                }
            }
        }

        Commands::Languages => {
            let registry = languages::LanguageRegistry::new();
            println!("Supported languages:");
            for lang in registry.list_languages() {
                println!(
                    "  - {} (extensions: {})",
                    lang.language_id(),
                    lang.file_extensions().join(", ")
                );
            }
        }

        Commands::Status { host, port } => {
            let url = format!("http://{}:{}/api/v1/health", host, port);
            match reqwest::get(&url).await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        println!("Server is running at {}:{}", host, port);
                    } else {
                        println!("Server returned status: {}", resp.status());
                    }
                }
                Err(e) => {
                    println!("Failed to connect to server: {}", e);
                }
            }
        }
    }

    Ok(())
}
