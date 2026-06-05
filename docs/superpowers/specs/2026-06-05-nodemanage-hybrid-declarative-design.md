# NodeManage Hybrid Declarative Design Specification

## 1. Overview
Evaluate and design a hybrid declarative model for NodeManage using `kube-rs` design patterns while maintaining current non-K8s infrastructure (MySQL/Memory).

## 2. Architecture
The system will adopt a "Control Plane" architecture:
- **API Server**: Exposes declarative endpoints (`/api/v1/nodes`).
- **Store**: MySQL/Memory repository storing full Resource JSON (Spec + Status).
- **Reconciliation Engine**: An internal background worker that observes state changes and executes actions (Install, Bind, Sync).

## 3. Data Model
Leverage `kube-derive` for standard metadata and structure.

### NodeResource
- **Metadata**: id, name, labels, annotations, generation.
- **Spec**:
    - `management`: managed (bool), owner.
    - `endpoint`: address, port, auth_ref.
    - `install`: desired (bool), package_url, root_dir.
    - `binding`: mode (auto|pinned), agent_id_expectation.
- **Status**:
    - `reconcile_state`: Success, Progressing, Failed.
    - `observed_generation`: Latest reconciled version.
    - `install`: phase, result, task_ref.
    - `runtime`: online_status, last_heartbeat_at.
    - `conditions`: List of standard K8s-style conditions.

## 4. Components
- **Declarative API Handler**: Converts Axum requests into `apply` operations on the repository.
- **Background Reconciler**: 
    - Polls repository for `generation != observed_generation`.
    - Dispatches tasks to existing `SshRsAgentInstaller` and `NodeManager`.
- **Status Syncer**: Updates `Status` fields based on Query-Engine heartbeats and task completions.

## 5. Integration with Existing Code
- **NodeManage Crate**: Will hold the new `Kube`-based resource definitions.
- **apiserver**: Adds new routes while keeping legacy routes as transitional.
- **MySQL Repository**: Migration needed to add `spec`, `status`, `generation`, and `observed_generation` columns.

## 6. Trade-offs
- **Pros**: Standardization, clean separation of intent vs. action, scalable state management.
- **Cons**: Requires custom reconciliation logic (cannot use `kube-runtime` directly), added complexity in internal eventing.
