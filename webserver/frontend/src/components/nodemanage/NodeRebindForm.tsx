import { useState } from 'react';
import { rebindNode } from '../../data/nodemanage';
import './NodeRebindForm.css';

export default function NodeRebindForm({
    nodeId,
    onSuccess,
    onCancel
}: {
    nodeId: string;
    onSuccess: () => void;
    onCancel: () => void;
}) {
    const [agentId, setAgentId] = useState('');
    const [reason, setReason] = useState('');
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!agentId.trim()) {
            setError('目标 Agent ID 不能为空');
            return;
        }

        setIsSubmitting(true);
        setError(null);
        try {
            await rebindNode(nodeId, { targetAgentId: agentId, reason });
            onSuccess();
        } catch (err) {
            setError(err instanceof Error ? err.message : '重新绑定失败');
        } finally {
            setIsSubmitting(false);
        }
    };

    return (
        <div className="rebind-form-container">
            <h3>重新绑定节点</h3>
            <form onSubmit={handleSubmit}>
                <div className="form-group">
                    <label htmlFor="agentIdInput">目标 Agent ID</label>
                    <input 
                        id="agentIdInput"
                        type="text" 
                        value={agentId} 
                        onChange={e => { setAgentId(e.target.value); setError(null); }}
                        disabled={isSubmitting}
                    />
                </div>
                <div className="form-group">
                    <label htmlFor="reasonInput">原因 (可选)</label>
                    <input 
                        id="reasonInput"
                        type="text" 
                        value={reason} 
                        onChange={e => setReason(e.target.value)}
                        disabled={isSubmitting}
                    />
                </div>
                {error && <p className="error-text">{error}</p>}
                <div className="form-actions">
                    <button type="button" className="btn btn-secondary" onClick={onCancel} disabled={isSubmitting}>取消</button>
                    <button type="submit" className="btn btn-warning" disabled={isSubmitting}>
                        {isSubmitting ? '提交中...' : '确认重绑'}
                    </button>
                </div>
            </form>
        </div>
    );
}
