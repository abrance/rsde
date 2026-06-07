import { useState } from 'react';
import { installNodeAgent } from '../../data/nodemanage';
import './NodeInstallForm.css';

export default function NodeInstallForm({
    nodeId,
    onSuccess,
    onCancel
}: {
    nodeId: string;
    onSuccess: () => void;
    onCancel: () => void;
}) {
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setIsSubmitting(true);
        setError(null);
        try {
            await installNodeAgent(nodeId);
            onSuccess();
        } catch (err) {
            setError(err instanceof Error ? err.message : '安装请求失败');
        } finally {
            setIsSubmitting(false);
        }
    };

    return (
        <div className="install-form-container">
            <h3>确认安装</h3>
            <form onSubmit={handleSubmit}>
                <p>将在此节点上安装 Agent。此操作将下发安装任务。</p>
                {error && <p className="error-text">{error}</p>}
                <div className="form-actions">
                    <button type="button" className="btn btn-secondary" onClick={onCancel} disabled={isSubmitting}>取消</button>
                    <button type="submit" className="btn btn-primary" disabled={isSubmitting}>
                        {isSubmitting ? '提交中...' : '确认安装'}
                    </button>
                </div>
            </form>
        </div>
    );
}
