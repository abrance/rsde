import type { JobStatusCounts } from '../../types/jobmanage'
import './JobManageStatusSummary.css'

interface JobManageStatusSummaryProps {
    counts: JobStatusCounts
    total: number
}

export default function JobManageStatusSummary({ counts, total }: JobManageStatusSummaryProps) {
    return (
        <section className="job-manage-summary" aria-label="作业状态摘要">
            <article className="job-manage-summary-card card">
                <span className="job-manage-summary-label">作业总数</span>
                <strong className="job-manage-summary-value">{total}</strong>
            </article>
            <article className="job-manage-summary-card card">
                <span className="job-manage-summary-label">运行中</span>
                <strong className="job-manage-summary-value is-running">{counts.running}</strong>
            </article>
            <article className="job-manage-summary-card card">
                <span className="job-manage-summary-label">失败待处理</span>
                <strong className="job-manage-summary-value is-failed">{counts.failed}</strong>
            </article>
        </section>
    )
}
