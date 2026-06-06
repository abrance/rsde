import { requestJson } from './api';
import {
  PaginatedNodeList,
  NodeDetail,
  NodeStatusBatchItem,
  NodeBindingView,
  NodeInstallRequest,
  NodeInstallTaskReceipt,
  NodeInstallTaskView,
  RebindNodeRequest,
  RebindNodeResponse,
  AgentSyncRequest,
  AgentSyncResponse,
} from '../types/nodemanage';

export async function listNodes(): Promise<PaginatedNodeList> {
  return requestJson<PaginatedNodeList>('/api/nm/v1/nodes', { method: 'GET' });
}

export async function getNode(nodeId: string): Promise<NodeDetail> {
  return requestJson<NodeDetail>(`/api/nm/v1/nodes/${encodeURIComponent(nodeId)}`, { method: 'GET' });
}

export async function getNodeStatusBatch(nodeIds: string[]): Promise<NodeStatusBatchItem[]> {
  if (nodeIds.length === 0) return [];
  const ids = nodeIds.map(encodeURIComponent).join(',');
  return requestJson<NodeStatusBatchItem[]>(`/api/nm/v1/nodes/status:batch?node_ids=${ids}`, { method: 'GET' });
}

export async function getNodeBinding(nodeId: string): Promise<NodeBindingView> {
  return requestJson<NodeBindingView>(`/api/nm/v1/nodes/${encodeURIComponent(nodeId)}/binding`, { method: 'GET' });
}

export async function installNode(nodeId: string, payload: NodeInstallRequest): Promise<NodeInstallTaskReceipt> {
  return requestJson<NodeInstallTaskReceipt>(`/api/nm/v1/nodes/${encodeURIComponent(nodeId)}/install`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
}

export async function getInstallTask(taskId: string): Promise<NodeInstallTaskView | null> {
  return requestJson<NodeInstallTaskView | null>(`/api/nm/v1/install-tasks/${encodeURIComponent(taskId)}`, { method: 'GET' });
}

export async function getLatestInstallTask(nodeId: string): Promise<NodeInstallTaskView | null> {
  return requestJson<NodeInstallTaskView | null>(`/api/nm/v1/nodes/${encodeURIComponent(nodeId)}/install-tasks:latest`, { method: 'GET' });
}

export async function rebindNode(nodeId: string, payload: RebindNodeRequest): Promise<RebindNodeResponse> {
  return requestJson<RebindNodeResponse>(`/api/nm/v1/nodes/${encodeURIComponent(nodeId)}/rebind`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
}

export async function syncAgent(payload: AgentSyncRequest): Promise<AgentSyncResponse> {
  return requestJson<AgentSyncResponse>('/api/nm/v1/agents/sync', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
}
