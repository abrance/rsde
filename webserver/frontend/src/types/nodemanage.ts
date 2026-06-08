export type NodeOnlineStatus = 'online' | 'offline' | 'unknown'
export type NodeBindingState = 'BOUND' | 'UNBOUND' | 'BINDING' | 'ERROR' | 'UNKNOWN'
export type NodeInstallPhase = 'NOT_STARTED' | 'PENDING' | 'RUNNING' | 'COMPLETED' | 'SUCCEEDED' | 'FAILED' | 'UNKNOWN'

export interface NodeRecord {
    id: string
    name: string
    endpoint?: string
    labels?: string[]
    bindingState?: NodeBindingState
    installPhase?: NodeInstallPhase
    onlineStatus?: NodeOnlineStatus
    updatedAt?: string
    lastHeartbeatAt?: string | null
}

export interface NodeListResponse {
    nodes: NodeRecord[]
    total: number
}

export interface NodeDetail extends NodeRecord {
    binding?: NodeBinding | null;
    latestInstallTask?: InstallTask | null;
}

export interface NodeBinding {
    state?: NodeBindingState;
    agentId?: string;
}

export interface InstallTask {
    id: string;
    status?: NodeInstallPhase;
    message?: string;
}

export interface CreateNodePayload {
    name: string;
    endpoint: string;
    labels?: string[];
}

export interface NodeStatusBatchItem {
    id: string;
    onlineStatus?: NodeOnlineStatus;
    bindingState?: NodeBindingState;
    installPhase?: NodeInstallPhase;
    updatedAt?: string;
    lastHeartbeatAt?: string | null;
}

export type NodeStatusBatch = Record<string, NodeStatusBatchItem>

export interface RebindNodePayload {
    targetAgentId: string;
    reason?: string;
}

export interface AgentSyncPayload {
    agentId: string;
}

export interface AgentSyncResponse {
    accepted?: boolean;
}
