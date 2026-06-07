import type { NodeRecord } from '../../types/nodemanage'

interface NodeManageTableProps {
    nodes: NodeRecord[];
    onSelectNode: (node: NodeRecord) => void;
}

export function formatDateTime(dateStr?: string) {
    if (!dateStr) return '-'
    const d = new Date(dateStr)
    if (isNaN(d.getTime())) return '-'
    const pad = (n: number) => n.toString().padStart(2, '0')
    return `${d.getUTCFullYear()}-${pad(d.getUTCMonth() + 1)}-${pad(d.getUTCDate())} ${pad(d.getUTCHours())}:${pad(d.getUTCMinutes())}:${pad(d.getUTCSeconds())} UTC`
}

function getOnlineStatusText(status?: string) {
    switch (status) {
        case 'online': return '在线'
        case 'offline': return '离线'
        case 'unknown': return '未知'
        default: return '-'
    }
}

function getBindingStateText(state?: string) {
    switch (state) {
        case 'BOUND': return '已绑定'
        case 'UNBOUND': return '未绑定'
        case 'BINDING': return '绑定中'
        case 'ERROR': return '异常'
        case 'UNKNOWN': return '未知'
        default: return '-'
    }
}

export default function NodeManageTable({ nodes, onSelectNode }: NodeManageTableProps) {
    return (
        <table className="node-manage-table">
            <thead>
                <tr>
                    <th>节点名称</th>
                    <th>节点地址</th>
                    <th>在线状态</th>
                    <th>绑定状态</th>
                    <th>最近更新时间</th>
                    <th className="action-column">操作</th>
                </tr>
            </thead>
            <tbody>
                {nodes.map(node => (
                    <tr key={node.id} onClick={() => onSelectNode(node)} className="clickable-row">
                        <td>{node.name}</td>
                        <td>{node.endpoint || '-'}</td>
                        <td>
                            <span className={`status-badge ${node.onlineStatus || 'unknown'}`}>
                                {getOnlineStatusText(node.onlineStatus)}
                            </span>
                        </td>
                        <td>
                            <span className={`status-badge binding-${node.bindingState?.toLowerCase() || 'unknown'}`}>
                                {getBindingStateText(node.bindingState)}
                            </span>
                        </td>
                        <td>{formatDateTime(node.updatedAt)}</td>
                        <td className="action-column">
                            <button 
                                className="btn btn-secondary btn-sm"
                                onClick={(e) => {
                                    e.stopPropagation()
                                    onSelectNode(node)
                                }}
                            >
                                查看详情
                            </button>
                        </td>
                    </tr>
                ))}
            </tbody>
        </table>
    )
}
