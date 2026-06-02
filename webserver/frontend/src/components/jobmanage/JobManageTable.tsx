import type { JobRecord } from '../../types/jobmanage'
import './JobManageTable.css'

interface JobManageTableProps {
    jobs: JobRecord[]
    selectedJobId: string | null
    onSelectJob: (jobId: string) => void
}

export default function JobManageTable({ jobs, selectedJobId, onSelectJob }: JobManageTableProps) {
    if (jobs.length === 0) {
        return (
            <article className="job-manage-list card" aria-label="作业列表">
                <div className="job-manage-section-heading">
                    <h2>当前作业</h2>
                </div>
                <div className="job-manage-empty">
                    <p>暂无符合条件的作业记录。</p>
                </div>
            </article>
        )
    }

    return (
        <article className="job-manage-list card" aria-label="作业列表">
            <div className="job-manage-section-heading">
                <h2>当前作业</h2>
                <p>先用 mock 数据把页面骨架与状态流转打通。</p>
            </div>
            <ul className="job-manage-job-list">
                {jobs.map((job) => (
                    <li
                        key={job.id}
                        className={`job-manage-job-item ${selectedJobId === job.id ? 'is-selected' : ''}`}
                        onClick={() => onSelectJob(job.id)}
                        onKeyDown={(e) => {
                            if (e.key === 'Enter' || e.key === ' ') {
                                e.preventDefault()
                                onSelectJob(job.id)
                            }
                        }}
                        tabIndex={0}
                        role="button"
                    >
                        <div className="job-manage-job-info">
                            <strong>{job.name}</strong>
                            <p>{job.nodeName}</p>
                        </div>
                        <span className={`job-manage-status-pill is-${job.status}`}>{job.status}</span>
                    </li>
                ))}
            </ul>
        </article>
    )
}
