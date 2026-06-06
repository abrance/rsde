import { useEffect, useState, useRef, useCallback } from 'react';
import { listNodes, getNode, getLatestInstallTask, getNodeBinding, getNodeStatusBatch } from '../lib/nodemanage';
import { NodeSummary, NodeDetail, NodeInstallTaskView, NodeBindingView, NodeInstallTaskReceipt } from '../types/nodemanage';
import NodeDetailPanel from '../components/nodemanage/NodeDetailPanel';
import NodeInstallForm from '../components/nodemanage/NodeInstallForm';
import { NodeRebindForm } from '../components/nodemanage/NodeRebindForm';
import './NodeManagePage.css';

export default function NodeManagePage() {
    const [nodes, setNodes] = useState<NodeSummary[]>([]);
    const [listError, setListError] = useState<string | null>(null);
    const [batchError, setBatchError] = useState<string | null>(null);
    const [filterText, setFilterText] = useState('');
    
    const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
    
    const [nodeDetail, setNodeDetail] = useState<NodeDetail | null>(null);
    const [detailError, setDetailError] = useState<string | null>(null);
    const [isDetailLoading, setIsDetailLoading] = useState(false);
    
    const [latestTask, setLatestTask] = useState<NodeInstallTaskView | null>(null);
    const [taskError, setTaskError] = useState<string | null>(null);
    const [isTaskLoading, setIsTaskLoading] = useState(false);

    const [nodeBinding, setNodeBinding] = useState<NodeBindingView | null>(null);
    const [bindingError, setBindingError] = useState<string | null>(null);
    const [isBindingLoading, setIsBindingLoading] = useState(false);

    const [installReceipt, setInstallReceipt] = useState<NodeInstallTaskReceipt | null>(null);

    const selectionRef = useRef<string | null>(null);
    const batchPollInFlightRef = useRef(false);
    const nodeIdsSignature = nodes.map((node) => node.node_id).join(',');

    const refreshNodeList = useCallback(() => {
        return listNodes()
            .then((data) => {
                setNodes(data.items);
                setListError(null);
            })
            .catch((err) => {
                setListError(err.message || 'Failed to load nodes');
            });
    }, []);

    useEffect(() => {
        void refreshNodeList();
    }, [refreshNodeList]);

    useEffect(() => {
        if (nodes.length === 0) return;
        
        const interval = setInterval(() => {
            if (batchPollInFlightRef.current) {
                return;
            }

            const nodeIds = nodes.map(n => n.node_id);
            batchPollInFlightRef.current = true;
            getNodeStatusBatch(nodeIds)
                .then(batchData => {
                    setBatchError(null);
                    setNodes(currentNodes => currentNodes.map(node => {
                        const update = batchData.find(b => b.node_id === node.node_id);
                        return update ? { ...node, ...update } : node;
                    }));
                })
                .catch(err => {
                    setBatchError(err.message || 'Batch status fetch failed');
                })
                .finally(() => {
                    batchPollInFlightRef.current = false;
                });
        }, 5000);
        
        return () => clearInterval(interval);
    }, [nodeIdsSignature]);

    const fetchNodeData = useCallback((nodeId: string) => {
        setDetailError(null);
        setIsDetailLoading(true);
        
        setTaskError(null);
        setIsTaskLoading(true);

        setBindingError(null);
        setIsBindingLoading(true);

        Promise.allSettled([
            getNode(nodeId),
            getLatestInstallTask(nodeId),
            getNodeBinding(nodeId)
        ]).then(([detailResult, taskResult, bindingResult]) => {
            if (selectionRef.current === nodeId) {
                setIsDetailLoading(false);
                if (detailResult.status === 'fulfilled') {
                    setNodeDetail(detailResult.value);
                } else {
                    setDetailError(detailResult.reason?.message || 'Failed to load node detail');
                }

                setIsTaskLoading(false);
                if (taskResult.status === 'fulfilled') {
                    setLatestTask(taskResult.value);
                } else {
                    setTaskError(taskResult.reason?.message || 'Failed to fetch latest task');
                }

                setIsBindingLoading(false);
                if (bindingResult.status === 'fulfilled') {
                    const fallbackBinding = detailResult.status === 'fulfilled'
                        ? detailResult.value.binding ?? null
                        : null;

                    setNodeBinding(bindingResult.value ?? fallbackBinding);
                } else {
                    const err = bindingResult.reason;
                    if (err && err.code === 'BINDING_NOT_FOUND') {
                        setNodeBinding(null);
                    } else {
                        setBindingError(err?.message || 'Failed to load binding');
                    }
                }
            }
        });
    }, []);

    const handleSelect = (nodeId: string) => {
        setSelectedNodeId(nodeId);
        selectionRef.current = nodeId;
        setInstallReceipt(null);
        
        setNodeDetail(null);
        setLatestTask(null);
        setNodeBinding(null);

        fetchNodeData(nodeId);
    };

    const handleInstallSuccess = (receipt: NodeInstallTaskReceipt) => {
        setInstallReceipt(receipt);
        if (selectedNodeId) {
            fetchNodeData(selectedNodeId);
        }
    };

    
    const handleRebindSuccess = () => {
        if (selectedNodeId) {
            fetchNodeData(selectedNodeId);
            void refreshNodeList();
        }
    };

    const filteredNodes = nodes.filter(node => 
        node.node_name.toLowerCase().includes(filterText.toLowerCase()) || 
        node.node_id.toLowerCase().includes(filterText.toLowerCase())
    );

    const totalNodes = nodes.length;
    const onlineNodes = nodes.filter(n => n.online_status === 'ONLINE').length;
    const boundNodes = nodes.filter(n => n.binding_state === 'BOUND').length;

    return (
        <div className="node-manage-page">
            <h1>NodeManage</h1>
            <p>节点管理的前端工作台 (v1 正在建设中)</p>
            
            {listError ? (
                <div className="error-message" data-testid="list-error">
                    Failed to load node list: {listError}
                </div>
            ) : null}
            
            {batchError ? (
                <div className="error-message batch-error" data-testid="batch-error">
                    Batch refresh failed: {batchError}
                </div>
            ) : null}
            
            <>
                <div className="summary-cards">
                        <div className="card">Total Nodes: {totalNodes}</div>
                        <div className="card">Online: {onlineNodes}</div>
                        <div className="card">Bound: {boundNodes}</div>
                    </div>

                    <div className="filter-toolbar">
                        <input 
                            type="text" 
                            placeholder="Filter nodes..." 
                            value={filterText}
                            onChange={(e) => setFilterText(e.target.value)}
                        />
                    </div>

                    <div className="node-list">
                        <table className="node-table">
                            <thead>
                                <tr>
                                    <th>Node Name</th>
                                    <th>Environment</th>
                                    <th>Status</th>
                                </tr>
                            </thead>
                            <tbody>
                                {nodes.length === 0 ? (
                                    <tr>
                                        <td colSpan={3}>No nodes found.</td>
                                    </tr>
                                ) : filteredNodes.length === 0 ? (
                                    <tr>
                                        <td colSpan={3} data-testid="empty-filtered-state">No nodes match the filter.</td>
                                    </tr>
                                ) : (
                                    filteredNodes.map(node => (
                                        <tr 
                                            key={node.node_id} 
                                            onClick={() => handleSelect(node.node_id)}
                                            className={selectedNodeId === node.node_id ? 'selected-row' : ''}
                                        >
                                            <td>{node.node_name}</td>
                                            <td>{node.environment}</td>
                                            <td>{node.online_status}</td>
                                        </tr>
                                    ))
                                )}
                            </tbody>
                        </table>
                    </div>
                </>

            <NodeDetailPanel 
                nodeId={selectedNodeId}
                detail={nodeDetail}
                isDetailLoading={isDetailLoading}
                detailError={detailError}
                latestTask={latestTask}
                isTaskLoading={isTaskLoading}
                taskError={taskError}
                binding={nodeBinding}
                isBindingLoading={isBindingLoading}
                bindingError={bindingError}
            />

            {selectedNodeId && (
                <div className="node-manage-actions">
                    {installReceipt && (
                        <div className="install-receipt-banner card">
                            <strong>Install request accepted.</strong> Task ID: {installReceipt.install_task_id}
                        </div>
                    )}
                    <NodeInstallForm key={`install-${selectedNodeId}`} nodeId={selectedNodeId} onSuccess={handleInstallSuccess} />
                    <NodeRebindForm key={`rebind-${selectedNodeId}`} nodeId={selectedNodeId} onSuccess={handleRebindSuccess} />
                </div>
            )}
        </div>
    )
}
