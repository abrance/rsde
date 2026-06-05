# NodeManage Hybrid Declarative Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transition NodeManage to a hybrid declarative system using `kube-rs` for resource modeling and a custom reconciliation loop.

**Architecture:** 
- Uses `kube-derive` to define `NodeResource`.
- Stores Spec/Status JSON in MySQL with Generation tracking.
- Implements a background reconciler that synchronizes the "Observed Generation" with the "Desired Generation".

**Tech Stack:** Rust, kube-rs, Axum, MySQL (sqlx), Tokio.

---

### Task 1: Define Declarative Models

**Files:**
- Create: `nodemanage/src/models/resource.rs`
- Modify: `nodemanage/src/lib.rs`

- [ ] **Step 1: Add dependencies to nodemanage/Cargo.toml**
Add `kube`, `k8s-openapi`, `schemars`.

- [ ] **Step 2: Create NodeResource model**
Implement `NodeSpec` and `NodeStatus` with `#[derive(CustomResource)]`.

- [ ] **Step 3: Commit**
`git add . && git commit -m "feat(nodemanage): add declarative NodeResource models"`

### Task 2: Update MySQL Repository

**Files:**
- Modify: `nodemanage/src/repository/mysql.rs`
- Create: `migrations/YYYYMMDD_add_declarative_fields.sql`

- [ ] **Step 1: Create migration script**
Add `spec`, `status`, `generation`, `observed_generation` columns to `nodes` table.

- [ ] **Step 2: Update Repository trait and MySQL implementation**
Implement `apply_resource(resource)` and `update_status(id, status, observed_generation)`.

- [ ] **Step 3: Test repository changes**
Verify that JSON fields are correctly serialized/deserialized.

- [ ] **Step 4: Commit**
`git add . && git commit -m "feat(nodemanage): update mysql repository for declarative fields"`

### Task 3: Implement Background Reconciler

**Files:**
- Create: `nodemanage/src/reconciler/mod.rs`

- [ ] **Step 1: Define Reconciler state machine**
Implement a function `reconcile(node)` that compares `generation` vs `observed_generation`.

- [ ] **Step 2: Implement background worker loop**
Create a tokio task that polls the repository for "dirty" nodes.

- [ ] **Step 3: Commit**
`git add . && git commit -m "feat(nodemanage): add background reconciliation engine"`

### Task 4: Add Declarative API Routes

**Files:**
- Modify: `apiserver/src/nodemanage.rs`

- [ ] **Step 1: Add Apply/Query routes**
Add `PUT /api/v1/nodes/{id}`, `GET /api/v1/nodes`, etc.

- [ ] **Step 2: Integration Test**
Write a test in `apiserver/tests/nodemanage_declarative_test.rs` verifying the full loop (Apply -> Reconcile -> Status Update).

- [ ] **Step 3: Commit**
`git add . && git commit -m "feat(apiserver): add declarative NodeManage API routes"`
