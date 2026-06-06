pub mod bootstrap;
pub mod error;
pub mod models;
pub mod protocol;
pub mod repository;

pub mod service;

pub use bootstrap::{
    InstallDecision, InstallMetadata, InstallMetadataStatus, InstallNodeRequest, InstallNodeResult,
    InstallPlugin, InstallRuntimeConfig, InstallStatus, InstallStep, NoopRsAgentInstaller,
    RegistrationWaiter, RemoteExecutor, RepositoryRegistrationWaiter, RsAgentInstaller,
    ShellRemoteExecutor, SshAuth, SshConnectionRequest, SshRsAgentInstaller,
};
pub use error::{NodeManageApiError, NodeManageError, NodeManageErrorCode, Result};
pub use models::{
    BindingState, CreateNode, InstallTaskState, InstallTaskStep, Node, NodeAgentBinding,
    NodeInstallTask, NodeStatus, NodeStatusSnapshot, OnlineStatus, PaginatedResult,
    PaginationParams, UpdateNode,
};
pub use protocol::{
    AgentRegistration, AgentRegistry, AgentRunMode, AgentSyncRequest, AgentSyncResponse,
    HeartbeatConfig, HeartbeatRef, InstallRequestSummary, JobManageConfig, NodeBindingView,
    NodeDetail, NodeInstallTaskReceipt, NodeInstallTaskView, NodeStatusBatchItem, NodeStatusView,
    NodeSummary, RebindNodeRequest, RebindNodeResponse, SyncBindingState, TaskFilterDefaults,
};
pub use repository::{MemoryNodeRepository, MySqlNodeRepository, NodeRepository};
pub use service::NodeManager;
