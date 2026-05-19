# SSH Client Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `util::client::ssh` module that matches the existing client style and supports password auth, private-key auth, remote exec, and path-based upload/download.

**Architecture:** Add a single new leaf module at `common/util/src/client/ssh.rs`, then wire it into `common/util/src/client/mod.rs` and crate dependencies. Keep the public API aligned with `mysql` and `redis`: a serializable config builder plus a client wrapper returning `Result<_, String>`.

**Tech Stack:** Rust 2024, Tokio, serde, SSH client crate (to be finalized from library research), existing util crate patterns.

---

### Task 1: Dependency wiring and module export

**Files:**
- Modify: `Cargo.toml`
- Modify: `common/util/Cargo.toml`
- Modify: `common/util/src/client/mod.rs`

- [ ] **Step 1: Write the failing test import**

Create a unit test in `common/util/src/client/ssh.rs` that expects `SshClientConfig` to exist and be importable.

- [ ] **Step 2: Run test to verify it fails**

Run: `~/.cargo/bin/cargo test -p util ssh::tests::test_config_creation_defaults -- --nocapture`

Expected: compile failure because `ssh` module does not exist yet.

- [ ] **Step 3: Add SSH dependencies and export**

Add the chosen SSH crates to workspace dependencies and util crate dependencies, then export `pub mod ssh;` from `common/util/src/client/mod.rs`.

- [ ] **Step 4: Re-run the same test**

Run: `~/.cargo/bin/cargo test -p util ssh::tests::test_config_creation_defaults -- --nocapture`

Expected: failure moves from missing module to missing types/behavior.

### Task 2: Config builder and validation

**Files:**
- Create: `common/util/src/client/ssh.rs`

- [ ] **Step 1: Write failing config tests**

Add tests for:
- default host/port parsing
- password auth builder
- private key builder
- timeout builder
- validation for missing auth
- validation for conflicting auth methods

- [ ] **Step 2: Run the targeted test to confirm red**

Run: `~/.cargo/bin/cargo test -p util ssh::tests -- --nocapture`

Expected: failing tests for missing config implementation.

- [ ] **Step 3: Implement minimal config builder**

Add `SshClientConfig` with:
- `host`, `port`, `username`
- `password`
- `private_key_path`
- `private_key_passphrase`
- `timeout`
- `disable_host_key_check`

Builder methods:
- `new(...)`
- `with_port(...)`
- `with_password(...)`
- `with_private_key_path(...)`
- `with_private_key_passphrase(...)`
- `with_timeout(...)`
- `with_disable_host_key_check(...)`

- [ ] **Step 4: Re-run config tests**

Run: `~/.cargo/bin/cargo test -p util ssh::tests -- --nocapture`

Expected: config tests pass.

### Task 3: Client skeleton and result types

**Files:**
- Modify: `common/util/src/client/ssh.rs`

- [ ] **Step 1: Write failing client tests**

Add tests for:
- `SshClient::new` rejecting missing auth
- `SshClient::new` rejecting conflicting auth
- `SshExecResult` serde round-trip

- [ ] **Step 2: Run the targeted test to confirm red**

Run: `~/.cargo/bin/cargo test -p util ssh::tests -- --nocapture`

Expected: failing tests for missing client/result implementation.

- [ ] **Step 3: Implement minimal client skeleton**

Add:
- `SshExecResult`
- `SshClient`
- `SshClient::new`
- `SshClient::get_config`

Validation rule for v1: reject ambiguous auth configuration instead of silently choosing one.

- [ ] **Step 4: Re-run unit tests**

Run: `~/.cargo/bin/cargo test -p util ssh::tests -- --nocapture`

Expected: unit tests pass.

### Task 4: Remote exec and ping

**Files:**
- Modify: `common/util/src/client/ssh.rs`
- Create: `test/plan/tp_ssh_client.rs`

- [ ] **Step 1: Write ignored integration tests first**

Add env-driven ignored tests for:
- password auth exec
- ping

- [ ] **Step 2: Run one ignored test to confirm red**

Run: `~/.cargo/bin/cargo test -p util --test tp_ssh_client test_ssh_password_exec -- --ignored --nocapture`

Expected: failure because connection/exec is not implemented yet.

- [ ] **Step 3: Implement password-auth connection, ping, and exec**

Add enough implementation to connect, authenticate, open a session/channel, execute a command, and collect stdout/stderr/exit status.

- [ ] **Step 4: Re-run tests**

Run:
- `~/.cargo/bin/cargo test -p util ssh::tests -- --nocapture`
- `~/.cargo/bin/cargo test -p util --test tp_ssh_client test_ssh_password_exec -- --ignored --nocapture`

Expected: unit tests pass; ignored integration test passes when SSH env is available.

### Task 5: Private key auth

**Files:**
- Modify: `common/util/src/client/ssh.rs`
- Modify: `test/plan/tp_ssh_client.rs`

- [ ] **Step 1: Write ignored private-key integration tests first**

Add tests for:
- private key exec
- passphrase-protected private key exec (if practical)

- [ ] **Step 2: Run one ignored test to confirm red**

Run: `~/.cargo/bin/cargo test -p util --test tp_ssh_client test_ssh_private_key_exec -- --ignored --nocapture`

Expected: failure because private key auth is not implemented yet.

- [ ] **Step 3: Implement private key auth**

Load key file and optional passphrase, then authenticate over the same client flow.

- [ ] **Step 4: Re-run tests**

Run:
- `~/.cargo/bin/cargo test -p util ssh::tests -- --nocapture`
- `~/.cargo/bin/cargo test -p util --test tp_ssh_client test_ssh_private_key_exec -- --ignored --nocapture`

Expected: unit tests pass; ignored integration test passes when SSH env is available.

### Task 6: Upload and download

**Files:**
- Modify: `common/util/src/client/ssh.rs`
- Modify: `test/plan/tp_ssh_client.rs`

- [ ] **Step 1: Write failing transfer tests first**

Add:
- unit test for rejecting missing local upload file
- ignored integration tests for upload and download

- [ ] **Step 2: Run the tests to confirm red**

Run:
- `~/.cargo/bin/cargo test -p util ssh::tests::test_upload_rejects_missing_local_file -- --nocapture`
- `~/.cargo/bin/cargo test -p util --test tp_ssh_client test_upload_file -- --ignored --nocapture`

Expected: failures before transfer implementation exists.

- [ ] **Step 3: Implement path-based upload/download**

Use the chosen SSH file-transfer support to upload a local file and download a remote file.

- [ ] **Step 4: Re-run tests**

Run:
- `~/.cargo/bin/cargo test -p util ssh::tests -- --nocapture`
- `~/.cargo/bin/cargo test -p util --test tp_ssh_client -- --ignored --nocapture`

Expected: unit tests pass; ignored integration tests pass when SSH env is available.

### Task 7: Verification and polish

**Files:**
- Modify: `common/util/src/client/ssh.rs`

- [ ] **Step 1: Add module docs and method docs**

Document the v1 constraints, especially that host key verification is disabled in this version.

- [ ] **Step 2: Run full verification**

Run:
- `~/.cargo/bin/cargo test -p util -- --nocapture`
- `~/.cargo/bin/cargo fmt -- --check`
- `~/.cargo/bin/cargo clippy -p util --all-features -- -D warnings`

Expected: all commands succeed.

- [ ] **Step 3: Run diagnostics and final review**

Check LSP diagnostics on changed files and consult Oracle on the finished implementation before claiming completion.
