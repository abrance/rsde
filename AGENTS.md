# AGENTS.md - AI Coding Agent Guidelines for rsde

> rsde(xy) is a Rust-based Kubernetes toolset providing data sync (rsync), remote config (rc), OCR (pic_recog), and unified API gateway (apiserver).

## Build, Lint, and Test Commands

### Rust (Primary)

```bash
# Build all workspace packages
cargo build --release

# Build specific package
cargo build -p rsync
cargo build -p apiserver
cargo build -p rc
cargo build -p pic_recog

# Run all tests
cargo test --workspace

# Run tests for specific package
cargo test -p rule --all-features

# Run single test
cargo test test_name                    # by name
cargo test module::tests::test_name     # by path
cargo test --package rule test_name     # in specific package

# Run tests with output
cargo test -- --nocapture --test-threads=1

# Format check and apply
cargo fmt -- --check
cargo fmt

# Lint
cargo clippy --workspace --all-features
```

### Frontend (webserver/frontend)

```bash
cd webserver/frontend
npm install
npm run build      # Production build (outputs to dist/)
npm run dev        # Development server (port 5173)
npm run lint       # ESLint
```

### Makefile Shortcuts (Root)

```bash
make build         # Build all binaries (rsync, rc, apiserver)
make test          # Run all tests
make fmt           # Format code
make fmt-check     # Check formatting
make clippy        # Run clippy
make run-apiserver # Build frontend + run apiserver
```

### Makefile Shortcuts (rsync/)

```bash
make test          # cargo test --workspace --all-features
make test-verbose  # With --nocapture
make check         # fmt-check + clippy + test
make dev           # fmt + test
```

### Development Environment

**Starting apiserver locally** requires the dev config file:

```bash
# Use the development config (REQUIRED for local dev)
API_CONFIG="manifest/dev/remote_ocr.toml" cargo run -p apiserver

# Or use make target
make run-apiserver
```

Config file location: `manifest/dev/remote_ocr.toml`

### Local Kubernetes (xy namespace)

The service is deployed to the `xy` namespace. Use the following commands:

```bash
# Check current deployment status
kubectl get all -n xy

# View logs
kubectl logs -f -n xy -l app.kubernetes.io/name=rsde-apiserver

# Update/upgrade the helm deployment
helm upgrade rsde-apiserver ./helm/rsde -n xy \
  -f k8s/file-transfer-go/helm/file-transfer-go/values.yaml

# Rollback if needed
helm rollback rsde-apiserver -n xy
```

**Important**: When updating the local K8s deployment, always specify the values file:
`-f k8s/file-transfer-go/helm/file-transfer-go/values.yaml`

## Code Style Guidelines

### Rust Edition and Dependencies

- **Edition**: 2024 (workspace-level)
- **Async Runtime**: tokio (full features)
- **Serialization**: serde + serde_json
- **Error Handling**: anyhow (applications), custom enum errors (libraries)
- **HTTP Framework**: axum 0.7
- **Config Format**: TOML (via `toml` crate)
- **Logging**: tracing + tracing-subscriber

### Import Organization

```rust
// 1. Standard library
use std::{
    fmt,
    path::Path,
    sync::Arc,
};

// 2. External crates
use async_trait::async_trait;
use axum::{Router, routing::get};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

// 3. Internal crates (workspace members)
use config::{ConfigLoader, GlobalConfig};
use rule::{Source, Sink, Transform};

// 4. Local modules
use crate::ocr;
mod anybox;
```

### Naming Conventions

| Item | Convention | Example |
|------|------------|---------|
| Types/Traits | PascalCase | `DataTransferConfig`, `SourceRuntime` |
| Functions/Methods | snake_case | `create_routes`, `next_event` |
| Constants | SCREAMING_SNAKE | `READ_SIZE` |
| Modules | snake_case | `rule_file_watch` |
| Test files | `*_test.rs` suffix | `dial_test.rs` |

### Error Handling Patterns

**For libraries (like rule)**, use custom error enums:

```rust
#[derive(Debug, Clone)]
pub enum RsyncError {
    BuildError(String),
    ReadError(String),
    WriteError(String),
    ConfigError(String),
}

impl std::error::Error for RsyncError {}

impl From<std::io::Error> for RsyncError {
    fn from(err: std::io::Error) -> Self {
        RsyncError::WriteError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, RsyncError>;
```

**For applications (like apiserver)**, use `anyhow::Result`.

### Async Trait Pattern

Use `#[async_trait]` for async trait methods:

```rust
#[async_trait]
#[typetag::serde(tag = "source_type")]
pub trait Source: Send + Sync + fmt::Debug {
    fn clone_box(&self) -> Box<dyn Source>;
    async fn build(&self, cx: SourceContext) -> Result<Box<dyn SourceRuntime>>;
    fn source_type(&self) -> &str;
}
```

### Configuration Pattern

Use the `config` crate (`common/config`) for unified configuration:

```rust
use config::{ConfigLoader, GlobalConfig};

let config = GlobalConfig::from_file("config.toml")?;
if let Some(ocr_config) = config.remote_ocr {
    // use config
}
```

### Serialization with typetag

For polymorphic serialization of trait objects:

```rust
#[typetag::serde(name = "file")]
#[async_trait]
impl Source for FileSourceConfig {
    // implementation
}
```

### Test Organization

- Unit tests: Same file, inside `#[cfg(test)] mod tests { ... }`
- Integration tests: Separate `*_test.rs` files
- Use `#[ignore]` for tests requiring external services

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_functionality() {
        // ...
    }

    #[tokio::test]
    async fn test_async_operation() {
        // ...
    }

    #[test]
    #[ignore] // Requires running server
    fn test_integration() {
        // ...
    }
}
```

## Project Structure

```
rsde/
├── apiserver/         # API gateway (axum-based web server)
├── rsync/             # Data sync tool
│   └── lib/rule/      # Core Source→Transform→Sink abstractions
├── rc/                # Remote config management
├── pic_recog/         # OCR recognition module
├── anybox/            # Text sharing service (Pastebin-like)
├── common/
│   ├── config/        # Unified configuration definitions
│   ├── core/          # Core functionality
│   └── util/          # Utilities (logging, metrics, HTTP client)
├── webserver/
│   └── frontend/      # React + TypeScript + Vite frontend
├── helm/rsde/         # Kubernetes Helm chart
└── manifest/          # Configuration files
```

## Key Architectural Patterns

### Source → Transform → Sink Pipeline (rsync)

```rust
// Configuration layer (serializable)
trait Source: Send + Sync + Debug {
    async fn build(&self, cx: SourceContext) -> Result<Box<dyn SourceRuntime>>;
}

// Runtime layer (stateful execution)
trait SourceRuntime: Send + Sync {
    async fn next_event(&mut self) -> Result<Option<Box<dyn Event>>>;
}
```

### Axum Route Creation

```rust
pub fn create_routes(config: MyConfig) -> Router {
    let state = AppState { config: Arc::new(config) };
    
    Router::new()
        .route("/health", get(health_check))
        .route("/action", post(handle_action))
        .with_state(state)
}
```

## CI/CD Pipeline

The GitHub Actions workflow (`.github/workflows/rsync-ci.yml`) runs:

1. **test**: Format check → Clippy → Tests → Build
2. **helm-lint**: Helm chart validation
3. **docker-build**: Build and push to ghcr.io (on push to main)

## Common Pitfalls

1. **Missing frontend build**: Run `cd webserver && make build` before `apiserver`
2. **Blocking in async**: Use `tokio::task::spawn_blocking` for sync operations
3. **Config loading**: Check `API_CONFIG` env var, defaults to `apiserver/config.toml`
4. **Test file placement**: Use `*_test.rs` suffix, not `test_*.rs`

## Environment Variables

| Variable | Purpose | Default |
|----------|---------|---------|
| `API_CONFIG` | Config file path | `apiserver/config.toml` |
| `RSYNC_CONFIG_DIR` | Rsync config directory | `.` |
| `RUST_LOG` | Log level filter | `apiserver=debug,tower_http=debug` |
