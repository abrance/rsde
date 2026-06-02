export type JobStatus = 'pending' | 'running' | 'success' | 'failed' | 'cancelled'

export type JobFilter = 'all' | JobStatus

export type JobPrecheck = {
    allowed: boolean
    reason: string
    checkedAt: string
}

export type JobRecord = {
    id: string
    name: string
    nodeId: string
    nodeName: string
    status: JobStatus
    createdAt: string
    updatedAt: string
    startedAt?: string | null
    finishedAt?: string | null
    owner: string
    description: string
    lastError?: string | null
    precheck?: JobPrecheck | null
}

export type JobActionMessage = {
    kind: 'success' | 'error' | 'info'
    text: string
}

export type JobStatusCounts = Record<JobStatus, number>
