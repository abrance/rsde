import { useState } from 'react';
import type { InstallNodePayload } from '../../types/nodemanage';
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
    const [showAdvanced, setShowAdvanced] = useState(false);
    const [sshPort, setSshPort] = useState('');
    const [username, setUsername] = useState('');
    const [password, setPassword] = useState('');
    const [privateKey, setPrivateKey] = useState('');
    const [installRoot, setInstallRoot] = useState('');
    const [rsagentUrl, setRsagentUrl] = useState('');

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setIsSubmitting(true);
        setError(null);

        const payload: InstallNodePayload = {};
        if (sshPort.trim()) payload.ssh_port = parseInt(sshPort.trim(), 10);
        if (username.trim()) payload.username = username.trim();
        if (password) payload.password = password;
        if (privateKey.trim()) payload.private_key = privateKey.trim();
        if (installRoot.trim()) payload.install_root = installRoot.trim();
        if (rsagentUrl.trim()) payload.rsagent_package_url = rsagentUrl.trim();

        const hasPayload = Object.keys(payload).length > 0;

        try {
            await installNodeAgent(nodeId, hasPayload ? payload : undefined);
            onSuccess();
        } catch (err) {
            setError(err instanceof Error ? err.message : '安装请求失败');
        } finally {
            setIsSubmitting(false);
        }
    };

    return (
        <div className="install-form-container">
            <h3>安装 Agent</h3>
            <form onSubmit={handleSubmit}>
                <p>将在此节点上安装 Agent。此操作将下发安装任务。</p>

                <button
                    type="button"
                    className="btn btn-text advanced-toggle"
                    onClick={() => setShowAdvanced(!showAdvanced)}
                >
                    {showAdvanced ? '▾ 收起覆盖参数' : '▸ 覆盖参数（可选）'}
                </button>

                {showAdvanced && (
                    <div className="advanced-fields">
                        <div className="form-row">
                            <div className="form-group form-group-half">
                                <label htmlFor="installSshPort">SSH 端口</label>
                                <input
                                    id="installSshPort"
                                    type="number"
                                    value={sshPort}
                                    onChange={e => setSshPort(e.target.value)}
                                    placeholder="22"
                                    disabled={isSubmitting}
                                />
                            </div>
                            <div className="form-group form-group-half">
                                <label htmlFor="installUsername">SSH 用户名</label>
                                <input
                                    id="installUsername"
                                    type="text"
                                    value={username}
                                    onChange={e => setUsername(e.target.value)}
                                    placeholder="root"
                                    disabled={isSubmitting}
                                />
                            </div>
                        </div>
                        <div className="form-group">
                            <label htmlFor="installPassword">SSH 密码</label>
                            <input
                                id="installPassword"
                                type="password"
                                value={password}
                                onChange={e => setPassword(e.target.value)}
                                disabled={isSubmitting}
                            />
                        </div>
                        <div className="form-group">
                            <label htmlFor="installPrivateKey">SSH 私钥</label>
                            <textarea
                                id="installPrivateKey"
                                value={privateKey}
                                onChange={e => setPrivateKey(e.target.value)}
                                placeholder="-----BEGIN OPENSSH PRIVATE KEY-----"
                                rows={3}
                                disabled={isSubmitting}
                            />
                        </div>
                        <div className="form-group">
                            <label htmlFor="installRoot">安装目录</label>
                            <input
                                id="installRoot"
                                type="text"
                                value={installRoot}
                                onChange={e => setInstallRoot(e.target.value)}
                                placeholder="/opt/rsagent"
                                disabled={isSubmitting}
                            />
                        </div>
                        <div className="form-group">
                            <label htmlFor="rsagentUrl">安装包地址</label>
                            <input
                                id="rsagentUrl"
                                type="text"
                                value={rsagentUrl}
                                onChange={e => setRsagentUrl(e.target.value)}
                                placeholder="https://..."
                                disabled={isSubmitting}
                            />
                        </div>
                    </div>
                )}

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
