import { useEffect, useState, useRef, useMemo } from 'react'
import type { NodeRecord, CreateNodePayload } from '../types/nodemanage'
import { fetchNodes, createNode, fetchNodeStatusBatch } from '../data/nodemanage'
import { nodeManageMeta } from '../data/nodeManageMetaData'
import NodeDetailPanel from '../components/nodemanage/NodeDetailPanel'
import NodeManageTable from '../components/nodemanage/NodeManageTable'
import './NodeManagePage.css'

function CreateNodeModal({ 
    isOpen, 
    onClose, 
    onSubmit 
}: { 
    isOpen: boolean; 
    onClose: () => void; 
    onSubmit: (payload: CreateNodePayload) => Promise<void>;
}) {
    const [newNodeName, setNewNodeName] = useState('')
    const [endpoint, setEndpoint] = useState('')
    const [showAdvanced, setShowAdvanced] = useState(false)
    const [environment, setEnvironment] = useState('')
    const [sshPort, setSshPort] = useState('')
    const [sshUsername, setSshUsername] = useState('')
    const [sshPassword, setSshPassword] = useState('')
    const [sshPrivateKey, setSshPrivateKey] = useState('')
    const [createError, setCreateError] = useState<string | null>(null)
    const [isSubmitting, setIsSubmitting] = useState(false)

    useEffect(() => {
        if (!isOpen) {
            setNewNodeName('')
            setEndpoint('')
            setShowAdvanced(false)
            setEnvironment('')
            setSshPort('')
            setSshUsername('')
            setSshPassword('')
            setSshPrivateKey('')
            setCreateError(null)
            setIsSubmitting(false)
        }
    }, [isOpen])

    if (!isOpen) return null

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault()
        if (isSubmitting) return

        if (!newNodeName.trim()) {
            setCreateError('节点名称不能为空')
            return
        }

        if (!endpoint.trim()) {
            setCreateError('节点地址不能为空')
            return
        }

        setIsSubmitting(true)
        setCreateError(null)
        
        try {
            const payload: CreateNodePayload = {
                name: newNodeName.trim(),
                endpoint: endpoint.trim(),
            }
            if (environment.trim()) payload.environment = environment.trim()
            if (sshPort.trim()) payload.ssh_port = parseInt(sshPort.trim(), 10)
            if (sshUsername.trim()) payload.ssh_username = sshUsername.trim()
            if (sshPassword) payload.ssh_password = sshPassword
            if (sshPrivateKey.trim()) payload.ssh_private_key = sshPrivateKey.trim()

            await onSubmit(payload)
        } catch (err) {
            setCreateError(err instanceof Error ? err.message : '创建失败')
            setIsSubmitting(false)
        }
    }

    return (
        <div className="modal-overlay">
            <dialog open className="modal-dialog" aria-label="纳管节点">
                <h2>纳管节点</h2>
                <form onSubmit={handleSubmit}>
                    <div className="form-group">
                        <label htmlFor="nodeName">节点名称</label>
                        <input
                            id="nodeName"
                            type="text"
                            value={newNodeName}
                            onChange={e => {
                                setNewNodeName(e.target.value)
                                setCreateError(null)
                            }}
                            disabled={isSubmitting}
                            autoFocus
                        />
                    </div>
                    <div className="form-group">
                        <label htmlFor="nodeEndpoint">节点地址</label>
                        <input
                            id="nodeEndpoint"
                            type="text"
                            value={endpoint}
                            onChange={e => {
                                setEndpoint(e.target.value)
                                setCreateError(null)
                            }}
                            placeholder="http://worker-1:8080"
                            disabled={isSubmitting}
                        />
                    </div>

                    <button
                        type="button"
                        className="btn btn-text advanced-toggle"
                        onClick={() => setShowAdvanced(!showAdvanced)}
                    >
                        {showAdvanced ? '▾ 收起 SSH 连接信息' : '▸ SSH 连接信息（可选）'}
                    </button>

                    {showAdvanced && (
                        <div className="advanced-fields">
                            <div className="form-group">
                                <label htmlFor="nodeEnv">环境</label>
                                <input
                                    id="nodeEnv"
                                    type="text"
                                    value={environment}
                                    onChange={e => setEnvironment(e.target.value)}
                                    placeholder="production / staging"
                                    disabled={isSubmitting}
                                />
                            </div>
                            <div className="form-row">
                                <div className="form-group form-group-half">
                                    <label htmlFor="sshPort">SSH 端口</label>
                                    <input
                                        id="sshPort"
                                        type="number"
                                        value={sshPort}
                                        onChange={e => setSshPort(e.target.value)}
                                        placeholder="22"
                                        disabled={isSubmitting}
                                    />
                                </div>
                                <div className="form-group form-group-half">
                                    <label htmlFor="sshUsername">SSH 用户名</label>
                                    <input
                                        id="sshUsername"
                                        type="text"
                                        value={sshUsername}
                                        onChange={e => setSshUsername(e.target.value)}
                                        placeholder="root"
                                        disabled={isSubmitting}
                                    />
                                </div>
                            </div>
                            <div className="form-group">
                                <label htmlFor="sshPassword">SSH 密码</label>
                                <input
                                    id="sshPassword"
                                    type="password"
                                    value={sshPassword}
                                    onChange={e => setSshPassword(e.target.value)}
                                    disabled={isSubmitting}
                                />
                            </div>
                            <div className="form-group">
                                <label htmlFor="sshPrivateKey">SSH 私钥</label>
                                <textarea
                                    id="sshPrivateKey"
                                    value={sshPrivateKey}
                                    onChange={e => setSshPrivateKey(e.target.value)}
                                    placeholder="-----BEGIN OPENSSH PRIVATE KEY-----"
                                    rows={3}
                                    disabled={isSubmitting}
                                />
                            </div>
                        </div>
                    )}

                    {createError && <p className="error-text">{createError}</p>}
                    <div className="modal-actions">
                        <button 
                            type="button" 
                            className="btn btn-secondary" 
                            onClick={onClose}
                            disabled={isSubmitting}
                        >
                            取消
                        </button>
                        <button 
                            type="submit" 
                            className="btn btn-primary"
                            disabled={isSubmitting}
                        >
                            {isSubmitting ? '提交中...' : '确认'}
                        </button>
                    </div>
                </form>
            </dialog>
        </div>
    )
}

export default function NodeManagePage() {
    const [nodes, setNodes] = useState<NodeRecord[] | null>(null)
    const [loading, setLoading] = useState(true)
    const [error, setError] = useState<Error | null>(null)
    const [warning, setWarning] = useState<{ type: 'load' | 'poll'; message: string } | null>(null)
    
    const [isCreateModalOpen, setIsCreateModalOpen] = useState(false)
    const [selectedNode, setSelectedNode] = useState<NodeRecord | null>(null)
    
    const isMounted = useRef(true)
    const requestCounter = useRef(0)

    const visibleNodes = useMemo(() => {
        const MAX_VISIBLE_ROWS = 50
        return nodes ? nodes.slice(0, MAX_VISIBLE_ROWS) : []
    }, [nodes])

    const loadNodes = (isRefresh = false) => {
        if (!isRefresh) {
            setLoading(true)
        }
        setError(null)
        setWarning(null)
        
        const currentReq = ++requestCounter.current

        fetchNodes()
            .then(res => {
                if (isMounted.current && currentReq === requestCounter.current) {
                    setNodes(res.nodes)
                    setLoading(false)
                }
            })
            .catch(err => {
                if (isMounted.current && currentReq === requestCounter.current) {
                    if (!isRefresh) {
                        setError(err instanceof Error ? err : new Error('Unknown error'))
                        setLoading(false)
                    } else {
                        setWarning({ type: 'load', message: `节点列表刷新失败: ${err instanceof Error ? err.message : 'Unknown error'}` })
                    }
                }
            })
    }

    useEffect(() => {
        isMounted.current = true
        loadNodes()
            
        return () => {
            isMounted.current = false
        }
    }, [])

    const visibleNodeIdsRef = useRef<string[]>([])
    useEffect(() => {
        visibleNodeIdsRef.current = visibleNodes.map(n => n.id)
    }, [visibleNodes])

    const isPollingRef = useRef(false)

    useEffect(() => {
        if (selectedNode) {
            return
        }

        const intervalId = setInterval(() => {
            if (!isMounted.current || isPollingRef.current) return
            
            const nodeIds = visibleNodeIdsRef.current
            if (nodeIds.length === 0) return
            
            isPollingRef.current = true

            fetchNodeStatusBatch(nodeIds)
                .then(batch => {
                    if (!isMounted.current) return
                    
                    setNodes(prev => {
                        if (!prev) return prev
                        return prev.map(node => {
                            const update = batch[node.id]
                            if (update) {
                                return {
                                    ...node,
                                    onlineStatus: update.onlineStatus ?? node.onlineStatus,
                                    bindingState: update.bindingState ?? node.bindingState,
                                    updatedAt: update.updatedAt ?? node.updatedAt,
                                }
                            }
                            return node
                        })
                    })
                    setWarning(prevWarning => prevWarning?.type === 'poll' ? null : prevWarning)
                })
                .catch(err => {
                    if (isMounted.current) {
                        setWarning({ type: 'poll', message: `节点列表状态刷新失败: ${err instanceof Error ? err.message : 'Unknown error'}` })
                    }
                })
                .finally(() => {
                    isPollingRef.current = false
                })
        }, 10000)

        return () => clearInterval(intervalId)
    }, [selectedNode])

    const handleCreateSubmit = async (payload: CreateNodePayload) => {
        const result = await createNode(payload)
        
        setIsCreateModalOpen(false)
        setSelectedNode({ ...result })
        
        loadNodes(true)
    }

    if (selectedNode) {
        return (
            <section className="node-manage-page detail-view">
                <header className="node-manage-hero card">
                    <button className="btn btn-secondary back-btn" onClick={() => setSelectedNode(null)}>
                        ← 返回列表
                    </button>
                    <h1>{selectedNode.name}</h1>
                </header>
                <div className="node-manage-content">
                    <NodeDetailPanel 
                        nodeId={selectedNode.id} 
                        onBack={() => setSelectedNode(null)} 
                    />
                </div>
            </section>
        )
    }

    return (
        <section className="node-manage-page">
            <header className="node-manage-hero card">
                <div className="hero-content">
                    <div>
                        <p className="node-manage-eyebrow">Infrastructure nodes</p>
                        <h1>{nodeManageMeta.title}</h1>
                        <p className="node-manage-description">
                            {nodeManageMeta.description}
                        </p>
                    </div>
                    {nodes && nodes.length > 0 && (
                        <button 
                            className="btn btn-primary"
                            onClick={() => setIsCreateModalOpen(true)}
                        >
                            纳管节点
                        </button>
                    )}
                </div>
            </header>

            <div className="node-manage-content">
                {warning && (
                    <div className="warning-toast">
                        {warning.message}
                        <button className="btn btn-secondary btn-sm" onClick={() => loadNodes(true)} style={{ marginLeft: '1rem' }}>
                            重试同步
                        </button>
                    </div>
                )}
                {loading && (
                    <div className="loading-state card">
                        <p>加载中...</p>
                    </div>
                )}
                
                {!loading && error && (
                    <div className="error-state card">
                        <h2>加载失败</h2>
                        <p>{error.message}</p>
                        <button className="btn btn-primary" onClick={() => loadNodes(false)} style={{ marginTop: '1rem' }}>
                            重试
                        </button>
                    </div>
                )}

                {!loading && !error && nodes && nodes.length === 0 && (
                    <div className="empty-state card">
                        <div className="empty-icon">{nodeManageMeta.icon}</div>
                        <h2>暂无纳管节点</h2>
                        <div className="empty-description">
                            <p>请先在平台纳管（创建）节点记录，然后再进行 Agent 的安装。</p>
                            <p>纳管节点 → 安装 agent → 等待/确认 binding → 节点可用于作业平台</p>
                        </div>
                        <button 
                            className="btn btn-primary cta-btn"
                            onClick={() => setIsCreateModalOpen(true)}
                        >
                            纳管节点
                        </button>
                    </div>
                )}

                {!loading && !error && nodes && nodes.length > 0 && (
                    <div className="nodes-list card">
                        <div className="nodes-list-header">
                            <h2>节点列表</h2>
                            <span className="nodes-count">共 {nodes.length} 个节点</span>
                        </div>
                        <NodeManageTable nodes={visibleNodes} onSelectNode={setSelectedNode} />
                    </div>
                )}
            </div>

            <CreateNodeModal 
                isOpen={isCreateModalOpen} 
                onClose={() => setIsCreateModalOpen(false)} 
                onSubmit={handleCreateSubmit} 
            />
        </section>
    )
}
