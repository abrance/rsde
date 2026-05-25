# Design Spec: nodemanage Persistence and Installer Integration (v1)

- **Date**: 2026-05-25
- **Status**: Draft
- **Scope**: `nodemanage` MySQL persistence, runtime dependency assembly, and real rsagent installation flow

## 1. Overview

`nodemanage` already has a usable domain crate, an Axum route adapter, and in-memory tests, but it is still running with `MemoryNodeRepository` and `NoopRsAgentInstaller` in the live wiring path. That means the component can expose CRUD and install-shaped APIs, but it cannot yet persist node state across restarts or perform a real bootstrap of `rsagent` on a target host.

This spec defines the next stage: keep the existing domain boundaries, add a real MySQL-backed repository, add a real SSH-based installer, and assemble those implementations from config in `apiserver`. The design reuses the existing MySQL instance referenced by Helm values, but isolates `nodemanage` data with its own table scope so testing and rollout do not interfere with other business data.

## 2. Goals

- Replace in-memory node persistence with a real MySQL repository when `[nodemanage.mysql]` is configured.
- Keep a safe fallback to `MemoryNodeRepository` for local/dev scenarios where MySQL is not configured.
- Replace `NoopRsAgentInstaller` in the configured runtime path with an SSH-based installer that performs a real `rsagent` deployment.
- Treat install success as **install completed and agent registration observed**, not merely “SSH command returned 0”.
- Preserve the existing separation of concerns: domain logic in `nodemanage/`, HTTP and dependency assembly in `apiserver/`.
- Make the new path testable with the existing MySQL instance from `helm/rsde/values.yaml`, using isolated `nodemanage` tables.

## 3. Non-Goals

- Building the `nodemanage` frontend in this phase.
- Designing full multi-user authn/authz for node management.
- Adding distributed job orchestration for installs.
- Supporting every possible installation transport beyond SSH in v1.
- Reworking the public HTTP contract unless required by persistence/installer correctness.

## 4. Assumptions

- The existing MySQL instance defined for the deployed stack can be reused for `nodemanage` verification.
- `nodemanage` must use its own table namespace or prefix and must not share mutable tables with unrelated components.
- `rsagent_package_url` remains the package source for online installation, either from config or overridden per request.
- The first acceptable completion bar for the installer is: **SSH install rsagent and required plugins → prepare installation metadata → start agent → wait for `/api/nodes/agent/register` success within timeout**.
- Heartbeat and richer liveness workflows may evolve later; they are not required to mark the install flow complete in v1.

## 5. Current State

### 5.1 Already Present

- `nodemanage/` contains domain models, repository traits, bootstrap abstractions, protocol types, and `NodeManager`.
- `apiserver/src/nodemanage.rs` already exposes health, CRUD, install, heartbeat, status update, and agent registration routes.
- `common/config/src/nodemanage.rs` already models optional MySQL config and installer-related settings.

### 5.2 Current Gaps

- Runtime wiring always uses `MemoryNodeRepository`.
- Runtime wiring always uses `NoopRsAgentInstaller`.
- Node state disappears on restart.
- `/install` does not actually bootstrap a machine.
- The component cannot yet prove the real persistence + install + register path end to end.

## 6. Proposed Design

### 6.1 Architecture Boundary

Keep the existing ownership model:

- `nodemanage/` owns domain logic plus concrete infrastructure implementations:
  - `MySqlNodeRepository`
  - `SshRsAgentInstaller`
- `apiserver/` owns request/response DTOs, route wiring, and dependency assembly.
- `common/config/` owns config types only.

This avoids putting SQL or SSH details into the HTTP layer and keeps future replacements localized.

### 6.2 Repository Strategy

Add `MySqlNodeRepository` under `nodemanage/` as the real implementation of `NodeRepository`.

Responsibilities:

- create node records
- fetch by id
- list paginated nodes
- update node fields
- delete node records
- update node status
- update heartbeat timestamp
- support idempotent agent registration writes

Runtime selection:

- if `NodeManageConfig.mysql` is `Some(...)`, use `MySqlNodeRepository`
- otherwise use `MemoryNodeRepository`

This keeps local bootstrap simple while allowing deployed environments to persist state.

### 6.3 Table Isolation

Use dedicated `nodemanage` tables in the shared MySQL instance.

Recommended rule:

- derive physical table names from `NodeManageConfig.table_prefix`
- keep the default explicit and component-specific, such as `node_manage_`

Minimum first table:

- `${table_prefix}nodes`

Suggested columns for the nodes table:

- `id`
- `name`
- `endpoint`
- `status`
- `labels` (JSON)
- `created_at`
- `updated_at`
- `last_heartbeat_at`

This is intentionally narrow: the repository should persist only what the current domain model already owns.

### 6.4 Registration Semantics

Agent registration must become safe for retries.

Rules:

- registration by `agent_id`/node id must be idempotent
- if a matching managed node already exists, registration updates endpoint, labels, status, and heartbeat-related fields instead of creating duplicates
- successful registration sets the node into an online/manageable state

This prevents install retries or agent restarts from creating duplicate node rows.

### 6.5 Installer Strategy

Add `SshRsAgentInstaller` as the real implementation of `RsAgentInstaller`.

The installer must:

1. establish SSH connection to target host
2. ensure the installation root exists, defaulting to `/opt/rsagent`
3. create the expected directory structure when missing:
   - `bin/`
   - `config/`
   - `plugin/`
4. download or fetch the core `rsagent` package from `rsagent_package_url`
5. download and install the required agent plugins
6. place binaries, config, and plugins into the installation root
7. write the minimal runtime configuration needed for the agent to call back
8. create or update `install.conf` to record installed components, versions, and install state
9. start or restart the agent service/process
10. wait for registration to appear within a bounded timeout

The installer should not mark success immediately after the remote shell script finishes. Success means the node has actually registered back through the `agent/register` path.

### 6.6 Installation Layout and Metadata

The first installation contract for `rsagent` is file-system based and must be deterministic.

Default installation root:

- `/opt/rsagent`

Expected layout after successful install:

- `/opt/rsagent/bin/` — rsagent executable and related launch binaries
- `/opt/rsagent/config/` — runtime config files used by rsagent
- `/opt/rsagent/plugin/` — agent plugin payloads
- `/opt/rsagent/install.conf` — installation metadata and state file

Installer rules:

- if `/opt/rsagent` does not exist, create it
- if subdirectories do not exist, create them
- if `install.conf` exists and records a successful install for the required rsagent version and required plugin versions, skip reinstallation
- if `install.conf` is missing, incomplete, failed, or version-mismatched, perform installation or repair

The design requires installer idempotency. A repeated install request against an already-correct host should become a fast verification path rather than a destructive reinstall.

### 6.7 `install.conf` Contract

`install.conf` is the authoritative local metadata record for rsagent installation state on the target host.

It must record at least:

- rsagent version
- installed plugin list
- plugin versions
- overall installation status
- last install/update time

Recommended semantics:

- `status = installed` means core agent and required plugins are fully installed and ready to start
- `status = failed` means the last install attempt did not complete successfully
- `status = installing` means an install is in progress or was interrupted mid-flight

The exact file format can be TOML, INI, or another simple text format, but it should be stable, human-readable, and easy for the remote installer logic to parse/update safely.

Before performing installation work, the installer should load and evaluate `install.conf`.

Decision logic:

- if file missing → install
- if file exists but status is not successful → repair/reinstall
- if file exists and versions do not match requested or required versions → upgrade/reinstall affected components
- if file exists and all required components are already present with successful state → skip reinstall and move directly to runtime verification/startup if needed

### 6.8 Install Flow Contract

`POST /api/nodes/install` remains the human/admin entrypoint.

Target flow:

1. request enters `apiserver`
2. package URL is completed from config if omitted
3. `NodeManager.install_node()` delegates to `SshRsAgentInstaller`
4. installer verifies installation root and `install.conf`
5. installer either skips install due to successful existing state or performs SSH deployment of rsagent and plugins
6. installer updates `install.conf`
7. installer starts or verifies the agent runtime
8. installed agent calls `/api/nodes/agent/register`
9. repository persists or updates the managed node record
10. installer returns success only after registration is observed

If registration is not observed before timeout, the install result must be reported as failed or timed out, even if the SSH step itself succeeded.

## 7. Error Handling

Errors should be explicit by stage, not flattened into generic install failure text.

### 7.1 Repository Errors

- MySQL connection failure
- schema/table missing
- SQL execution failure
- serialization/deserialization failure for labels
- not found / uniqueness conflict

### 7.2 Installer Errors

- SSH authentication failure
- SSH connection timeout
- installation root creation failure
- remote download failure
- remote unpack/install failure
- plugin installation failure
- `install.conf` read/write/parse failure
- installed version mismatch
- remote service start failure
- registration timeout

### 7.3 API Error Semantics

The API response should preserve enough structured detail for operators to tell whether the failure happened in:

- persistence setup
- SSH connectivity
- deployment execution
- callback registration waiting

The install result should include a stage-like status progression, for example:

- `pending`
- `installing`
- `waiting_register`
- `registered`
- `failed`

The exact enum names can be adapted, but the design requires stage visibility.

## 8. Configuration Direction

`NodeManageConfig` already contains the right starting points. The runtime behavior should become:

- `mysql = None` → in-memory repository + noop installer allowed for dev/test scaffolding
- `mysql = Some(...)` and installer fields configured → MySQL repository + SSH installer

Recommended additions or clarifications if needed:

- agent registration wait timeout
- remote install directory or service name override
- required plugin manifest or plugin package source list
- callback base URL used by the installed agent

These can be added incrementally, but the spec assumes the installer has enough config to tell the remote agent how to reach `apiserver`.

## 9. Testing Strategy

### 9.1 Repository Tests

Add real MySQL integration tests for `MySqlNodeRepository`.

Requirements:

- point tests at the existing MySQL instance available from the deployment config
- use `nodemanage`-specific tables only
- clean test data by prefix or dedicated test table names
- verify create/get/list/update/delete/heartbeat/idempotent registration behavior

This is the main proof that `nodemanage` has moved beyond in-memory state.

### 9.2 Installer Tests

Keep unit tests around the installer boundary by mocking the remote execution layer.

Verify:

- correct handling of password vs private key paths
- package URL propagation
- default install root creation and custom install root override if supported
- expected directory layout creation
- `install.conf` creation on fresh install
- skip behavior when `install.conf` already marks a successful matching install
- reinstall behavior when `install.conf` is failed, incomplete, or version-mismatched
- plugin installation sequencing and version recording
- timeout behavior while waiting for registration
- stage/error mapping

Real SSH verification can be treated as integration validation rather than a mandatory fast test.

### 9.3 Route/Assembly Tests

Add or extend route tests to verify:

- `create_routes()` can still boot with default config
- configured runtime chooses MySQL repository when MySQL config is present
- install route fills package URL from config when missing
- agent registration persists through the configured repository path

## 10. Migration and Rollout

Recommended rollout order:

1. land `MySqlNodeRepository`
2. prove repository tests against the shared MySQL instance with isolated tables
3. add runtime dependency assembly in `apiserver`
4. land `SshRsAgentInstaller`
5. verify install → register flow end to end

This order keeps persistence risk and installer risk separable.

## 11. Trade-offs

### Chosen Design

Shared MySQL instance, isolated `nodemanage` tables, real SSH installer, success gated by registration, deterministic `/opt/rsagent` install layout, and idempotent `install.conf`-driven install behavior.

### Why

- fastest path to a real, testable component
- avoids introducing another database prematurely
- keeps blast radius low through table isolation
- matches the component’s actual business value: managed node lifecycle, not just mock CRUD

### Rejected Alternative A

Keep only `MemoryNodeRepository` and implement installer later.

Why rejected:

- node state remains ephemeral
- deployed behavior still does not match the API promise

### Rejected Alternative B

Implement MySQL repository only, keep noop installer.

Why rejected:

- persistence improves, but the core “online add node” workflow remains fake

### Rejected Alternative C

Create a separate dedicated MySQL instance just for `nodemanage` now.

Why rejected:

- adds operational cost before the component has even validated its first real persistence path
- existing shared instance is sufficient as long as table scope is isolated

## 12. Success Criteria

This phase is complete when all of the following are true:

- `nodemanage` persists nodes in MySQL when configured
- node CRUD and heartbeat survive process restart
- `POST /api/nodes/install` performs a real SSH deployment path for rsagent and required plugins
- target hosts use `/opt/rsagent` as the default install root with `bin/`, `config/`, `plugin/`, and `install.conf`
- successful existing installs are detected from `install.conf` and are not redundantly reinstalled
- install success requires observed agent registration
- tests cover repository behavior against real MySQL tables owned by `nodemanage`
- runtime still supports an in-memory fallback for non-MySQL development scenarios

## 13. Open Questions

- What exact callback URL/config does `rsagent` need in order to register successfully from a target host?
- Should the first MySQL migration be code-driven at startup, or managed externally through SQL/migration files?
- Do we want explicit install job records in v1, or is a single result object enough for now?
- What is the authoritative source of the required plugin list and expected plugin versions for a given install request?

For now, these do not block the architectural direction, but they must be answered during implementation planning.
