import React, { useState } from 'react';
import './NodeRebindForm.css';
import { rebindNode } from '../../lib/nodemanage';
import { StructuredApiError } from '../../types/api';

interface NodeRebindFormProps {
  nodeId: string;
  onSuccess: () => void;
}

export const NodeRebindForm: React.FC<NodeRebindFormProps> = ({ nodeId, onSuccess }) => {
  const [targetAgentId, setTargetAgentId] = useState('');
  const [reason, setReason] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setSuccess(false);

    const trimmedAgentId = targetAgentId.trim();
    if (!trimmedAgentId) {
      setError('Target Agent ID is required');
      return;
    }

    setLoading(true);
    try {
      await rebindNode(nodeId, {
        target_agent_id: trimmedAgentId,
        reason: reason.trim() || undefined,
      });
      setSuccess(true);
      onSuccess();
    } catch (err: unknown) {
      const message =
        err instanceof StructuredApiError
          ? err.message
          : err instanceof Error
            ? err.message
            : 'An unknown error occurred during rebind';
      setError(message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="node-rebind-form card" data-testid="node-rebind-form">
      <h3>Force Rebind</h3>
      <p className="rebind-help">
        Manually associate this node with a specific agent. Use only to recover from binding conflicts.
      </p>

      {error && <div className="rebind-error">{error}</div>}
      {success && <div className="rebind-success">Rebind successful. Refreshing...</div>}

      <form onSubmit={handleSubmit}>
        <div className="form-group">
          <label htmlFor="rebind-target-agent-id">Target Agent ID</label>
          <input
            id="rebind-target-agent-id"
            type="text"
            placeholder="agent-..."
            value={targetAgentId}
            onChange={(e) => setTargetAgentId(e.target.value)}
            disabled={loading}
          />
        </div>

        <div className="form-group">
          <label htmlFor="rebind-reason">Reason (Optional)</label>
          <input
            id="rebind-reason"
            type="text"
            placeholder="Recovery from conflict..."
            value={reason}
            onChange={(e) => setReason(e.target.value)}
            disabled={loading}
          />
        </div>

        <div className="form-actions">
          <button type="submit" disabled={loading} className="btn-danger">
            {loading ? 'Submitting...' : 'Force Rebind'}
          </button>
        </div>
      </form>
    </div>
  );
};
