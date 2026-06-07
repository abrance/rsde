import type {
    AgentSyncPayload,
    AgentSyncResponse,
    CreateNodePayload,
    InstallTask,
    NodeBinding,
    NodeBindingState,
    NodeDetail,
    NodeInstallPhase,
    NodeListResponse,
    NodeOnlineStatus,
    NodeRecord,
    NodeStatusBatch,
    RebindNodePayload,
} from '../types/nodemanage'

type InstallNodePayload = Record<string, unknown>

type ApiError = {
    code?: string
    message?: string
    retryable?: boolean
    details?: unknown
}

type ApiEnvelope<T> = {
    success: boolean
    data?: T | null
    error?: ApiError | string | null
}

type LegacyNode = {
    id: string
    name: string
    endpoint?: string
    labels?: string[]
    status?: string
    updated_at?: string
    last_heartbeat_at?: string | null
}

type LegacyPaginated<T> = {
    items?: T[]
    total?: number
}

function toErrorMessage(error: ApiEnvelope<unknown>['error'], fallback: string): string {
    if (!error) return fallback
    if (typeof error === 'string') return error
    return error.message || error.code || fallback
}

export class TransportError extends Error {
    public code?: string
    public status: number

    constructor(message: string, status: number, code?: string) {
        super(message)
        this.name = 'TransportError'
        this.status = status
        this.code = code
    }
}

async function requestJson<T>(input: string, init?: RequestInit): Promise<T> {
    const headers = new Headers(init?.headers)
    if (!headers.has('Content-Type')) {
        headers.set('Content-Type', 'application/json')
    }

    const response = await fetch(input, {
        ...init,
        headers,
    })

    const text = await response.text()
    let json: ApiEnvelope<T> | null = null
    
    if (text) {
        try {
            json = JSON.parse(text) as ApiEnvelope<T>
        } catch (err) {
            const message = err instanceof Error ? err.message : String(err)
            throw new TransportError(`Invalid JSON response: ${message}`, response.status, 'INVALID_JSON')
        }
    }

    if (!response.ok || !json?.success) {
        const errorObj = typeof json?.error === 'object' ? json?.error : null;
        const code = errorObj?.code;
        const msg = toErrorMessage(json?.error, `Request failed: ${response.status}`)
        throw new TransportError(msg, response.status, code)
    }

    if (json.data == null) {
        throw new TransportError('Response missing data', response.status, 'MISSING_DATA')
    }

    return json.data
}

function parseOnlineStatus(val: string | undefined): NodeOnlineStatus | undefined {
    if (val === 'online' || val === 'offline' || val === 'unknown') return val
    return undefined
}

function parseBindingState(val: string | undefined): NodeBindingState | undefined {
    if (val === 'BOUND' || val === 'UNBOUND' || val === 'BINDING' || val === 'ERROR' || val === 'UNKNOWN') return val
    return undefined
}

function parseInstallPhase(val: string | undefined): NodeInstallPhase | undefined {
    if (val === 'PENDING' || val === 'RUNNING' || val === 'COMPLETED' || val === 'FAILED' || val === 'UNKNOWN') return val
    return undefined
}

function mapLegacyNode(node: LegacyNode): NodeRecord {
    return {
        id: node.id,
        name: node.name,
        endpoint: node.endpoint,
        labels: node.labels,
        onlineStatus: parseOnlineStatus(node.status),
        updatedAt: node.updated_at,
        lastHeartbeatAt: node.last_heartbeat_at,
    }
}

export async function fetchNodes(): Promise<NodeListResponse> {
    const data = await requestJson<LegacyPaginated<LegacyNode>>('/api/nm/v1/nodes')
    const items = data.items || []
    return {
        nodes: items.map(mapLegacyNode),
        total: data.total ?? items.length,
    }
}

export async function createNode(payload: CreateNodePayload): Promise<NodeDetail> {
    const data = await requestJson<LegacyNode>('/api/nm/v1/nodes', {
        method: 'POST',
        body: JSON.stringify(payload),
    })

    return {
        ...mapLegacyNode(data),
    }
}

export async function fetchNodeDetail(nodeId: string): Promise<NodeDetail> {
    const data = await requestJson<LegacyNode>(`/api/nm/v1/nodes/${nodeId}`)
    return {
        ...mapLegacyNode(data),
    }
}

type RawNodeStatusBatchItem = {
    node_id: string
    status?: string
    binding_state?: string
    install_phase?: string
    updated_at?: string
    last_heartbeat_at?: string | null
}

export async function fetchNodeStatusBatch(nodeIds: string[]): Promise<NodeStatusBatch> {
    if (!nodeIds || nodeIds.length === 0) {
        return {}
    }

    const query = new URLSearchParams({ node_ids: nodeIds.join(',') })
    const rawItems = await requestJson<RawNodeStatusBatchItem[]>(`/api/nm/v1/nodes/status:batch?${query.toString()}`)
    
    const result: NodeStatusBatch = {}
    for (const item of rawItems) {
        result[item.node_id] = {
            id: item.node_id,
            onlineStatus: parseOnlineStatus(item.status),
            bindingState: parseBindingState(item.binding_state),
            installPhase: parseInstallPhase(item.install_phase),
            updatedAt: item.updated_at,
            lastHeartbeatAt: item.last_heartbeat_at,
        }
    }
    return result
}

export async function fetchNodeBinding(nodeId: string): Promise<NodeBinding> {
    const data = await requestJson<{ state?: string; agent_id?: string }>(`/api/nm/v1/nodes/${nodeId}/binding`)
    return {
        state: parseBindingState(data.state),
        agentId: data.agent_id,
    }
}

export async function installNodeAgent(nodeId: string, payload?: InstallNodePayload): Promise<InstallTask> {
    return requestJson<InstallTask>(`/api/nm/v1/nodes/${nodeId}/install`, {
        method: 'POST',
        body: JSON.stringify(payload || {}),
    })
}

export async function fetchInstallTask(taskId: string): Promise<InstallTask> {
    return requestJson<InstallTask>(`/api/nm/v1/install-tasks/${taskId}`)
}

export async function fetchLatestInstallTask(nodeId: string): Promise<InstallTask | null> {
    try {
        return await requestJson<InstallTask>(`/api/nm/v1/nodes/${nodeId}/install-tasks:latest`)
    } catch (err) {
        if (err instanceof TransportError && (err.status === 404 || err.code === 'NODE_NOT_FOUND' || err.code === 'TASK_NOT_FOUND')) {
            return null
        }
        throw err
    }
}

export async function rebindNode(nodeId: string, payload: RebindNodePayload): Promise<void> {
    await requestJson<Record<string, unknown>>(`/api/nm/v1/nodes/${nodeId}/rebind`, {
        method: 'POST',
        body: JSON.stringify({
            target_agent_id: payload.targetAgentId,
            reason: payload.reason,
        }),
    })
}

export async function syncAgent(payload: AgentSyncPayload): Promise<AgentSyncResponse> {
    return requestJson<AgentSyncResponse>('/api/nm/v1/agents/sync', {
        method: 'POST',
        body: JSON.stringify({
            agent_id: payload.agentId,
        }),
    })
}
