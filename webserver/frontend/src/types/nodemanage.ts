export interface NodeSummary {
  node_id: string;
  node_name: string;
  environment: string;
  labels: string[];
  lifecycle_state: string;
  install_phase: string;
  binding_state: string;
  online_status: string;
  last_heartbeat_at?: string;
  updated_at: string;
}

export interface PaginatedNodeList {
  items: NodeSummary[];
  total: number;
  page: number;
  page_size: number;
  total_pages: number;
}

export interface NodeBaseInfo {
  node_id: string;
  node_name: string;
  endpoint: string;
  environment: string;
  labels: string[];
  created_at: string;
  updated_at: string;
}

export interface NodeBindingView {
  node_id: string;
  agent_id: string;
  binding_state: string;
  first_registered_at: string;
  last_handshake_at: string;
}

export interface NodeStatusView {
  lifecycle_state: string;
  install_phase: string;
  binding_state: string;
  online_status: string;
  last_heartbeat_at?: string;
  status_reason?: string;
}

export interface InstallRequestSummary {
  host?: string;
  ssh_port?: number;
  username?: string;
  rsagent_package_url?: string;
  install_root?: string;
  labels: string[];
  plugin_names: string[];
}

export interface NodeInstallTaskView {
  install_task_id: string;
  node_id: string;
  task_state: string;
  current_step?: string;
  error_code?: string;
  error_message?: string;
  started_at: string;
  finished_at?: string;
  retryable: boolean;
  request_summary?: InstallRequestSummary;
}

export interface HeartbeatRef {
  data_link_id: string;
  link_purpose: string;
  owner_service: string;
  result_table_name?: string;
}

export interface NodeDetail {
  node: NodeBaseInfo;
  binding?: NodeBindingView;
  status: NodeStatusView;
  latest_install_task?: NodeInstallTaskView;
  heartbeat_ref?: HeartbeatRef;
}

export interface NodeStatusBatchItem {
  node_id: string;
  install_phase: string;
  binding_state: string;
  online_status: string;
  last_heartbeat_at?: string;
  updated_at: string;
  status_reason?: string;
}

export interface InstallPlugin {
  name: string;
  version: string;
  package_url?: string;
}

export interface NodeInstallRequest {
  host: string;
  ssh_port?: number;
  username: string;
  password?: string;
  private_key?: string;
  rsagent_package_url: string;
  install_root?: string;
  register_callback_url?: string;
  plugins?: InstallPlugin[];
  labels?: string[];
}

export interface NodeInstallTaskReceipt {
  install_task_id: string;
  node_id: string;
  accepted: boolean;
  task_state: string;
}

export interface RebindNodeRequest {
  target_agent_id: string;
  reason?: string;
}

export interface RebindNodeResponse {
  accepted: boolean;
  node_id: string;
  target_agent_id: string;
  binding_state: string;
  previous_agent_id?: string;
}

export interface AgentSyncRequest {
  agent_id: string;
  node_id?: string;
  agent_version: string;
  hostname: string;
  os_family: string;
  os_distribution: string;
  arch: string;
  capabilities: string[];
  started_at: string;
  config_version?: string;
}

export interface HeartbeatConfig {
  version: string;
  data_link_id: string;
  vm_base_url: string;
  interval_secs: number;
}

export interface TaskFilterDefaults {
  states: string[];
}

export interface JobManageConfig {
  version: string;
  base_url: string;
  task_filter_defaults: TaskFilterDefaults;
}

export interface AgentSyncResponse {
  accepted: boolean;
  agent_id: string;
  bound_node_id: string;
  binding_state: string;
  agent_run_mode: string;
  config_version: string;
  heartbeat_config: HeartbeatConfig;
  job_manage_config: JobManageConfig;
  sync_interval_secs: number;
  task_sync_interval_secs: number;
  rejection_reason?: string;
}
