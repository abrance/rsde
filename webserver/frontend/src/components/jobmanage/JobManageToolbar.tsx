import type { JobFilter } from '../../types/jobmanage'
import './JobManageToolbar.css'

interface JobManageToolbarProps {
    currentFilter: JobFilter
    onFilterChange: (filter: JobFilter) => void
    onRefresh: () => void
}

export default function JobManageToolbar({ currentFilter, onFilterChange, onRefresh }: JobManageToolbarProps) {
    const filters: { value: JobFilter; label: string }[] = [
        { value: 'all', label: '全部' },
        { value: 'pending', label: '待执行' },
        { value: 'running', label: '运行中' },
        { value: 'success', label: '成功' },
        { value: 'failed', label: '失败' },
        { value: 'cancelled', label: '已取消' },
    ]

    return (
        <div className="job-manage-toolbar">
            <div className="job-manage-filters">
                {filters.map(({ value, label }) => (
                    <button
                        key={value}
                        className={`job-manage-filter-btn ${currentFilter === value ? 'is-active' : ''}`}
                        onClick={() => onFilterChange(value)}
                        type="button"
                    >
                        {label}
                    </button>
                ))}
            </div>
            <button className="job-manage-refresh-btn" onClick={onRefresh} type="button">
                刷新数据
            </button>
        </div>
    )
}
