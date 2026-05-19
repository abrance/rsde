# Design Spec: SSH Client Wrapper (v1)

- **Date**: 2026-05-17
- **Status**: Draft
- **Module**: `common/util/src/client/ssh.rs`
- **Primary Crate**: `russh` (v0.60+), `russh-sftp` (v2.1+)

## 1. Overview
A high-level Rust wrapper for SSH operations, providing a developer-friendly API for remote command execution and file transfers. It aims to abstract the complexity of `russh`'s async handlers while maintaining high performance and security.

## 2. Goals
- **Builder API**: Ergonomic configuration for host, port, user, and authentication.
- **Multiple Auth**: Support for Password and Private Key (OpenSSH/PKCS8) authentication.
- **Remote Execution**: Simple `execute(cmd)` returning stdout/stderr/exit_code.
- **Path-based File Transfer**: SFTP-backed `upload` and `download` methods.
- **No-Verify Mode**: Optional bypass for host key verification (v1 requirement).
- **Test-First**: Integration testing using `testcontainers-rs`.

## 3. Architecture

### 3.1 Components
- `SshClientBuilder`: Entry point for configuring the connection.
- `SshClient`: The active session handle.
- `SshAuth`: Enum for credential management.
- `CommandOutput`: Structured result of remote execution.

### 3.2 Key Data Structures (Simplified)
```rust
pub enum SshAuth {
    Password(String),
    PrivateKey {
        path: PathBuf,
        passphrase: Option<String>,
    },
}

pub struct SshClient {
    session: russh::client::Handle<ClientHandler>,
}

impl SshClient {
    pub async fn execute(&self, cmd: &str) -> Result<CommandOutput>;
    pub async fn upload(&self, local: &Path, remote: &Path) -> Result<()>;
    pub async fn download(&self, remote: &Path, local: &Path) -> Result<()>;
}
```

## 4. Implementation Details

### 4.1 Authentication
`russh` requires a `Handler` trait. For v1, we will implement a `NoCheckHandler` that accepts any host key to satisfy the "no host key verification" requirement.

### 4.2 File Transfers
While the requirement mentions "SCP/SFTP-like", we will use **SFTP** as the underlying protocol. SFTP is more robust, provides standard error codes, and is natively supported by `russh-sftp`. 

### 4.3 Error Handling
Errors will be categorized into:
- `ConnectionError`: DNS, Timeout, TCP issues.
- `AuthError`: Incorrect password/key, unauthorized.
- `ChannelError`: Command execution failures, SFTP subsystem issues.
- `IoError`: Local file access issues.

## 5. Testing Plan
- **Unit Tests**: Mock logic for builder configuration and parameter validation.
- **Integration Tests**: 
  - File: `common/util/src/client/ssh_test.rs`
  - Tool: `testcontainers-rs` + `linuxserver/openssh-server`.
  - Scenarios:
    1. Successful Password Auth + Command Output.
    2. Successful Private Key Auth.
    3. File Upload and subsequent verification via remote `ls`.
    4. File Download and local integrity check (SHA256).

## 6. Dependencies
```toml
[dependencies]
russh = "0.60"
russh-sftp = "2.1"
tokio = { version = "1", features = ["full"] }
anyhow = "1.0"
async-trait = "0.1"
```
