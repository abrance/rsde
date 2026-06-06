import type { NodeInstallTaskView } from '../../types/nodemanage';
import './NodeTaskTrackingCard.css';

interface NodeTaskTrackingCardProps {
    task: NodeInstallTaskView | null;
    isLoading: boolean;
    error: string | null;
}

export default function NodeTaskTrackingCard({ task, isLoading, error }: NodeTaskTrackingCardProps) {
    if (isLoading) {
        return (
            <div className="node-task-tracking-card">
                <h3>最新安装任务 (Latest Task)</h3>
                <div data-testid="task-loading" className="loading-state">Loading task...</div>
            </div>
        );
    }

    if (error) {
        return (
            <div className="node-task-tracking-card">
                <h3>最新安装任务 (Latest Task)</h3>
                <div className="error-message" data-testid="task-error">
                    Error loading task: {error}
                </div>
            </div>
        );
    }

    if (!task) {
        return (
            <div className="node-task-tracking-card">
                <h3>最新安装任务 (Latest Task)</h3>
                <div data-testid="task-empty" className="detail-muted">No latest task found for this node.</div>
            </div>
        );
    }

    const getStateClass = (state: string) => {
        const s = state.toLowerCase();
        if (s === 'completed') return 'completed';
        if (s === 'failed') return 'failed';
        if (s === 'running') return 'running';
        if (s === 'pending') return 'pending';
        return '';
    };

    return (
        <div className="node-task-tracking-card">
            <h3>最新安装任务 (Latest Task)</h3>
            
            <dl className="detail-list">
                <dt>Task ID</dt>
                <dd data-testid="task-tracking-id">{task.install_task_id}</dd>
                
                <dt>Node ID</dt>
                <dd data-testid="task-tracking-node">{task.node_id}</dd>
                
                <dt>State</dt>
                <dd>
                    <span data-testid="task-tracking-state" className={`task-state-badge ${getStateClass(task.task_state)}`}>
                        {task.task_state}
                    </span>
                </dd>
                
                {task.current_step && (
                    <>
                        <dt>Current Step</dt>
                        <dd data-testid="task-tracking-step">{task.current_step}</dd>
                    </>
                )}
                
                <dt>Started At</dt>
                <dd data-testid="task-tracking-started">{task.started_at}</dd>
                
                {task.finished_at && (
                    <>
                        <dt>Finished At</dt>
                        <dd data-testid="task-tracking-finished">{task.finished_at}</dd>
                    </>
                )}
            </dl>

            {(task.error_code || task.error_message) && (
                <div className="task-error-box">
                    {task.error_code && <strong data-testid="task-tracking-errcode">{task.error_code}</strong>}
                    {task.error_message && <span data-testid="task-tracking-errmsg">{task.error_message}</span>}
                </div>
            )}

            {task.request_summary && (
                <div className="task-request-summary">
                    <h4>Request Summary</h4>
                    <dl className="task-summary-grid">
                        {task.request_summary.host && (
                            <>
                                <dt>Host</dt>
                                <dd data-testid="task-summary-host">{task.request_summary.host}</dd>
                            </>
                        )}
                        {task.request_summary.ssh_port !== undefined && (
                            <>
                                <dt>SSH Port</dt>
                                <dd data-testid="task-summary-port">{task.request_summary.ssh_port}</dd>
                            </>
                        )}
                        {task.request_summary.username && (
                            <>
                                <dt>Username</dt>
                                <dd data-testid="task-summary-user">{task.request_summary.username}</dd>
                            </>
                        )}
                        {task.request_summary.rsagent_package_url && (
                            <>
                                <dt>Package URL</dt>
                                <dd data-testid="task-summary-pkg">{task.request_summary.rsagent_package_url}</dd>
                            </>
                        )}
                        {task.request_summary.install_root && (
                            <>
                                <dt>Install Root</dt>
                                <dd data-testid="task-summary-root">{task.request_summary.install_root}</dd>
                            </>
                        )}
                        {task.request_summary.labels && task.request_summary.labels.length > 0 && (
                            <>
                                <dt>Labels</dt>
                                <dd data-testid="task-summary-labels">{task.request_summary.labels.join(', ')}</dd>
                            </>
                        )}
                        {task.request_summary.plugin_names && task.request_summary.plugin_names.length > 0 && (
                            <>
                                <dt>Plugins</dt>
                                <dd data-testid="task-summary-plugins">{task.request_summary.plugin_names.join(', ')}</dd>
                            </>
                        )}
                    </dl>
                </div>
            )}
        </div>
    );
}
