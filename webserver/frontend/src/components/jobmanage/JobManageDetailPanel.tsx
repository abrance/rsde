import type { JobRecord } from '../../types/jobmanage'
import './JobManageDetailPanel.css'

interface JobManageDetailPanelProps {
    job: JobRecord | null
    onPrecheck: (jobId: string) => void
    onRetry: (jobId: string) => void
    onCancel: (jobId: string) => void
}

export default function JobManageDetailPanel({ job, onPrecheck, onRetry, onCancel }: JobManageDetailPanelProps) {
    if (!job) {
        return (
            <aside className="job-manage-detail card" aria-label="作业详情">
                <div className="job-manage-section-heading">
                    <h2>详情面板</h2>
                </div>
                <p>选择左侧作业查看详情、预检结果与最近一次执行摘要。</p>
            </aside>
        )
    }

    return (
        <aside className="job-manage-detail card" aria-label="作业详情">
            <div className="job-manage-section-heading">
                <h2>作业详情</h2>
                <div className="job-manage-detail-actions">
                    <button type="button" onClick={() => onPrecheck(job.id)} className="action-btn">执行预检</button>
                    {(job.status === 'failed' || job.status === 'cancelled') && (
                        <button type="button" onClick={() => onRetry(job.id)} className="action-btn">重试作业</button>
                    )}
                    {(job.status === 'pending' || job.status === 'running') && (
                        <button type="button" onClick={() => onCancel(job.id)} className="action-btn btn-danger">取消作业</button>
                    )}
                </div>
            </div>
            
            <div className="job-manage-detail-content">
                <div className="detail-group">
                    <h3>基本信息</h3>
                    <dl className="detail-list">
                        <dt>作业名称</dt>
                        <dd>{job.name}</dd>
                        <dt>目标节点</dt>
                        <dd>{job.nodeName}</dd>
                        <dt>当前状态</dt>
                        <dd><span className={`job-manage-status-pill is-${job.status}`}>{job.status}</span></dd>
                        <dt>负责人</dt>
                        <dd>{job.owner}</dd>
                        <dt>作业描述</dt>
                        <dd>{job.description}</dd>
                    </dl>
                </div>

                <div className="detail-group">
                    <h3>预检结果</h3>
                    {job.precheck ? (
                        <div className={`precheck-card is-${job.precheck.allowed ? 'allowed' : 'denied'}`}>
                            <p><strong>是否允许执行：</strong> {job.precheck.allowed ? '是' : '否'}</p>
                            <p><strong>原因：</strong> {job.precheck.reason}</p>
                            <p className="precheck-time">检查时间: {new Date(job.precheck.checkedAt).toLocaleString()}</p>
                        </div>
                    ) : (
                        <p className="detail-muted">暂无预检记录</p>
                    )}
                </div>

                <div className="detail-group">
                    <h3>最近执行摘要</h3>
                    <dl className="detail-list">
                        <dt>创建时间</dt>
                        <dd>{new Date(job.createdAt).toLocaleString()}</dd>
                        {job.startedAt && (
                            <>
                                <dt>开始时间</dt>
                                <dd>{new Date(job.startedAt).toLocaleString()}</dd>
                            </>
                        )}
                        {job.finishedAt && (
                            <>
                                <dt>结束时间</dt>
                                <dd>{new Date(job.finishedAt).toLocaleString()}</dd>
                            </>
                        )}
                        {job.lastError && (
                            <>
                                <dt>最后错误</dt>
                                <dd className="detail-error">{job.lastError}</dd>
                            </>
                        )}
                    </dl>
                </div>
            </div>
        </aside>
    )
}
