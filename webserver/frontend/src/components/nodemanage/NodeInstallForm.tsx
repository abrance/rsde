import { useState } from 'react';
import { NodeInstallRequest, NodeInstallTaskReceipt } from '../../types/nodemanage';
import { installNode } from '../../lib/nodemanage';
import './NodeInstallForm.css';

export interface NodeInstallFormProps {
    nodeId: string;
    onSuccess: (receipt: NodeInstallTaskReceipt) => void;
}

export default function NodeInstallForm({ nodeId, onSuccess }: NodeInstallFormProps) {
    const [host, setHost] = useState('');
    const [username, setUsername] = useState('');
    const [password, setPassword] = useState('');
    const [privateKey, setPrivateKey] = useState('');
    const [packageUrl, setPackageUrl] = useState('');
    
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [validationErrors, setValidationErrors] = useState<string[]>([]);
    
    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        
        const errors: string[] = [];
        if (!host.trim()) errors.push('Host is required');
        if (!username.trim()) errors.push('Username is required');
        if (!packageUrl.trim()) errors.push('Package URL is required');
        if (!password && !privateKey) errors.push('Must provide either Password or Private Key');
        
        if (errors.length > 0) {
            setValidationErrors(errors);
            return;
        }
        
        setValidationErrors([]);
        setIsSubmitting(true);
        setError(null);
        
        try {
            const payload: NodeInstallRequest = {
                host: host.trim(),
                username: username.trim(),
                rsagent_package_url: packageUrl.trim(),
                ...(password ? { password } : {}),
                ...(privateKey ? { private_key: privateKey } : {}),
            };
            
            const receipt = await installNode(nodeId, payload);
            onSuccess(receipt);
        } catch (err: unknown) {
            const message = err instanceof Error ? err.message : 'Failed to submit install request';
            setError(message);
        } finally {
            setIsSubmitting(false);
        }
    };
    
    return (
        <div className="node-install-form card" data-testid="node-install-form">
            <h3>Install Node</h3>
            
            {validationErrors.length > 0 && (
                <div className="validation-errors">
                    {validationErrors.map((msg, idx) => (
                        <div key={idx} className="error-text">{msg}</div>
                    ))}
                </div>
            )}
            
            {error && (
                <div className="error-message">
                    {error}
                </div>
            )}
            
            <form onSubmit={handleSubmit}>
                <div className="form-group">
                    <label htmlFor="install-host">Host</label>
                    <input 
                        id="install-host"
                        type="text" 
                        value={host}
                        onChange={e => setHost(e.target.value)}
                        placeholder="10.0.0.1"
                    />
                </div>
                
                <div className="form-group">
                    <label htmlFor="install-username">Username</label>
                    <input 
                        id="install-username"
                        type="text" 
                        value={username}
                        onChange={e => setUsername(e.target.value)}
                        placeholder="root"
                    />
                </div>
                
                <div className="form-group">
                    <label htmlFor="install-password">Password</label>
                    <input 
                        id="install-password"
                        type="password" 
                        value={password}
                        onChange={e => setPassword(e.target.value)}
                    />
                </div>
                
                <div className="form-group">
                    <label htmlFor="install-private-key">Private Key</label>
                    <textarea 
                        id="install-private-key"
                        value={privateKey}
                        onChange={e => setPrivateKey(e.target.value)}
                        rows={3}
                    />
                </div>
                
                <div className="form-group">
                    <label htmlFor="install-package-url">Package URL</label>
                    <input 
                        id="install-package-url"
                        type="text" 
                        value={packageUrl}
                        onChange={e => setPackageUrl(e.target.value)}
                        placeholder="http://..."
                    />
                </div>
                
                <div className="form-actions">
                    <button type="submit" disabled={isSubmitting}>
                        {isSubmitting ? 'Submitting...' : 'Submit Install'}
                    </button>
                </div>
            </form>
        </div>
    );
}
