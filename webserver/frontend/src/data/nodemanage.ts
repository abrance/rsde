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

type V1NodeSummary = {
    node_id: string
    node_name: string
    endpoint?: string
    labels?: string[]
    binding_state?: string
    install_phase?: string
    online_status?: string
    updated_at?: string
    last_heartbeat_at?: string | null
}

type V1NodeBaseInfo = {
    node_id: string
    node_name: string
    endpoint?: string
    labels?: string[]
    updated_at?: string
}

type V1NodeBindingView = {
    agent_id?: string
    binding_state?: string
}

type V1NodeStatusView = {
    binding_state?: string
    install_phase?: string
    online_status?: string
    updated_at?: string
    last_heartbeat_at?: string | null
}

type V1NodeInstallTaskView = {
    install_task_id: string
    task_state?: string
    error_message?: string | null
}

type V1NodeDetail = {
    node: V1NodeBaseInfo
    binding?: V1NodeBindingView | null
    status: V1NodeStatusView
    latest_install_task?: V1NodeInstallTaskView | null
}

type RawNodeBinding = {
    state?: string
    agent_id?: string
    binding_state?: string
}

type InstallTaskPayload =
    | InstallTask
    | {
          id?: string
          status?: string
          message?: string
          install_task_id?: string
          task_state?: string
          error_message?: string | null
      }

type InstallReceiptPayload = {
    id?: string
    status?: string
    message?: string
    install_task_id?: string
    task_state?: string
    error_message?: string | null
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
    if (val === 'maintenance') return 'unknown'
    return undefined
}

function parseBindingState(val: string | undefined): NodeBindingState | undefined {
    if (val === 'BOUND' || val === 'UNBOUND' || val === 'BINDING' || val === 'ERROR' || val === 'UNKNOWN') return val
    if (val === 'bound') return 'BOUND'
    if (val === 'unbound') return 'UNBOUND'
    if (val === 'binding') return 'BINDING'
    if (val === 'error') return 'ERROR'
    if (val === 'unknown') return 'UNKNOWN'
    return undefined
}

function parseInstallPhase(val: string | undefined): NodeInstallPhase | undefined {
    if (val === 'PENDING' || val === 'RUNNING' || val === 'COMPLETED' || val === 'FAILED' || val === 'UNKNOWN') return val
    if (val === 'pending' || val === 'not_started') return 'PENDING'
    if (val === 'running') return 'RUNNING'
    if (val === 'completed' || val === 'succeeded') return 'COMPLETED'
    if (val === 'failed') return 'FAILED'
    if (val === 'unknown') return 'UNKNOWN'
    return undefined
}

function mapNodeRecord(node: LegacyNode | V1NodeSummary | V1NodeBaseInfo, status?: V1NodeStatusView): NodeRecord {
    const rawId = 'node_id' in node ? node.node_id : node.id
    const rawName = 'node_name' in node ? node.node_name : node.name
    const rawEndpoint = 'endpoint' in node ? node.endpoint : undefined
    const rawLabels = 'labels' in node ? node.labels : undefined
    const rawBindingState = status?.binding_state ?? ('binding_state' in node ? node.binding_state : undefined)
    const rawInstallPhase = status?.install_phase ?? ('install_phase' in node ? node.install_phase : undefined)
    const rawOnlineStatus = status?.online_status ?? ('online_status' in node ? node.online_status : 'status' in node ? node.status : undefined)
    const rawUpdatedAt = status?.updated_at ?? node.updated_at
    const rawLastHeartbeatAt = status?.last_heartbeat_at ?? ('last_heartbeat_at' in node ? node.last_heartbeat_at : undefined)

    return {
        id: rawId,
        name: rawName,
        endpoint: rawEndpoint,
        labels: rawLabels,
        bindingState: parseBindingState(rawBindingState),
        installPhase: parseInstallPhase(rawInstallPhase),
        onlineStatus: parseOnlineStatus(rawOnlineStatus),
        updatedAt: rawUpdatedAt,
        lastHeartbeatAt: rawLastHeartbeatAt,
    }
}

function mapInstallTask(task: InstallTaskPayload): InstallTask {
    return {
        id: 'install_task_id' in task && typeof task.install_task_id === 'string' ? task.install_task_id : task.id ?? '',
        status: parseInstallPhase('task_state' in task ? task.task_state : task.status),
        message: 'error_message' in task ? task.error_message ?? undefined : task.message,
    }
}

export async function fetchNodes(): Promise<NodeListResponse> {
    const data = await requestJson<LegacyPaginated<LegacyNode | V1NodeSummary>>('/api/nm/v1/nodes')
    const items = data.items || []
    return {
        nodes: items.map(item => mapNodeRecord(item)),
        total: data.total ?? items.length,
    }
}

export async function createNode(payload: CreateNodePayload): Promise<NodeDetail> {
    const data = await requestJson<LegacyNode>('/api/nm/v1/nodes', {
        method: 'POST',
        body: JSON.stringify(payload),
    })

    return {
        ...mapNodeRecord(data),
    }
}

export async function fetchNodeDetail(nodeId: string): Promise<NodeDetail> {
    const data = await requestJson<LegacyNode | V1NodeDetail>(`/api/nm/v1/nodes/${nodeId}`)
    if ('node' in data) {
        return {
            ...mapNodeRecord(data.node, data.status),
            binding: data.binding
                ? {
                      state: parseBindingState(data.binding.binding_state),
                      agentId: data.binding.agent_id,
                  }
                : null,
            latestInstallTask: data.latest_install_task ? mapInstallTask(data.latest_install_task) : null,
        }
    }

    return {
        ...mapNodeRecord(data),
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
    const data = await requestJson<RawNodeBinding>(`/api/nm/v1/nodes/${nodeId}/binding`)
    return {
        state: parseBindingState(data.binding_state ?? data.state),
        agentId: data.agent_id,
    }
}

export async function installNodeAgent(nodeId: string, payload?: InstallNodePayload): Promise<InstallTask> {
    const data = await requestJson<InstallReceiptPayload>(`/api/nm/v1/nodes/${nodeId}/install`, {
        method: 'POST',
        body: JSON.stringify(payload || {}),
    })
    return mapInstallTask(data)
}

export async function fetchInstallTask(taskId: string): Promise<InstallTask> {
    const data = await requestJson<InstallTaskPayload>(`/api/nm/v1/install-tasks/${taskId}`)
    return mapInstallTask(data)
}

export async function fetchLatestInstallTask(nodeId: string): Promise<InstallTask | null> {
    try {
        const data = await requestJson<InstallTaskPayload>(`/api/nm/v1/nodes/${nodeId}/install-tasks:latest`)
        return mapInstallTask(data)
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
