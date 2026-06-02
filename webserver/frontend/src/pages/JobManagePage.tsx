import { useMemo, useState } from 'react'
import { buildInitialJobRecords } from '../data/jobManageMockData'
import type { JobFilter, JobStatusCounts, JobActionMessage } from '../types/jobmanage'
import './JobManagePage.css'
import JobManageToolbar from '../components/jobmanage/JobManageToolbar'
import JobManageStatusSummary from '../components/jobmanage/JobManageStatusSummary'
import JobManageTable from '../components/jobmanage/JobManageTable'
import JobManageDetailPanel from '../components/jobmanage/JobManageDetailPanel'

const emptyStatusCounts: JobStatusCounts = {
    pending: 0,
    running: 0,
    success: 0,
    failed: 0,
    cancelled: 0,
}

export default function JobManagePage() {
    const [jobs, setJobs] = useState(() => buildInitialJobRecords())
    const [filter, setFilter] = useState<JobFilter>('all')
    const [selectedJobId, setSelectedJobId] = useState<string | null>(null)
    const [message, setMessage] = useState<JobActionMessage | null>(null)

    const statusCounts = useMemo(() => {
        return jobs.reduce<JobStatusCounts>((counts, job) => {
            counts[job.status] += 1
            return counts
        }, { ...emptyStatusCounts })
    }, [jobs])

    const filteredJobs = useMemo(() => {
        if (filter === 'all') return jobs
        return jobs.filter((j) => j.status === filter)
    }, [jobs, filter])

    const selectedJob = useMemo(() => {
        if (!selectedJobId) return null
        return jobs.find((j) => j.id === selectedJobId) || null
    }, [jobs, selectedJobId])

    const handleRefresh = () => {
        setJobs(buildInitialJobRecords())
        setFilter('all')
        setSelectedJobId(null)
        setMessage({ kind: 'info', text: '数据已刷新' })
        setTimeout(() => setMessage(null), 3000)
    }

    const handlePrecheck = (jobId: string) => {
        setJobs(prev => prev.map(job => {
            if (job.id === jobId) {
                return {
                    ...job,
                    precheck: {
                        allowed: true,
                        reason: '预检已通过，可以执行',
                        checkedAt: new Date().toISOString()
                    }
                }
            }
            return job
        }))
        setMessage({ kind: 'success', text: '执行预检成功' })
        setTimeout(() => setMessage(null), 3000)
    }

    const handleRetry = (jobId: string) => {
        setJobs(prev => prev.map(job => {
            if (job.id === jobId) {
                return {
                    ...job,
                    status: 'pending',
                    lastError: null
                }
            }
            return job
        }))
        setMessage({ kind: 'success', text: '已提交重试' })
        setTimeout(() => setMessage(null), 3000)
    }

    const handleCancel = (jobId: string) => {
        setJobs(prev => prev.map(job => {
            if (job.id === jobId) {
                return {
                    ...job,
                    status: 'cancelled'
                }
            }
            return job
        }))
        setMessage({ kind: 'success', text: '已取消该作业' })
        setTimeout(() => setMessage(null), 3000)
    }

    return (
        <section className="job-manage-page">
            <header className="job-manage-hero card">
                <p className="job-manage-eyebrow">Node operations control plane</p>
                <h1>JobManage</h1>
                <p className="job-manage-description">
                    面向 nodemanage 节点的前端作业管理工作台，后续会在这里补齐筛选、预检、重试和执行详情。
                </p>
                <p className="job-manage-caption">
                    用 mock 数据先打通作业筛选、节点预检、失败重试和执行状态回看这条前端闭环。
                </p>
                {message && (
                    <div className={`job-manage-message is-${message.kind}`}>
                        {message.text}
                    </div>
                )}
            </header>

            <JobManageStatusSummary counts={statusCounts} total={jobs.length} />
            <JobManageToolbar currentFilter={filter} onFilterChange={setFilter} onRefresh={handleRefresh} />

            <section className="job-manage-layout">
                <JobManageTable jobs={filteredJobs} selectedJobId={selectedJobId} onSelectJob={setSelectedJobId} />
                <JobManageDetailPanel 
                    job={selectedJob} 
                    onPrecheck={handlePrecheck}
                    onRetry={handleRetry}
                    onCancel={handleCancel}
                />
            </section>
        </section>
    )
}
