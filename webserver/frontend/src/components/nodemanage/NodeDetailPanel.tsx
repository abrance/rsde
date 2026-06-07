import { useEffect, useState, useCallback } from 'react';
import type { NodeDetail, NodeBinding, InstallTask } from '../../types/nodemanage';
import { fetchNodeDetail, fetchNodeBinding, fetchLatestInstallTask } from '../../data/nodemanage';
import NodeInstallForm from './NodeInstallForm';
import NodeRebindForm from './NodeRebindForm';
import './NodeDetailPanel.css';

export default function NodeDetailPanel({ 
    nodeId,
    onBack
}: { 
    nodeId: string;
    onBack: () => void;
}) {
    const [detail, setDetail] = useState<NodeDetail | null>(null);
    const [binding, setBinding] = useState<NodeBinding | null>(null);
    const [latestTask, setLatestTask] = useState<InstallTask | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<Error | null>(null);
    const [showInstallForm, setShowInstallForm] = useState(false);
    const [showRebindForm, setShowRebindForm] = useState(false);

    const loadData = useCallback(async (abortSignal?: AbortSignal) => {
        setLoading(true);
        setError(null);
        try {
            const [nodeData, bindingData, taskData] = await Promise.all([
                fetchNodeDetail(nodeId),
                fetchNodeBinding(nodeId),
                fetchLatestInstallTask(nodeId)
            ]);
            
            if (abortSignal?.aborted) return;
            
            setDetail(nodeData);
            setBinding(bindingData);
            setLatestTask(taskData);
        } catch (err) {
            if (abortSignal?.aborted) return;
            setError(err instanceof Error ? err : new Error('Failed to load node detail'));
        } finally {
            if (!abortSignal?.aborted) {
                setLoading(false);
            }
        }
    }, [nodeId]);

    useEffect(() => {
        const controller = new AbortController();
        loadData(controller.signal);
        return () => controller.abort();
    }, [loadData]);

    const handleInstallSuccess = () => {
        setShowInstallForm(false);
        loadData();
    };

    const handleRebindSuccess = () => {
        setShowRebindForm(false);
        loadData();
    };

    if (loading) {
        return (
            <div className="node-detail card">
                <p>加载详情中...</p>
            </div>
        );
    }

    if (error || !detail) {
        return (
            <div className="node-detail card error-state">
                <h2>加载失败</h2>
                <p>{error?.message}</p>
                <div style={{ display: 'flex', gap: '1rem', marginTop: '1rem' }}>
                    <button className="btn btn-secondary" onClick={onBack}>返回列表</button>
                    <button className="btn btn-primary" onClick={() => loadData()}>重试</button>
                </div>
            </div>
        );
    }

    const bindingState = binding?.state;
    const stateStr = typeof bindingState === 'string' ? bindingState.toUpperCase() : 'UNBOUND';
    const detailEndpoint = detail.endpoint;
    const latestTaskStatus = latestTask?.status;
    const latestTaskMessage = latestTask?.message;
    const bindingStatusText = stateStr === 'BOUND' ? '已绑定' : 
                              stateStr === 'BINDING' ? '绑定中' : 
                              stateStr === 'ERROR' ? '异常' : '未绑定';

    return (
        <div className="node-detail card">
            <header className="detail-header">
                <h2>节点详情</h2>
                <div className="detail-status">
                    <span className={`status-badge status-${stateStr.toLowerCase()}`}>
                        {bindingStatusText}
                    </span>
                </div>
            </header>
            
            <div className="detail-info">
                <div className="info-row">
                    <span className="info-label">名称:</span>
                    <span className="info-value">{detail.name}</span>
                </div>
                <div className="info-row">
                    <span className="info-label">ID:</span>
                    <span className="info-value">{detail.id}</span>
                </div>
                {typeof detailEndpoint === 'string' && detailEndpoint && (
                    <div className="info-row">
                        <span className="info-label">Endpoint:</span>
                        <span className="info-value">{detailEndpoint}</span>
                    </div>
                )}
            </div>

            {latestTask && (
                <div className="task-summary">
                    <h3>最新安装任务</h3>
                    <p>状态: {typeof latestTaskStatus === 'string' ? latestTaskStatus : 'UNKNOWN'}</p>
                    {typeof latestTaskMessage === 'string' && latestTaskMessage && <p>信息: {latestTaskMessage}</p>}
                </div>
            )}

            <div className="next-action">
                <p>下一步操作：</p>
                <button 
                    className="btn btn-primary"
                    onClick={() => setShowInstallForm(true)}
                >
                    安装 agent
                </button>
            </div>

            <div className="secondary-actions">
                <button 
                    className="btn btn-text"
                    onClick={() => setShowRebindForm(!showRebindForm)}
                >
                    需要重新绑定？
                </button>
            </div>

            {showInstallForm && (
                <NodeInstallForm 
                    nodeId={nodeId} 
                    onSuccess={handleInstallSuccess} 
                    onCancel={() => setShowInstallForm(false)} 
                />
            )}

            {showRebindForm && (
                <NodeRebindForm 
                    nodeId={nodeId} 
                    onSuccess={handleRebindSuccess} 
                    onCancel={() => setShowRebindForm(false)} 
                />
            )}
        </div>
    );
}
