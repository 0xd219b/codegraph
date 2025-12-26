# CodeGraph

A multi-language code graph parsing service built with Tree-sitter. It constructs searchable code graphs for symbol definition lookup, reference tracking, and call graph analysis.

## Features

- **Multi-language Support**: Currently supports Java and Go, extensible to more languages
- **Symbol Definition Lookup**: Quickly locate where functions, classes, and methods are defined
- **Reference Tracking**: Find all usages of a symbol across the codebase
- **Call Graph Analysis**: Analyze function call relationships (callers/callees)
- **Project-level Parsing**: Parse entire projects and build cross-file code graphs
- **Incremental Updates**: Support for incremental parsing on code changes (WIP)

## Installation

### Build from Source

```bash
# Clone the repository
git clone https://github.com/yourname/codegraph.git
cd codegraph

# Build
cargo build --release

# Install to system path
cp target/release/codegraph /usr/local/bin/
```

## Quick Start

### 1. Parse a Project

```bash
# Parse current directory
codegraph parse --path . --name myproject

# Parse a specific directory
codegraph parse --path /path/to/project --name myproject

# Parse only specific languages
codegraph parse --path . --name myproject --languages java,go
```

### 2. Query Symbols

```bash
# Find symbol definition
codegraph query definition --symbol "UserService"

# Find all references to a symbol
codegraph query references --symbol "getUserById" --limit 50

# Search symbols by name pattern
codegraph query symbols --query "User" --symbol-type class --limit 20

# Get call graph
codegraph query callgraph --symbol "handleRequest" --depth 2 --direction both
```

### 3. Project Management

```bash
# List all parsed projects
codegraph projects

# Query a specific project
codegraph query --project myproject symbols --query "Service"
```

## Command Reference

### parse

Parse a project and build the code graph.

```bash
codegraph parse [OPTIONS]

Options:
  -p, --path <PATH>           Project root path
  -n, --name <NAME>           Project name (defaults to directory name)
  -l, --languages <LANGS>     Languages to parse (comma-separated, e.g., java,go)
  -d, --database <FILE>       Database file path [default: codegraph.db]
```

### query

Query the code graph.

#### definition

Find where a symbol is defined.

```bash
codegraph query definition --symbol <NAME>
```

#### references

Find all references to a symbol.

```bash
codegraph query references --symbol <NAME> [--limit <N>]
```

#### symbols

Search for symbols by name pattern.

```bash
codegraph query symbols --query <PATTERN> [--symbol-type <TYPE>] [--limit <N>]

Symbol types: class, interface, struct, method, function, field, variable
```

#### callgraph

Get the call graph for a symbol.

```bash
codegraph query callgraph --symbol <NAME> [--depth <N>] [--direction <DIR>]

Directions: callers, callees, both
```

### projects

List all parsed projects.

```bash
codegraph projects [--database <FILE>]
```

### languages

List supported languages.

```bash
codegraph languages
```

## Configuration

Create a `config.toml` file (optional):

```toml
[server]
host = "127.0.0.1"
port = 8080
cors_enabled = true
cors_origins = ["*"]

[database]
path = "codegraph.db"
pool_size = 4

[logging]
level = "info"      # trace, debug, info, warn, error
format = "pretty"   # pretty, json, compact
```

## Output Format

All query results are returned in JSON format:

```json
{
  "found": true,
  "definition": {
    "file": "/path/to/UserService.java",
    "line": 15,
    "column": 1,
    "node_type": "class",
    "name": "UserService",
    "qualified_name": "com.example.UserService"
  }
}
```

## Tech Stack

- **Language**: Rust
- **Syntax Parsing**: tree-sitter
- **Storage**: SQLite (rusqlite)
- **CLI**: clap
- **Serialization**: serde, serde_json, toml

## Project Structure

```
codegraph/
├── src/
│   ├── main.rs           # CLI entry point
│   ├── core/             # Core engine
│   │   ├── config.rs     # Configuration
│   │   ├── parser.rs     # Code parser
│   │   ├── query.rs      # Query executor
│   │   └── ...
│   ├── storage/          # Storage layer
│   │   ├── sqlite.rs     # SQLite implementation
│   │   └── models.rs     # Data models
│   ├── languages/        # Language support
│   │   ├── java/         # Java support
│   │   └── go/           # Go support
│   └── server/           # HTTP server (optional)
├── config.example.toml   # Example configuration
└── Cargo.toml
```

## Development

```bash
# Run tests
cargo test

# Development build
cargo build

# Verbose logging
codegraph --verbose parse --path .
```

## License

MIT
