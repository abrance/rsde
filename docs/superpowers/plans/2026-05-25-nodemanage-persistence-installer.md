# NodeManage Persistence and Installer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add real MySQL persistence and a real SSH-based rsagent installer to `nodemanage`, including `/opt/rsagent` installation layout, plugin installation, `install.conf`-driven idempotency, and install success gated by agent registration.

**Architecture:** `nodemanage/` keeps ownership of repository and installer implementations. `apiserver/` only assembles concrete dependencies from config and exposes existing routes. Persistence should follow the repository’s current domain model without expanding scope. Installer work should be decomposed into metadata modeling, remote filesystem preparation, package/plugin deployment, `install.conf` handling, and registration wait logic so each part can be tested independently.

**Tech Stack:** Rust 2024, Axum 0.7, mysql_async, tokio, serde, chrono, uuid, TOML/INI-style install metadata, existing `config::mysql::MysqlConfig`

---

## File Structure

- Modify: `nodemanage/Cargo.toml` — add MySQL and any installer/runtime dependencies actually needed by the implementation.
- Modify: `nodemanage/src/lib.rs` — export new repository and installer types.
- Modify: `nodemanage/src/error.rs` — extend error variants to cover MySQL, SSH/install metadata, and registration wait failures.
- Modify: `nodemanage/src/models.rs` — add or refine request/result types only if required by persistence or installer stages.
- Modify: `nodemanage/src/bootstrap.rs` — extend installer request/result modeling for install root, plugin/version metadata, and richer stage/status reporting.
- Modify: `nodemanage/src/repository.rs` — keep trait, preserve memory repository, add MySQL repository implementation and table initialization logic.
- Modify: `nodemanage/src/service.rs` — preserve `NodeManager` boundary and add any idempotent registration/update semantics required by the MySQL path.
- Create: `nodemanage/tests/mysql_repository.rs` — real MySQL integration tests against isolated nodemanage tables.
- Create: `nodemanage/tests/installer.rs` — unit tests for installer metadata, idempotency, and registration wait logic.
- Modify: `common/config/src/nodemanage.rs` — add config fields needed for install root, plugin manifest/source, callback target, and register-wait timeout.
- Modify: `common/config/src/lib.rs` — update config example assertions if defaults change.
- Modify: `config.example.toml` — document new nodemanage config keys.
- Modify: `manifest/dev/remote_ocr.toml` — add nodemanage config needed for local/dev verification.
- Modify: `apiserver/src/nodemanage.rs` — replace fixed memory/noop wiring with runtime assembly helpers while keeping handlers focused on HTTP.
- Modify: `apiserver/src/main.rs` — keep optional route mount behavior, but pass through richer nodemanage config.
- Modify: `apiserver/tests/nodemanage_routes.rs` — extend route tests for config-driven package URL fill and repository-backed registration persistence.

## Task 1: Expand bootstrap and config contracts for real installation

**Files:**
- Modify: `nodemanage/src/bootstrap.rs`
- Modify: `common/config/src/nodemanage.rs`
- Modify: `config.example.toml`
- Modify: `manifest/dev/remote_ocr.toml`
- Test: `nodemanage/tests/domain.rs`

- [ ] **Step 1: Write the failing installer contract tests**

Add tests covering:
- default install root is `/opt/rsagent`
- install result/status can represent `waiting_register`
- install request/result can carry enough metadata for plugin/version-aware installation decisions
- callback/runtime config can carry the registration target that installed `rsagent` must call back to

- [ ] **Step 2: Run test to verify it fails**

Run: `source ~/.bashrc && cargo test -p nodemanage --test domain`
Expected: FAIL because the new install metadata contract does not exist yet.

- [ ] **Step 3: Extend bootstrap models minimally**

Implement the smallest changes needed so installer-facing types can model:
- install root
- required plugin metadata or plugin source list
- richer install status progression
- optional message/stage detail for debugging and API responses

- [ ] **Step 4: Extend NodeManage config minimally**

Add only the config fields required by the approved spec, for example:
- `install_root` with default `/opt/rsagent`
- registration wait timeout
- callback base URL or equivalent registration target for installed agents
- plugin source/manifest setting with explicit precedence rules

- [ ] **Step 5: Define callback and plugin source-of-truth rules**

Write the failing assertions first, then lock down the exact contract:
- where callback URL comes from
- whether required plugins come from request, config, or both
- which side wins when both provide values

- [ ] **Step 6: Update config examples**

Document the new fields in `config.example.toml` and `manifest/dev/remote_ocr.toml` without inventing unrelated config.

- [ ] **Step 7: Run tests again**

Run: `source ~/.bashrc && cargo test -p nodemanage --test domain`
Expected: PASS.

## Task 2: Add MySQL repository while preserving memory fallback

**Files:**
- Modify: `nodemanage/Cargo.toml`
- Modify: `nodemanage/src/lib.rs`
- Modify: `nodemanage/src/error.rs`
- Modify: `nodemanage/src/repository.rs`
- Test: `nodemanage/tests/mysql_repository.rs`

- [ ] **Step 1: Write the failing MySQL repository tests**

Create `nodemanage/tests/mysql_repository.rs` with tests for:
- table initialization uses the configured nodemanage table prefix only
- create/get/list round-trip persists labels and timestamps
- update persists name/status/labels changes
- delete removes rows
- heartbeat-related updates persist correctly

Use a dedicated nodemanage test table prefix such as `node_test_` and clean up only those tables.
State the precondition in the test module comments or helpers:
- connection info comes from the shared MySQL instance used by the deployment config
- tests may only create/drop nodemanage-prefixed tables

- [ ] **Step 2: Run test to verify it fails**

Run: `source ~/.bashrc && cargo test -p nodemanage --test mysql_repository -- --nocapture`
Expected: FAIL because `MySqlNodeRepository` does not exist yet.

- [ ] **Step 3: Add only the required MySQL dependencies**

Follow the repository’s existing style and prefer `mysql_async`, matching `prompt/src/storage.rs`, unless a different dependency is strictly necessary for nodemanage-specific behavior.

- [ ] **Step 4: Implement `MySqlNodeRepository::new` and table initialization**

Implement:
- connection creation from `config::mysql::MysqlConfig`
- physical table naming from `NodeManageConfig.table_prefix`
- `CREATE TABLE IF NOT EXISTS` for `${table_prefix}nodes`

- [ ] **Step 5: Implement repository CRUD/list behavior**

Implement the `NodeRepository` trait for MySQL with the same semantics as the memory repository, including deterministic ordering for pagination.

- [ ] **Step 6: Add label serialization and timestamp conversion**

Keep storage narrow to the current domain model:
- `labels` stored as JSON
- chrono conversions handled explicitly

- [ ] **Step 7: Run the MySQL repository tests**

Run: `source ~/.bashrc && cargo test -p nodemanage --test mysql_repository -- --nocapture`
Expected: PASS against the shared MySQL instance using isolated nodemanage tables.

## Task 3: Make registration idempotent across repository implementations

**Files:**
- Modify: `nodemanage/src/repository.rs`
- Modify: `nodemanage/src/service.rs`
- Modify: `nodemanage/tests/service.rs`
- Modify: `nodemanage/tests/mysql_repository.rs`

- [ ] **Step 1: Write the failing idempotent registration tests**

Cover:
- repeated registration for the same agent id does not create duplicates
- repeated registration updates endpoint/labels/status instead of inserting a second node
- memory and MySQL paths both preserve this behavior

- [ ] **Step 2: Run test to verify it fails**

Run: `source ~/.bashrc && cargo test -p nodemanage --test service register_agent_creates_online_node_record -- --nocapture`
Expected: FAIL or prove current behavior is insufficient for idempotent registration.

- [ ] **Step 3: Implement the smallest repository/service change**

Choose one boundary and keep it consistent:
- either extend the repository with an upsert-like registration method
- or make `NodeManager.register_agent` fetch/update before create

Do not duplicate registration policy in multiple layers.

- [ ] **Step 4: Re-run service and MySQL tests**

Run:
- `source ~/.bashrc && cargo test -p nodemanage --test service -- --nocapture`
- `source ~/.bashrc && cargo test -p nodemanage --test mysql_repository -- --nocapture`

Expected: PASS.

## Task 4: Model `install.conf` and installation idempotency logic

**Files:**
- Modify: `nodemanage/src/bootstrap.rs`
- Modify: `nodemanage/src/error.rs`
- Create or Modify: installer helper code under `nodemanage/src/` (split into focused files if needed)
- Test: `nodemanage/tests/installer.rs`

- [ ] **Step 1: Write the failing install metadata tests**

Create `nodemanage/tests/installer.rs` covering:
- install metadata parses a successful existing install
- version mismatch forces reinstall
- failed/incomplete status forces repair
- missing `install.conf` triggers full install path
- plugin version mismatch triggers plugin reinstall even when core rsagent is already present

- [ ] **Step 2: Run test to verify it fails**

Run: `source ~/.bashrc && cargo test -p nodemanage --test installer -- --nocapture`
Expected: FAIL because install metadata logic does not exist yet.

- [ ] **Step 3: Implement a focused install metadata model**

Represent:
- rsagent version
- plugin names and versions
- overall install state
- last update time

Keep the format human-readable and stable. Do not combine remote execution and metadata parsing into one large file.

- [ ] **Step 4: Implement decision logic for skip/repair/reinstall**

Add pure functions that decide:
- skip install
- install missing components
- reinstall failed/incomplete state
- reinstall or upgrade version-mismatched components

- [ ] **Step 5: Re-run installer metadata tests**

Run: `source ~/.bashrc && cargo test -p nodemanage --test installer -- --nocapture`
Expected: PASS.

## Task 5: Implement SSH installer filesystem preparation and package/plugin deployment

**Files:**
- Modify or Create: focused installer execution files under `nodemanage/src/`
- Modify: `nodemanage/src/lib.rs`
- Test: `nodemanage/tests/installer.rs`

- [ ] **Step 1: Write the failing remote layout tests**

Add tests for the installer execution layer covering:
- `/opt/rsagent` is used by default
- `bin/`, `config/`, and `plugin/` directories are created when missing
- rsagent package and plugin installation steps are issued in the expected order
- `install.conf` write happens after component placement and before startup verification
- password authentication and private-key authentication are both translated into the remote execution layer correctly

- [ ] **Step 2: Run test to verify it fails**

Run: `source ~/.bashrc && cargo test -p nodemanage --test installer -- --nocapture`
Expected: FAIL because the SSH installer execution flow does not exist yet.

- [ ] **Step 3: Introduce a mockable remote execution boundary**

Add a narrow abstraction for remote commands/file operations so installer logic can be unit-tested without a live SSH host.

- [ ] **Step 4: Implement the filesystem preparation steps**

Implement only:
- installation root creation
- subdirectory creation
- package/plugin placement hooks

- [ ] **Step 5: Implement package and plugin deployment sequencing**

Ensure the installer can express:
- core rsagent deployment
- plugin deployment
- metadata write/update
- service start/restart

- [ ] **Step 6: Implement callback/runtime config generation**

Generate the remote runtime config that tells `rsagent` where `/api/nodes/agent/register` lives, using the source-of-truth contract defined in Task 1.

- [ ] **Step 7: Re-run installer tests**

Run: `source ~/.bashrc && cargo test -p nodemanage --test installer -- --nocapture`
Expected: PASS.

## Task 6: Gate install success on observed registration

**Files:**
- Modify: `nodemanage/src/bootstrap.rs`
- Modify: `nodemanage/src/service.rs`
- Modify or Create: focused installer runtime files under `nodemanage/src/`
- Modify: `nodemanage/tests/installer.rs`

- [ ] **Step 1: Write the failing registration-wait tests**

Cover:
- successful install path transitions to `waiting_register` before `registered`
- missing registration within timeout returns failure/timeout
- already-installed hosts still perform runtime verification if necessary

- [ ] **Step 2: Run test to verify it fails**

Run: `source ~/.bashrc && cargo test -p nodemanage --test installer -- --nocapture`
Expected: FAIL because registration wait handling is not implemented.

- [ ] **Step 3: Implement registration wait behavior minimally**

Implement a bounded wait mechanism that watches for the registration side effect rather than assuming deployment success.

- [ ] **Step 4: Ensure install result stages are observable**

Return enough status/message detail so operators can distinguish deployment failure from registration timeout.

- [ ] **Step 5: Re-run installer and service tests**

Run:
- `source ~/.bashrc && cargo test -p nodemanage --test installer -- --nocapture`
- `source ~/.bashrc && cargo test -p nodemanage --test service -- --nocapture`

Expected: PASS.

## Task 7: Wire runtime assembly in apiserver

**Files:**
- Modify: `apiserver/src/nodemanage.rs`
- Modify: `apiserver/src/main.rs`
- Modify: `apiserver/tests/nodemanage_routes.rs`

- [ ] **Step 1: Write the failing assembly/route tests**

Add tests covering:
- package URL is filled from config when omitted in install request
- nodemanage route creation still works with default config
- route assembly can use configured persistence instead of fixed memory/noop wiring
- agent registration persists through the configured repository path

- [ ] **Step 2: Run test to verify it fails**

Run: `source ~/.bashrc && cargo test -p apiserver --test nodemanage_routes -- --nocapture`
Expected: FAIL because route assembly is still hard-coded.

- [ ] **Step 3: Refactor dependency assembly out of handler logic**

Introduce a small factory/constructor path so `create_routes` can choose:
- memory repository + noop installer
- MySQL repository + SSH installer

Keep handlers focused on HTTP serialization and errors.

- [ ] **Step 4: Re-run route tests**

Run: `source ~/.bashrc && cargo test -p apiserver --test nodemanage_routes -- --nocapture`
Expected: PASS.

## Task 8: Add an early assembly/config contract check

**Files:**
- Modify: `apiserver/tests/nodemanage_routes.rs`
- Modify: `common/config/src/nodemanage.rs`

- [ ] **Step 1: Write a failing config assembly test early**

Add a test that proves a config carrying MySQL settings, callback target, plugin source, and installer timeout can be loaded and used to construct nodemanage runtime dependencies without silent fallback.

- [ ] **Step 2: Run test to verify it fails**

Run: `source ~/.bashrc && cargo test -p apiserver --test nodemanage_routes -- --nocapture`
Expected: FAIL because the runtime assembly/config contract is still incomplete.

- [ ] **Step 3: Implement the minimal assembly/config support**

Land only enough assembly logic to stabilize the config contract before most installer work depends on it.

- [ ] **Step 4: Re-run the config assembly test**

Run: `source ~/.bashrc && cargo test -p apiserver --test nodemanage_routes -- --nocapture`
Expected: PASS.

## Task 9: End-to-end verification and cleanup

**Files:** all touched files.

- [ ] **Step 1: Run formatter**

Run: `source ~/.bashrc && cargo fmt -- --check`
Expected: PASS.

- [ ] **Step 2: Run nodemanage test suites**

Run:
- `source ~/.bashrc && cargo test -p nodemanage --test domain -- --nocapture`
- `source ~/.bashrc && cargo test -p nodemanage --test service -- --nocapture`
- `source ~/.bashrc && cargo test -p nodemanage --test mysql_repository -- --nocapture`
- `source ~/.bashrc && cargo test -p nodemanage --test installer -- --nocapture`

Expected: PASS.

- [ ] **Step 3: Run route verification**

Run: `source ~/.bashrc && cargo test -p apiserver --test nodemanage_routes -- --nocapture`
Expected: PASS.

- [ ] **Step 4: Compile impacted packages**

Run:
- `source ~/.bashrc && cargo check -p nodemanage`
- `source ~/.bashrc && cargo check -p apiserver`

Expected: PASS, except for any clearly pre-existing unrelated warnings.

- [ ] **Step 5: Perform one manual install-path verification checklist**

Verify in a controlled environment that:
- target host gets `/opt/rsagent`
- `bin/`, `config/`, `plugin/`, and `install.conf` exist
- a successful `install.conf` prevents redundant reinstall
- agent registration appears in nodemanage-backed storage

- [ ] **Step 6: Commit in small logical batches**

Use frequent commits that follow the repository style, for example:
- `feat: add nodemanage mysql repository`
- `feat: add rsagent install metadata handling`
- `feat: wire nodemanage ssh installer`
