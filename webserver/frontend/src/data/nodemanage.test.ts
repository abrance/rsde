import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
    createNode,
    fetchLatestInstallTask,
    fetchNodeBinding,
    fetchNodeDetail,
    fetchNodeStatusBatch,
    fetchNodes,
    installNodeAgent,
    rebindNode,
    syncAgent,
} from './nodemanage'

type MockResponseOptions = {
    ok?: boolean
    status?: number
    body?: unknown
}

function makeResponse({ ok = true, status = 200, body }: MockResponseOptions): Response {
    return {
        ok,
        status,
        text: vi.fn().mockResolvedValue(body == null ? '' : JSON.stringify(body)),
    } as unknown as Response
}

describe('nodemanage transport', () => {
    const fetchMock = vi.fn()

    beforeEach(() => {
        fetchMock.mockReset()
        vi.stubGlobal('fetch', fetchMock)
    })

    afterEach(() => {
        vi.unstubAllGlobals()
    })

    it('fetchNodes maps v1 node summary pages into frontend node records', async () => {
        fetchMock.mockResolvedValue(
            makeResponse({
                body: {
                    success: true,
                    data: {
                        items: [
                            {
                                node_id: 'node-1',
                                node_name: 'worker-1',
                                environment: 'prod',
                                labels: ['edge'],
                                binding_state: 'bound',
                                install_phase: 'running',
                                online_status: 'offline',
                                updated_at: '2026-06-07T08:00:00Z',
                                last_heartbeat_at: null,
                            },
                        ],
                        total: 1,
                    },
                },
            }),
        )

        await expect(fetchNodes()).resolves.toEqual({
            nodes: [
                {
                    id: 'node-1',
                    name: 'worker-1',
                    endpoint: undefined,
                    labels: ['edge'],
                    bindingState: 'BOUND',
                    installPhase: 'RUNNING',
                    onlineStatus: 'offline',
                    updatedAt: '2026-06-07T08:00:00Z',
                    lastHeartbeatAt: null,
                },
            ],
            total: 1,
        })

        expect(fetchMock).toHaveBeenCalledWith('/api/nm/v1/nodes', expect.any(Object))
    })

    it('createNode posts payload to nm v1 endpoint', async () => {
        fetchMock.mockResolvedValue(
            makeResponse({
                body: {
                    success: true,
                    data: {
                        id: 'node-2',
                        name: 'worker-2',
                        endpoint: 'http://worker-2:8080',
                        labels: ['gpu'],
                    },
                },
            }),
        )

        await expect(
            createNode({
                name: 'worker-2',
                endpoint: 'http://worker-2:8080',
                labels: ['gpu'],
            }),
        ).resolves.toMatchObject({
            id: 'node-2',
            name: 'worker-2',
            endpoint: 'http://worker-2:8080',
            labels: ['gpu'],
        })

        expect(fetchMock).toHaveBeenCalledWith(
            '/api/nm/v1/nodes',
            expect.objectContaining({
                method: 'POST',
                body: JSON.stringify({
                    name: 'worker-2',
                    endpoint: 'http://worker-2:8080',
                    labels: ['gpu'],
                }),
            }),
        )
    })

    it('fetchNodeDetail maps v1 aggregate payloads into frontend detail records', async () => {
        fetchMock.mockResolvedValue(
            makeResponse({
                body: {
                    success: true,
                    data: {
                        node: {
                            node_id: 'node-3',
                            node_name: 'worker-3',
                            endpoint: 'http://worker-3:8080',
                            environment: 'prod',
                            labels: ['gpu'],
                            created_at: '2026-06-07T07:00:00Z',
                            updated_at: '2026-06-07T08:00:00Z',
                        },
                        binding: {
                            node_id: 'node-3',
                            agent_id: 'agent-3',
                            binding_state: 'bound',
                            first_registered_at: '2026-06-07T07:10:00Z',
                            last_handshake_at: '2026-06-07T08:10:00Z',
                        },
                        status: {
                            lifecycle_state: 'ready',
                            install_phase: 'succeeded',
                            binding_state: 'bound',
                            online_status: 'online',
                            last_heartbeat_at: '2026-06-07T08:09:00Z',
                            status_reason: null,
                        },
                        latest_install_task: {
                            install_task_id: 'task-3',
                            node_id: 'node-3',
                            task_state: 'running',
                            error_message: null,
                            started_at: '2026-06-07T08:00:00Z',
                            finished_at: null,
                            retryable: true,
                        },
                    },
                },
            }),
        )

        await expect(fetchNodeDetail('node-3')).resolves.toMatchObject({
            id: 'node-3',
            name: 'worker-3',
            endpoint: 'http://worker-3:8080',
            labels: ['gpu'],
            bindingState: 'BOUND',
            installPhase: 'COMPLETED',
            onlineStatus: 'online',
            updatedAt: '2026-06-07T08:00:00Z',
            lastHeartbeatAt: '2026-06-07T08:09:00Z',
            binding: {
                state: 'BOUND',
                agentId: 'agent-3',
            },
            latestInstallTask: {
                id: 'task-3',
                status: 'RUNNING',
                message: undefined,
            },
        })

        expect(fetchMock).toHaveBeenCalledWith('/api/nm/v1/nodes/node-3', expect.any(Object))
    })

    it('fetchNodeStatusBatch maps backend array into explicit UI-safe record shape', async () => {
        fetchMock.mockResolvedValue(
            makeResponse({
                body: {
                    success: true,
                    data: [
                        {
                            node_id: 'node-1',
                            status: 'online',
                            binding_state: 'BOUND',
                            install_phase: 'COMPLETED',
                            updated_at: '2026-06-07T08:00:00Z',
                            last_heartbeat_at: '2026-06-07T08:00:00Z',
                        },
                        {
                            node_id: 'node-2',
                            status: 'offline',
                            binding_state: 'UNBOUND',
                        }
                    ],
                },
            }),
        )

        await expect(fetchNodeStatusBatch(['node-1', 'node-2'])).resolves.toEqual({
            'node-1': {
                id: 'node-1',
                onlineStatus: 'online',
                bindingState: 'BOUND',
                installPhase: 'COMPLETED',
                updatedAt: '2026-06-07T08:00:00Z',
                lastHeartbeatAt: '2026-06-07T08:00:00Z',
            },
            'node-2': {
                id: 'node-2',
                onlineStatus: 'offline',
                bindingState: 'UNBOUND',
                installPhase: undefined,
                updatedAt: undefined,
                lastHeartbeatAt: undefined,
            }
        })

        expect(fetchMock).toHaveBeenCalledWith(
            '/api/nm/v1/nodes/status:batch?node_ids=node-1%2Cnode-2',
            expect.any(Object),
        )
    })

    it('fetchNodeStatusBatch returns empty object immediately for empty input', async () => {
        const result = await fetchNodeStatusBatch([])
        expect(result).toEqual({})
        expect(fetchMock).not.toHaveBeenCalled()
    })

    it('fetchNodeBinding maps v1 binding views', async () => {
        fetchMock.mockResolvedValue(
            makeResponse({ body: { success: true, data: { node_id: 'node-1', agent_id: 'agent-1', binding_state: 'bound' } } }),
        )

        await expect(fetchNodeBinding('node-1')).resolves.toEqual({ state: 'BOUND', agentId: 'agent-1' })
    })

    it('fetchLatestInstallTask maps v1 install task views', async () => {
        fetchMock.mockResolvedValue(
            makeResponse({
                body: {
                    success: true,
                    data: {
                        install_task_id: 'task-9',
                        node_id: 'node-1',
                        task_state: 'failed',
                        error_message: 'ssh failed',
                        started_at: '2026-06-07T08:00:00Z',
                        finished_at: '2026-06-07T08:05:00Z',
                        retryable: true,
                    },
                },
            }),
        )

        await expect(fetchLatestInstallTask('node-1')).resolves.toEqual({
            id: 'task-9',
            status: 'FAILED',
            message: 'ssh failed',
        })
    })

    it('installNodeAgent maps v1 install receipts into the frontend task handle shape', async () => {
        fetchMock.mockResolvedValue(
            makeResponse({ body: { success: true, data: { install_task_id: 'task-1', node_id: 'node-1', accepted: true, task_state: 'pending' } } }),
        )

        await expect(installNodeAgent('node-1')).resolves.toEqual({ id: 'task-1', status: 'PENDING', message: undefined })

        expect(fetchMock).toHaveBeenCalledWith(
            '/api/nm/v1/nodes/node-1/install',
            expect.objectContaining({
                method: 'POST',
                body: JSON.stringify({}),
            }),
        )
    })

    it('installNodeAgent still supports legacy flat install task payloads', async () => {
        fetchMock.mockResolvedValue(
            makeResponse({ body: { success: true, data: { id: 'task-1', status: 'RUNNING' } } }),
        )

        await expect(installNodeAgent('node-1')).resolves.toEqual({ id: 'task-1', status: 'RUNNING' })

        expect(fetchMock).toHaveBeenCalledWith(
            '/api/nm/v1/nodes/node-1/install',
            expect.objectContaining({
                method: 'POST',
                body: JSON.stringify({}),
            }),
        )
    })

    it('fetchLatestInstallTask returns null on missing task response', async () => {
        fetchMock.mockResolvedValue(
            makeResponse({ ok: false, status: 404, body: { success: false, error: { code: 'NODE_NOT_FOUND', message: 'missing' } } }),
        )

        await expect(fetchLatestInstallTask('node-1')).resolves.toBeNull()
    })

    it('fetchLatestInstallTask throws on unexpected server failure instead of collapsing to null', async () => {
        fetchMock.mockResolvedValue(
            makeResponse({ ok: false, status: 500, body: { success: false, error: 'task failed' } })
        )

        await expect(fetchLatestInstallTask('node-1')).rejects.toThrow('task failed')
    })

    it('rebindNode posts the repair payload', async () => {
        fetchMock.mockResolvedValue(makeResponse({ body: { success: true, data: {} } }))

        await expect(rebindNode('node-1', { targetAgentId: 'agent-2', reason: 'repair' })).resolves.toBeUndefined()

        expect(fetchMock).toHaveBeenCalledWith(
            '/api/nm/v1/nodes/node-1/rebind',
            expect.objectContaining({
                method: 'POST',
                body: JSON.stringify({ target_agent_id: 'agent-2', reason: 'repair' }),
            }),
        )
    })

    it('syncAgent posts to the sync endpoint', async () => {
        fetchMock.mockResolvedValue(makeResponse({ body: { success: true, data: { accepted: true } } }))

        await expect(syncAgent({ agentId: 'agent-1' })).resolves.toEqual({ accepted: true })
    })

    it('surfaces structured API error messages', async () => {
        fetchMock.mockResolvedValue(
            makeResponse({
                ok: false,
                status: 400,
                body: {
                    success: false,
                    error: {
                        code: 'INVALID_ARGUMENT',
                        message: 'node endpoint is required',
                    },
                },
            }),
        )

        await expect(createNode({ name: 'bad', endpoint: '' })).rejects.toThrow('node endpoint is required')
    })

    describe('Contract Drift and Error Handling', () => {
        it('throws a TransportError if response is not valid JSON', async () => {
            fetchMock.mockResolvedValue(
                makeResponse({ ok: true, status: 200, body: undefined })
            )
            fetchMock.mockImplementationOnce(() => Promise.resolve({
                ok: true,
                status: 200,
                text: vi.fn().mockResolvedValue('<html><body>Bad Gateway</body></html>')
            }))

            await expect(fetchNodes()).rejects.toThrow(/JSON/)
        })

        it('throws a TransportError if response missing data field when success is true', async () => {
            fetchMock.mockResolvedValue(
                makeResponse({
                    ok: true,
                    status: 200,
                    body: { success: true },
                })
            )

            await expect(fetchNodeDetail('node-3')).rejects.toThrow('Response missing data')
        })

        it('fetchNodes tolerates missing items array in list payload', async () => {
            fetchMock.mockResolvedValue(
                makeResponse({
                    body: {
                        success: true,
                        data: {
                            total: 0
                        },
                    },
                }),
            )

            await expect(fetchNodes()).resolves.toEqual({
                nodes: [],
                total: 0,
            })
        })
        
        it('fetchNodes throws a TransportError if response missing data payload when success is true', async () => {
            fetchMock.mockResolvedValue(
                makeResponse({
                    body: {
                        success: true,
                        data: null
                    },
                }),
            )

            await expect(fetchNodes()).rejects.toThrow('Response missing data')
        })


        it('fetchNodeStatusBatch throws if response is malformed', async () => {
            fetchMock.mockResolvedValue(
                makeResponse({ ok: false, status: 500, body: { success: false, error: 'batch failed' } })
            )
            await expect(fetchNodeStatusBatch(['node-1'])).rejects.toThrow('batch failed')
        })

        it('fetchNodeStatusBatch throws a TransportError if response is missing data field when success is true', async () => {
            fetchMock.mockResolvedValue(
                makeResponse({ ok: true, status: 200, body: { success: true, data: null } })
            )
            await expect(fetchNodeStatusBatch(['node-1'])).rejects.toThrow('Response missing data')
        })
        
        it('falls back to string error if error object is a string', async () => {
            fetchMock.mockResolvedValue(
                makeResponse({
                    ok: false,
                    status: 500,
                    body: {
                        success: false,
                        error: 'Internal Server Error String',
                    },
                }),
            )

            await expect(createNode({ name: 'bad', endpoint: '' })).rejects.toThrow('Internal Server Error String')
        })

        it('falls back to code if message is missing in error object', async () => {
            fetchMock.mockResolvedValue(
                makeResponse({
                    ok: false,
                    status: 403,
                    body: {
                        success: false,
                        error: {
                            code: 'FORBIDDEN_ACTION'
                        },
                    },
                }),
            )

            await expect(createNode({ name: 'bad', endpoint: '' })).rejects.toThrow('FORBIDDEN_ACTION')
        })
    })
})
