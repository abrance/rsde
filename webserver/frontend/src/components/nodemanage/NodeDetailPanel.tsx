import type { NodeDetail, NodeInstallTaskView, NodeBindingView } from '../../types/nodemanage';
import NodeTaskTrackingCard from './NodeTaskTrackingCard';
import './NodeDetailPanel.css';

interface NodeDetailPanelProps {
    nodeId: string | null;
    detail: NodeDetail | null;
    isDetailLoading: boolean;
    detailError: string | null;
    latestTask: NodeInstallTaskView | null;
    isTaskLoading: boolean;
    taskError: string | null;
    binding: NodeBindingView | null;
    isBindingLoading: boolean;
    bindingError: string | null;
}

export default function NodeDetailPanel({
    nodeId,
    detail,
    isDetailLoading,
    detailError,
    latestTask,
    isTaskLoading,
    taskError,
    binding,
    isBindingLoading,
    bindingError,
}: NodeDetailPanelProps) {
    if (!nodeId) {
        return (
            <aside className="node-detail-panel card" aria-label="Node Detail">
                <div className="node-detail-section-heading">
                    <h2>Node Detail</h2>
                </div>
                <p data-testid="detail-placeholder">Please select a node to view details.</p>
            </aside>
        );
    }

    const effectiveBinding = binding || detail?.binding;

    return (
        <aside className="node-detail-panel card" aria-label="Node Detail">
            <div className="node-detail-section-heading">
                <h2>Node Detail</h2>
                <div className="node-detail-actions">
                    <span data-testid="selected-node-id" className="node-detail-id-pill">
                        {nodeId}
                    </span>
                </div>
            </div>

            <div className="node-detail-content">
                {isDetailLoading ? (
                    <div data-testid="detail-loading" className="loading-state">Loading detail...</div>
                ) : detailError ? (
                    <div className="error-message" data-testid="detail-error">
                        Error loading detail: {detailError}
                    </div>
                ) : detail ? (
                    <div data-testid="detail-loaded">
                        <div className="detail-group">
                            <h3>基本信息 (Node)</h3>
                            <dl className="detail-list">
                                <dt>Node Name</dt>
                                <dd data-testid="detail-node-name">{detail.node.node_name}</dd>
                                <dt>Environment</dt>
                                <dd data-testid="detail-node-env">{detail.node.environment}</dd>
                                <dt>Labels</dt>
                                <dd data-testid="detail-node-labels">{detail.node.labels.join(', ')}</dd>
                                <dt>Updated At</dt>
                                <dd data-testid="detail-node-updated">{detail.node.updated_at}</dd>
                            </dl>
                        </div>

                        <div className="detail-group">
                            <h3>状态信息 (Status)</h3>
                            <dl className="detail-list">
                                <dt>Lifecycle</dt>
                                <dd data-testid="detail-status-lifecycle">{detail.status.lifecycle_state}</dd>
                                <dt>Online Status</dt>
                                <dd data-testid="detail-status-online">{detail.status.online_status}</dd>
                                <dt>Binding State</dt>
                                <dd data-testid="detail-status-binding">{detail.status.binding_state}</dd>
                                <dt>Install Phase</dt>
                                <dd>{detail.status.install_phase}</dd>
                                <dt>Last Heartbeat</dt>
                                <dd>{detail.status.last_heartbeat_at || 'Never'}</dd>
                            </dl>
                        </div>

                        <div className="detail-group">
                            <h3>绑定状态 (Binding)</h3>
                            {isBindingLoading ? (
                                <div data-testid="binding-loading" className="loading-state">Loading binding...</div>
                            ) : bindingError ? (
                                <div className="error-message" data-testid="binding-error">
                                    Error loading binding: {bindingError}
                                </div>
                            ) : effectiveBinding ? (
                                <dl className="detail-list">
                                    <dt>Agent ID</dt>
                                    <dd data-testid="detail-binding-agent">{effectiveBinding.agent_id}</dd>
                                    <dt>State</dt>
                                    <dd>{effectiveBinding.binding_state}</dd>
                                </dl>
                            ) : (
                                <p data-testid="detail-binding-empty" className="detail-muted">暂无绑定</p>
                            )}
                        </div>

                        <div className="detail-group">
                            <h3>心跳引用 (Heartbeat Ref)</h3>
                            {detail.heartbeat_ref ? (
                                <dl className="detail-list">
                                    <dt>DataLink ID</dt>
                                    <dd data-testid="detail-heartbeat-link">{detail.heartbeat_ref.data_link_id}</dd>
                                </dl>
                            ) : (
                                <p className="detail-muted">No heartbeat ref.</p>
                            )}
                        </div>
                    </div>
                ) : null}

                <NodeTaskTrackingCard
                    task={latestTask}
                    isLoading={isTaskLoading}
                    error={taskError}
                />
            </div>
        </aside>
    );
}
