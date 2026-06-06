import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import * as api from './api';
import * as nm from './nodemanage';

describe('NodeManage API Client', () => {
  beforeEach(() => {
    vi.spyOn(api, 'requestJson').mockResolvedValue({});
  });

  afterEach(() => {
    vi.resetAllMocks();
  });

  it('listNodes calls GET /api/nm/v1/nodes', async () => {
    await nm.listNodes();
    expect(api.requestJson).toHaveBeenCalledWith('/api/nm/v1/nodes', { method: 'GET' });
  });

  it('getNode calls GET /api/nm/v1/nodes/:node_id', async () => {
    await nm.getNode('node-123');
    expect(api.requestJson).toHaveBeenCalledWith('/api/nm/v1/nodes/node-123', { method: 'GET' });
  });

  it('getNodeStatusBatch calls GET with comma-separated node_ids', async () => {
    await nm.getNodeStatusBatch(['node-1', 'node-2']);
    expect(api.requestJson).toHaveBeenCalledWith('/api/nm/v1/nodes/status:batch?node_ids=node-1,node-2', { method: 'GET' });
  });

  it('getNodeStatusBatch short-circuits on empty array', async () => {
    const result = await nm.getNodeStatusBatch([]);
    expect(result).toEqual([]);
    expect(api.requestJson).not.toHaveBeenCalled();
  });

  it('getNodeBinding calls GET /api/nm/v1/nodes/:node_id/binding', async () => {
    await nm.getNodeBinding('node-123');
    expect(api.requestJson).toHaveBeenCalledWith('/api/nm/v1/nodes/node-123/binding', { method: 'GET' });
  });

  it('installNode calls POST /api/nm/v1/nodes/:node_id/install', async () => {
    const payload = {
      host: '127.0.0.1',
      username: 'root',
      rsagent_package_url: 'http://example.com/agent.tar.gz'
    };
    await nm.installNode('node-123', payload);
    expect(api.requestJson).toHaveBeenCalledWith('/api/nm/v1/nodes/node-123/install', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
  });

  it('getInstallTask calls GET /api/nm/v1/install-tasks/:install_task_id', async () => {
    await nm.getInstallTask('task-456');
    expect(api.requestJson).toHaveBeenCalledWith('/api/nm/v1/install-tasks/task-456', { method: 'GET' });
  });

  it('getLatestInstallTask calls GET /api/nm/v1/nodes/:node_id/install-tasks:latest', async () => {
    await nm.getLatestInstallTask('node-123');
    expect(api.requestJson).toHaveBeenCalledWith('/api/nm/v1/nodes/node-123/install-tasks:latest', { method: 'GET' });
  });

  it('getLatestInstallTask handles nullable success payload gracefully', async () => {
    vi.spyOn(api, 'requestJson').mockResolvedValueOnce(null);
    const result = await nm.getLatestInstallTask('node-missing-task');
    expect(api.requestJson).toHaveBeenCalledWith('/api/nm/v1/nodes/node-missing-task/install-tasks:latest', { method: 'GET' });
    expect(result).toBeNull();
  });

  it('rebindNode calls POST /api/nm/v1/nodes/:node_id/rebind', async () => {
    const payload = { target_agent_id: 'agent-99', reason: 'fix' };
    await nm.rebindNode('node-123', payload);
    expect(api.requestJson).toHaveBeenCalledWith('/api/nm/v1/nodes/node-123/rebind', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
  });

  it('syncAgent calls POST /api/nm/v1/agents/sync', async () => {
    const payload = {
      agent_id: 'agent-1',
      agent_version: '1.0.0',
      hostname: 'host1',
      os_family: 'linux',
      os_distribution: 'ubuntu',
      arch: 'amd64',
      capabilities: [],
      started_at: '2026-06-06T00:00:00Z',
    };
    await nm.syncAgent(payload);
    expect(api.requestJson).toHaveBeenCalledWith('/api/nm/v1/agents/sync', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    });
  });

  it('encodes URL path parameters and query strings properly', async () => {
    await nm.getNode('node/123@#');
    expect(api.requestJson).toHaveBeenCalledWith('/api/nm/v1/nodes/node%2F123%40%23', { method: 'GET' });
    
    await nm.getNodeStatusBatch(['node-1', 'weird/node']);
    expect(api.requestJson).toHaveBeenCalledWith('/api/nm/v1/nodes/status:batch?node_ids=node-1,weird%2Fnode', { method: 'GET' });
  });
});
