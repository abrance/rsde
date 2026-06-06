import { render, screen, waitFor, fireEvent, act } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest'
import NodeManagePage from './NodeManagePage'
import * as nmApi from '../lib/nodemanage'
import { StructuredApiError } from '../types/api'
import { NodeSummary, NodeDetail, NodeInstallTaskView } from '../types/nodemanage'

vi.mock('../lib/nodemanage', () => ({
  listNodes: vi.fn(),
  getNode: vi.fn(),
  getLatestInstallTask: vi.fn(),
  getNodeBinding: vi.fn(),
  installNode: vi.fn(),
  rebindNode: vi.fn(),
  getNodeStatusBatch: vi.fn(),
}))

const mockNodes: NodeSummary[] = [
  {
    node_id: 'node-1',
    node_name: 'test-node-1',
    environment: 'prod',
    labels: ['db'],
    lifecycle_state: 'ACTIVE',
    install_phase: 'COMPLETED',
    binding_state: 'BOUND',
    online_status: 'ONLINE',
    updated_at: '2026-01-01T00:00:00Z'
  },
  {
    node_id: 'node-2',
    node_name: 'test-node-2',
    environment: 'dev',
    labels: ['web'],
    lifecycle_state: 'PENDING',
    install_phase: 'PENDING',
    binding_state: 'UNBOUND',
    online_status: 'OFFLINE',
    updated_at: '2026-01-01T00:00:00Z'
  }
]

describe('NodeManagePage UI', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('renders summary cards based on list data', async () => {
    vi.mocked(nmApi.listNodes).mockResolvedValue({
      items: mockNodes,
      total: 2,
      page: 1,
      page_size: 10,
      total_pages: 1
    })

    render(
      <MemoryRouter>
        <NodeManagePage />
      </MemoryRouter>
    )

    expect(nmApi.listNodes).toHaveBeenCalled()
    
    await waitFor(() => {
      expect(screen.getByText('Total Nodes: 2')).toBeInTheDocument()
      expect(screen.getByText('Online: 1')).toBeInTheDocument()
      expect(screen.getByText('Bound: 1')).toBeInTheDocument()
    })
  })

  it('renders list of nodes and supports client-side filtering', async () => {
    vi.mocked(nmApi.listNodes).mockResolvedValue({
      items: mockNodes,
      total: 2,
      page: 1,
      page_size: 10,
      total_pages: 1
    })

    render(
      <MemoryRouter>
        <NodeManagePage />
      </MemoryRouter>
    )

    await waitFor(() => {
      expect(screen.getByText('test-node-1')).toBeInTheDocument()
      expect(screen.getByText('test-node-2')).toBeInTheDocument()
    })

    const filterInput = screen.getByPlaceholderText('Filter nodes...')
    fireEvent.change(filterInput, { target: { value: 'node-1' } })

    expect(screen.getByText('test-node-1')).toBeInTheDocument()
    expect(screen.queryByText('test-node-2')).not.toBeInTheDocument()
  })

  it('handles row selection, triggers detail fetch with stale-request protection', async () => {
    vi.mocked(nmApi.listNodes).mockResolvedValue({
      items: mockNodes,
      total: 2,
      page: 1,
      page_size: 10,
      total_pages: 1
    })

    let resolveGetNode1: (val: NodeDetail) => void
    let resolveGetNode2: (val: NodeDetail) => void
    const getNodePromise1 = new Promise<NodeDetail>((resolve) => { resolveGetNode1 = resolve })
    const getNodePromise2 = new Promise<NodeDetail>((resolve) => { resolveGetNode2 = resolve })

    vi.mocked(nmApi.getNode).mockImplementation((nodeId) => {
      if (nodeId === 'node-1') return getNodePromise1 as Promise<NodeDetail>
      if (nodeId === 'node-2') return getNodePromise2 as Promise<NodeDetail>
      return Promise.resolve({} as NodeDetail)
    })

    vi.mocked(nmApi.getLatestInstallTask).mockResolvedValue({
      task_state: 'COMPLETED'
    } as NodeInstallTaskView)

    render(
      <MemoryRouter>
        <NodeManagePage />
      </MemoryRouter>
    )

    await waitFor(() => {
      expect(screen.getByText('test-node-1')).toBeInTheDocument()
    })

    fireEvent.click(screen.getByText('test-node-1'))
    expect(nmApi.getNode).toHaveBeenCalledWith('node-1')
    expect(nmApi.getLatestInstallTask).toHaveBeenCalledWith('node-1')

    fireEvent.click(screen.getByText('test-node-2'))
    expect(nmApi.getNode).toHaveBeenCalledWith('node-2')
    expect(nmApi.getLatestInstallTask).toHaveBeenCalledWith('node-2')

    const createMockDetail = (id: string) => ({
      node: { node_id: id, node_name: `test-${id}`, environment: 'dev', labels: [], endpoint: '', created_at: '', updated_at: '' },
      status: { lifecycle_state: 'ACTIVE', install_phase: 'COMPLETED', binding_state: 'BOUND', online_status: 'ONLINE' },
      binding: undefined
    })

    resolveGetNode1!(createMockDetail('node-1'))
    resolveGetNode2!(createMockDetail('node-2'))

    await waitFor(() => {
      expect(screen.getByTestId('selected-node-id')).toHaveTextContent('node-2')
      expect(screen.getByTestId('detail-node-name')).toHaveTextContent('test-node-2')
      expect(screen.getByTestId('task-tracking-state')).toHaveTextContent('COMPLETED')
    })
  })

  it('renders user-visible error states for list loading failure', async () => {
    vi.mocked(nmApi.listNodes).mockRejectedValue(new Error('Network error'))

    render(
      <MemoryRouter>
        <NodeManagePage />
      </MemoryRouter>
    )

    await waitFor(() => {
      expect(screen.getByTestId('list-error')).toHaveTextContent('Failed to load node list: Network error')
    })
  })

  it('renders user-visible error states for detail fetching failure', async () => {
    vi.mocked(nmApi.listNodes).mockResolvedValue({
      items: [mockNodes[0]],
      total: 1,
      page: 1,
      page_size: 10,
      total_pages: 1
    })

    vi.mocked(nmApi.getNode).mockRejectedValue(new Error('Detail fetch failed'))
    vi.mocked(nmApi.getLatestInstallTask).mockRejectedValue(new Error('Task fetch failed'))

    render(
      <MemoryRouter>
        <NodeManagePage />
      </MemoryRouter>
    )

    await waitFor(() => {
      expect(screen.getByText('test-node-1')).toBeInTheDocument()
    })

    fireEvent.click(screen.getByText('test-node-1'))

    await waitFor(() => {
      expect(screen.getByTestId('detail-error')).toHaveTextContent('Error loading detail: Detail fetch failed')
      expect(screen.getByTestId('task-error')).toHaveTextContent('Error loading task: Task fetch failed')
    })
  })

  it('renders explicit empty state when filter yields zero matches', async () => {
    vi.mocked(nmApi.listNodes).mockResolvedValue({
      items: mockNodes,
      total: 2,
      page: 1,
      page_size: 10,
      total_pages: 1
    })

    render(
      <MemoryRouter>
        <NodeManagePage />
      </MemoryRouter>
    )

    await waitFor(() => {
      expect(screen.getByText('test-node-1')).toBeInTheDocument()
    })

    const filterInput = screen.getByPlaceholderText('Filter nodes...')
    fireEvent.change(filterInput, { target: { value: 'non-existent-node-123' } })

    expect(screen.getByTestId('empty-filtered-state')).toHaveTextContent('No nodes match the filter.')
    expect(screen.queryByText('test-node-1')).not.toBeInTheDocument()
  })

  it('renders empty task state (not error) when latest task is absent/null', async () => {
    vi.mocked(nmApi.listNodes).mockResolvedValue({
      items: [mockNodes[0]],
      total: 1,
      page: 1,
      page_size: 10,
      total_pages: 1
    })

    vi.mocked(nmApi.getNode).mockResolvedValue({
      node: { node_id: 'node-1', node_name: 'test-node-1', environment: 'dev', labels: [], endpoint: '', created_at: '', updated_at: '' },
      status: { lifecycle_state: 'ACTIVE', install_phase: 'COMPLETED', binding_state: 'BOUND', online_status: 'ONLINE' },
      binding: undefined
    } as NodeDetail)
    
    vi.mocked(nmApi.getLatestInstallTask).mockResolvedValue(null)

    render(
      <MemoryRouter>
        <NodeManagePage />
      </MemoryRouter>
    )

    await waitFor(() => {
      expect(screen.getByText('test-node-1')).toBeInTheDocument()
    })

    fireEvent.click(screen.getByText('test-node-1'))

    await waitFor(() => {
      expect(screen.getByTestId('task-empty')).toHaveTextContent('No latest task found for this node.')
    })
  })

  it('renders placeholder detail panel when no node is selected', async () => {
    vi.mocked(nmApi.listNodes).mockResolvedValue({
      items: mockNodes, total: 2, page: 1, page_size: 10, total_pages: 1
    })
    render(<MemoryRouter><NodeManagePage /></MemoryRouter>)
    await waitFor(() => {
      expect(screen.getByTestId('detail-placeholder')).toHaveTextContent('Please select a node to view details')
    })
  })

  it('renders detail sections when node is selected', async () => {
    vi.mocked(nmApi.listNodes).mockResolvedValue({ items: mockNodes, total: 2, page: 1, page_size: 10, total_pages: 1 })
    vi.mocked(nmApi.getNode).mockResolvedValue({
      node: { node_id: 'node-1', node_name: 'test-node-1', environment: 'prod', labels: ['db'], endpoint: '10.0.0.1', created_at: '2026-01-01T00:00:00Z', updated_at: '2026-01-01T00:00:00Z' },
      status: { lifecycle_state: 'ACTIVE', install_phase: 'COMPLETED', binding_state: 'BOUND', online_status: 'ONLINE', last_heartbeat_at: '2026-01-01T00:00:00Z' },
      binding: { agent_id: 'agent-1', node_id: 'node-1', binding_state: 'BOUND', first_registered_at: '2026-01-01T00:00:00Z', last_handshake_at: '2026-01-01T00:00:00Z' },
      heartbeat_ref: { data_link_id: 'link-1', link_purpose: 'HEARTBEAT', owner_service: 'rsagent', result_table_name: 'rt_node_heartbeats' }
    })
    vi.mocked(nmApi.getLatestInstallTask).mockResolvedValue({
      task_state: 'COMPLETED'
    } as NodeInstallTaskView)

    render(<MemoryRouter><NodeManagePage /></MemoryRouter>)

    await waitFor(() => expect(screen.getByText('test-node-1')).toBeInTheDocument())
    fireEvent.click(screen.getByText('test-node-1'))

    await waitFor(() => {
      expect(screen.getByTestId('detail-node-name')).toHaveTextContent('test-node-1')
      expect(screen.getByTestId('detail-node-env')).toHaveTextContent('prod')
      expect(screen.getByTestId('detail-node-labels')).toHaveTextContent('db')
      
      expect(screen.getByTestId('detail-status-lifecycle')).toHaveTextContent('ACTIVE')
      expect(screen.getByTestId('detail-status-online')).toHaveTextContent('ONLINE')

      expect(screen.getByTestId('detail-binding-agent')).toHaveTextContent('agent-1')

      expect(screen.getByTestId('detail-heartbeat-link')).toHaveTextContent('link-1')

      expect(screen.getByTestId('task-tracking-state')).toHaveTextContent('COMPLETED')
    })
  })

  it('renders updated_at and status.binding_state even when binding is absent', async () => {
    vi.mocked(nmApi.listNodes).mockResolvedValue({ items: mockNodes, total: 2, page: 1, page_size: 10, total_pages: 1 })
    vi.mocked(nmApi.getNode).mockResolvedValue({
      node: { node_id: 'node-1', node_name: 'test-node-1', environment: 'prod', labels: [], endpoint: '', created_at: '2026-01-01T00:00:00Z', updated_at: '2026-06-07T00:00:00Z' },
      status: { lifecycle_state: 'ACTIVE', install_phase: 'PENDING', binding_state: 'UNBOUND', online_status: 'OFFLINE' },
      binding: undefined,
    })
    
    vi.mocked(nmApi.getNodeBinding).mockRejectedValue({
      code: 'BINDING_NOT_FOUND',
      message: 'binding not found'
    })
    
    vi.mocked(nmApi.getLatestInstallTask).mockResolvedValue(null)

    render(<MemoryRouter><NodeManagePage /></MemoryRouter>)

    await waitFor(() => expect(screen.getByText('test-node-1')).toBeInTheDocument())
    fireEvent.click(screen.getByText('test-node-1'))

    await waitFor(() => {
      expect(screen.getByTestId('detail-node-updated')).toHaveTextContent('2026-06-07T00:00:00Z')
      expect(screen.getByTestId('detail-status-binding')).toHaveTextContent('UNBOUND')
      expect(screen.getByTestId('detail-binding-empty')).toHaveTextContent('暂无绑定')
      
      expect(screen.queryByTestId('detail-node-endpoint')).not.toBeInTheDocument()
      expect(screen.queryByTestId('detail-binding-first-reg')).not.toBeInTheDocument()
      expect(screen.queryByTestId('detail-heartbeat-purpose')).not.toBeInTheDocument()
    })
  })

  it('renders explicit generic error when dedicated binding fetch fails for reason other than BINDING_NOT_FOUND', async () => {
    vi.mocked(nmApi.listNodes).mockResolvedValue({ items: mockNodes, total: 2, page: 1, page_size: 10, total_pages: 1 })
    vi.mocked(nmApi.getNode).mockResolvedValue({
      node: { node_id: 'node-1', node_name: 'test-node-1', environment: 'prod', labels: [], endpoint: '', created_at: '2026-01-01T00:00:00Z', updated_at: '2026-06-07T00:00:00Z' },
      status: { lifecycle_state: 'ACTIVE', install_phase: 'PENDING', binding_state: 'UNBOUND', online_status: 'OFFLINE' },
    })
    
    vi.mocked(nmApi.getNodeBinding).mockRejectedValue(new Error('Generic Network Error'))
    vi.mocked(nmApi.getLatestInstallTask).mockResolvedValue(null)

    render(<MemoryRouter><NodeManagePage /></MemoryRouter>)

    await waitFor(() => expect(screen.getByText('test-node-1')).toBeInTheDocument())
    fireEvent.click(screen.getByText('test-node-1'))

    await waitFor(() => {
      expect(screen.getByTestId('binding-error')).toHaveTextContent('Error loading binding: Generic Network Error')
    })
  })

  it('renders binding data from dedicated binding read if it succeeds', async () => {
    vi.mocked(nmApi.listNodes).mockResolvedValue({ items: mockNodes, total: 2, page: 1, page_size: 10, total_pages: 1 })
    vi.mocked(nmApi.getNode).mockResolvedValue({
      node: { node_id: 'node-1', node_name: 'test-node-1', environment: 'prod', labels: [], endpoint: '', created_at: '2026-01-01T00:00:00Z', updated_at: '2026-06-07T00:00:00Z' },
      status: { lifecycle_state: 'ACTIVE', install_phase: 'COMPLETED', binding_state: 'BOUND', online_status: 'ONLINE' },
      binding: undefined
    })
    
    vi.mocked(nmApi.getNodeBinding).mockResolvedValue({
      agent_id: 'agent-99', node_id: 'node-1', binding_state: 'BOUND', first_registered_at: '2026-01-01', last_handshake_at: '2026-01-01'
    })
    
    vi.mocked(nmApi.getLatestInstallTask).mockResolvedValue(null)

    render(<MemoryRouter><NodeManagePage /></MemoryRouter>)

    await waitFor(() => expect(screen.getByText('test-node-1')).toBeInTheDocument())
    fireEvent.click(screen.getByText('test-node-1'))

    await waitFor(() => {
      expect(screen.getByTestId('detail-binding-agent')).toHaveTextContent('agent-99')
    })
  })

  describe('Node Install Form', () => {
    const mockDetailNode: NodeDetail = {
      node: {
        node_id: mockNodes[0].node_id,
        node_name: mockNodes[0].node_name,
        environment: mockNodes[0].environment,
        labels: mockNodes[0].labels,
        endpoint: '127.0.0.1:22',
        created_at: '2026-01-01T00:00:00Z',
        updated_at: mockNodes[0].updated_at
      },
      status: { lifecycle_state: 'ACTIVE', install_phase: 'COMPLETED', binding_state: 'BOUND', online_status: 'ONLINE' },
      binding: undefined
    }

    beforeEach(() => {
      vi.mocked(nmApi.listNodes).mockResolvedValue({
        items: [mockNodes[0]],
        total: 1,
        page: 1,
        page_size: 10,
        total_pages: 1
      })
      vi.mocked(nmApi.getNode).mockResolvedValue(mockDetailNode)
      vi.mocked(nmApi.getLatestInstallTask).mockResolvedValue(null)
      vi.mocked(nmApi.getNodeBinding).mockRejectedValue({ code: 'BINDING_NOT_FOUND', message: 'Not found' })
      vi.mocked(nmApi.installNode).mockClear()
    })

    it('renders install form when a node is selected', async () => {
      render(<MemoryRouter><NodeManagePage /></MemoryRouter>)
      await waitFor(() => expect(screen.getByText('test-node-1')).toBeInTheDocument())
      fireEvent.click(screen.getByText('test-node-1'))
      await waitFor(() => expect(screen.getByTestId('node-install-form')).toBeInTheDocument())
    })

    it('validates required fields before submitting', async () => {
      render(<MemoryRouter><NodeManagePage /></MemoryRouter>)
      await waitFor(() => expect(screen.getByText('test-node-1')).toBeInTheDocument())
      fireEvent.click(screen.getByText('test-node-1'))
      await waitFor(() => expect(screen.getByTestId('node-install-form')).toBeInTheDocument())

      const submitBtn = screen.getByRole('button', { name: /Submit Install/i })
      fireEvent.click(submitBtn)

      await waitFor(() => {
        expect(screen.getByText(/Host is required/i)).toBeInTheDocument()
        expect(screen.getByText(/Username is required/i)).toBeInTheDocument()
        expect(screen.getByText(/Package URL is required/i)).toBeInTheDocument()
        expect(screen.getByText(/Must provide either Password or Private Key/i)).toBeInTheDocument()
      })
      expect(nmApi.installNode).not.toHaveBeenCalled()
    })

    it('submits install and shows receipt banner on success', async () => {
      vi.mocked(nmApi.installNode).mockResolvedValue({
        install_task_id: 'task-123',
        node_id: 'node-1',
        accepted: true,
        task_state: 'PENDING'
      })

      render(<MemoryRouter><NodeManagePage /></MemoryRouter>)
      await waitFor(() => expect(screen.getByText('test-node-1')).toBeInTheDocument())
      fireEvent.click(screen.getByText('test-node-1'))
      await waitFor(() => expect(screen.getByTestId('node-install-form')).toBeInTheDocument())

      fireEvent.change(screen.getByLabelText(/Host/i), { target: { value: '10.0.0.1' } })
      fireEvent.change(screen.getByLabelText(/Username/i), { target: { value: 'root' } })
      fireEvent.change(screen.getByLabelText(/Password/i), { target: { value: 'secret' } })
      fireEvent.change(screen.getByLabelText(/Package URL/i), { target: { value: 'http://pkg' } })

      vi.mocked(nmApi.getNode).mockClear()
      vi.mocked(nmApi.getLatestInstallTask).mockClear()

      fireEvent.click(screen.getByRole('button', { name: /Submit Install/i }))

      await waitFor(() => {
        expect(nmApi.installNode).toHaveBeenCalledWith('node-1', {
          host: '10.0.0.1',
          username: 'root',
          password: 'secret',
          rsagent_package_url: 'http://pkg'
        })
      })

      await waitFor(() => {
        expect(screen.getByText(/Install request accepted/i)).toBeInTheDocument()
        expect(screen.getByText(/task-123/i)).toBeInTheDocument()
      })

      expect(nmApi.getNode).toHaveBeenCalledWith('node-1')
      expect(nmApi.getLatestInstallTask).toHaveBeenCalledWith('node-1')
    })
  })

  describe('Node Rebind Form', () => {
    const mockDetailNode: NodeDetail = {
      node: {
        node_id: mockNodes[0].node_id,
        node_name: mockNodes[0].node_name,
        environment: mockNodes[0].environment,
        labels: mockNodes[0].labels,
        endpoint: '127.0.0.1:22',
        created_at: '2026-01-01T00:00:00Z',
        updated_at: mockNodes[0].updated_at,
      },
      status: { lifecycle_state: 'ACTIVE', install_phase: 'COMPLETED', binding_state: 'BOUND', online_status: 'ONLINE' },
      binding: undefined,
    };

    beforeEach(() => {
      vi.mocked(nmApi.listNodes).mockResolvedValue({
        items: [mockNodes[0]],
        total: 1,
        page: 1,
        page_size: 10,
        total_pages: 1
      });
      vi.mocked(nmApi.getNode).mockResolvedValue(mockDetailNode);
      vi.mocked(nmApi.getLatestInstallTask).mockResolvedValue(null);
      vi.mocked(nmApi.getNodeBinding).mockRejectedValue({ code: 'BINDING_NOT_FOUND', message: 'Not found' });
      vi.mocked(nmApi.rebindNode).mockClear();
    });

    it('renders rebind form when a node is selected', async () => {
      render(<MemoryRouter><NodeManagePage /></MemoryRouter>);
      await waitFor(() => expect(screen.getByText('test-node-1')).toBeInTheDocument());
      fireEvent.click(screen.getByText('test-node-1'));
      await waitFor(() => expect(screen.getByTestId('node-rebind-form')).toBeInTheDocument());
    });

    it('blocks blank target_agent_id client-side', async () => {
      render(<MemoryRouter><NodeManagePage /></MemoryRouter>);
      await waitFor(() => expect(screen.getByText('test-node-1')).toBeInTheDocument());
      fireEvent.click(screen.getByText('test-node-1'));
      await waitFor(() => expect(screen.getByTestId('node-rebind-form')).toBeInTheDocument());

      const submitBtn = screen.getByRole('button', { name: /Force Rebind/i });
      fireEvent.click(submitBtn);

      await waitFor(() => {
        expect(screen.getByText(/Target Agent ID is required/i)).toBeInTheDocument();
      });
      expect(nmApi.rebindNode).not.toHaveBeenCalled();
    });

    it('displays TARGET_AGENT_NOT_FOUND error inline', async () => {
      vi.mocked(nmApi.rebindNode).mockRejectedValue(
        new StructuredApiError('Agent not found', 'TARGET_AGENT_NOT_FOUND')
      );

      render(<MemoryRouter><NodeManagePage /></MemoryRouter>);
      await waitFor(() => expect(screen.getByText('test-node-1')).toBeInTheDocument());
      fireEvent.click(screen.getByText('test-node-1'));
      await waitFor(() => expect(screen.getByTestId('node-rebind-form')).toBeInTheDocument());

      fireEvent.change(screen.getByLabelText(/Target Agent ID/i), { target: { value: 'missing-agent' } });
      fireEvent.click(screen.getByRole('button', { name: /Force Rebind/i }));

      await waitFor(() => {
        expect(screen.getByText(/Agent not found/i)).toBeInTheDocument();
      });
    });

    it('displays REBIND_TARGET_ALREADY_BOUND error inline', async () => {
      vi.mocked(nmApi.rebindNode).mockRejectedValue(
        new StructuredApiError('Target already bound to another node', 'REBIND_TARGET_ALREADY_BOUND')
      );

      render(<MemoryRouter><NodeManagePage /></MemoryRouter>);
      await waitFor(() => expect(screen.getByText('test-node-1')).toBeInTheDocument());
      fireEvent.click(screen.getByText('test-node-1'));
      await waitFor(() => expect(screen.getByTestId('node-rebind-form')).toBeInTheDocument());

      fireEvent.change(screen.getByLabelText(/Target Agent ID/i), { target: { value: 'busy-agent' } });
      fireEvent.click(screen.getByRole('button', { name: /Force Rebind/i }));

      await waitFor(() => {
        expect(screen.getByText(/Target already bound to another node/i)).toBeInTheDocument();
      });
    });

    it('resets rebind form state when switching to a different node', async () => {
      vi.mocked(nmApi.listNodes).mockResolvedValue({
        items: mockNodes,
        total: 2,
        page: 1,
        page_size: 10,
        total_pages: 1,
      });
      vi.mocked(nmApi.getNode).mockImplementation(async (nodeId) => ({
        node: {
          node_id: nodeId,
          node_name: nodeId === 'node-1' ? 'test-node-1' : 'test-node-2',
          environment: nodeId === 'node-1' ? 'prod' : 'dev',
          labels: [],
          endpoint: '127.0.0.1:22',
          created_at: '2026-01-01T00:00:00Z',
          updated_at: '2026-01-01T00:00:00Z',
        },
        status: { lifecycle_state: 'ACTIVE', install_phase: 'COMPLETED', binding_state: 'BOUND', online_status: 'ONLINE' },
        binding: undefined,
      }));
      vi.mocked(nmApi.rebindNode).mockRejectedValue(
        new StructuredApiError('Agent not found', 'TARGET_AGENT_NOT_FOUND')
      );

      render(<MemoryRouter><NodeManagePage /></MemoryRouter>);
      await waitFor(() => expect(screen.getByText('test-node-1')).toBeInTheDocument());

      fireEvent.click(screen.getByText('test-node-1'));
      await waitFor(() => expect(screen.getByTestId('node-rebind-form')).toBeInTheDocument());

      fireEvent.change(screen.getByLabelText(/Target Agent ID/i), { target: { value: 'stale-agent' } });
      fireEvent.change(screen.getByLabelText(/Reason/i), { target: { value: 'stale reason' } });
      fireEvent.click(screen.getByRole('button', { name: /Force Rebind/i }));

      await waitFor(() => {
        expect(screen.getByText(/Agent not found/i)).toBeInTheDocument();
      });

      fireEvent.click(screen.getByText('test-node-2'));

      await waitFor(() => {
        expect(screen.queryByText(/Agent not found/i)).not.toBeInTheDocument();
        expect(screen.getByLabelText(/Target Agent ID/i)).toHaveValue('');
        expect(screen.getByLabelText(/Reason/i)).toHaveValue('');
      });
    });

    it('submits successfully, triggers rereads, and shows success banner', async () => {
      vi.mocked(nmApi.rebindNode).mockResolvedValue({
        accepted: true,
        node_id: 'node-1',
        target_agent_id: 'new-agent',
        binding_state: 'BOUND'
      });

      render(<MemoryRouter><NodeManagePage /></MemoryRouter>);
      await waitFor(() => expect(screen.getByText('test-node-1')).toBeInTheDocument());
      fireEvent.click(screen.getByText('test-node-1'));
      await waitFor(() => expect(screen.getByTestId('node-rebind-form')).toBeInTheDocument());

      fireEvent.change(screen.getByLabelText(/Target Agent ID/i), { target: { value: 'new-agent' } });
      fireEvent.change(screen.getByLabelText(/Reason/i), { target: { value: 'fix binding' } });

      vi.mocked(nmApi.getNode).mockClear();
      vi.mocked(nmApi.getLatestInstallTask).mockClear();
      vi.mocked(nmApi.getNodeBinding).mockClear();
      vi.mocked(nmApi.listNodes).mockClear();

      fireEvent.click(screen.getByRole('button', { name: /Force Rebind/i }));

      await waitFor(() => {
        expect(nmApi.rebindNode).toHaveBeenCalledWith('node-1', {
          target_agent_id: 'new-agent',
          reason: 'fix binding'
        });
      });

      await waitFor(() => {
        expect(screen.getByText(/Rebind successful/i)).toBeInTheDocument();
      });

      expect(nmApi.getNode).toHaveBeenCalledWith('node-1');
      expect(nmApi.getLatestInstallTask).toHaveBeenCalledWith('node-1');
      expect(nmApi.getNodeBinding).toHaveBeenCalledWith('node-1');
      expect(nmApi.listNodes).toHaveBeenCalled();
    });
  });

  describe('List Polling with batch status API', () => {
    beforeEach(() => {
      vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] })
    })

    afterEach(() => {
      vi.useRealTimers()
    })

    it('polls for batch status with active node ids and merges updates without wiping rows', async () => {
      vi.mocked(nmApi.listNodes).mockResolvedValue({
        items: mockNodes,
        total: 2,
        page: 1,
        page_size: 10,
        total_pages: 1
      })
      vi.mocked(nmApi.getNodeStatusBatch).mockResolvedValue([
        { node_id: 'node-1', install_phase: 'COMPLETED', binding_state: 'BOUND', online_status: 'OFFLINE', updated_at: '2026-06-07T00:00:00Z', status_reason: 'Polled offline' }
      ])

      render(<MemoryRouter><NodeManagePage /></MemoryRouter>)

      await waitFor(() => {
        expect(screen.getByText('Total Nodes: 2')).toBeInTheDocument()
      })

      expect(nmApi.getNodeStatusBatch).not.toHaveBeenCalled()

      await act(async () => {
        await vi.advanceTimersByTimeAsync(5000)
      })

      expect(nmApi.getNodeStatusBatch).toHaveBeenCalledWith(['node-1', 'node-2'])

      await waitFor(() => {
        expect(screen.getByText('Online: 0')).toBeInTheDocument()
      })
    })

    it('shows StructuredApiError batch refresh failures non-blockingly without wiping rows', async () => {
      vi.mocked(nmApi.listNodes).mockResolvedValue({
        items: mockNodes,
        total: 2,
        page: 1,
        page_size: 10,
        total_pages: 1
      })
      vi.mocked(nmApi.getNodeStatusBatch).mockRejectedValue(
        new StructuredApiError('Batch refresh failed', 'INTERNAL_ERROR')
      )

      render(<MemoryRouter><NodeManagePage /></MemoryRouter>)

      await waitFor(() => {
        expect(screen.getByText('Total Nodes: 2')).toBeInTheDocument()
      })

      await waitFor(() => {
        expect(screen.getByText('test-node-1')).toBeInTheDocument()
        expect(screen.getByText('test-node-2')).toBeInTheDocument()
      })

      await act(async () => {
        await vi.advanceTimersByTimeAsync(5000)
      })

      await waitFor(() => {
        expect(screen.getByText('Total Nodes: 2')).toBeInTheDocument()
        expect(screen.getByTestId('batch-error')).toHaveTextContent('Batch refresh failed')
        expect(screen.getByText('test-node-1')).toBeInTheDocument()
        expect(screen.getByText('test-node-2')).toBeInTheDocument()
      })
    })

    it('continues polling with the same active node ids across repeated status-only updates', async () => {
      vi.mocked(nmApi.listNodes).mockResolvedValue({
        items: mockNodes,
        total: 2,
        page: 1,
        page_size: 10,
        total_pages: 1,
      })
      vi.mocked(nmApi.getNodeStatusBatch)
        .mockResolvedValueOnce([
          {
            node_id: 'node-1',
            install_phase: 'COMPLETED',
            binding_state: 'BOUND',
            online_status: 'OFFLINE',
            updated_at: '2026-06-07T00:00:00Z',
            status_reason: 'Polled offline',
          },
        ])
        .mockResolvedValueOnce([
          {
            node_id: 'node-1',
            install_phase: 'COMPLETED',
            binding_state: 'BOUND',
            online_status: 'ONLINE',
            updated_at: '2026-06-07T00:00:05Z',
            status_reason: 'Polled back online',
          },
        ])

      render(<MemoryRouter><NodeManagePage /></MemoryRouter>)

      await waitFor(() => {
        expect(screen.getByText('Total Nodes: 2')).toBeInTheDocument()
      })

      await act(async () => {
        await vi.advanceTimersByTimeAsync(5000)
      })

      await waitFor(() => {
        expect(screen.getByText('Online: 0')).toBeInTheDocument()
      })

      expect(nmApi.getNodeStatusBatch).toHaveBeenNthCalledWith(1, ['node-1', 'node-2'])

      await act(async () => {
        await vi.advanceTimersByTimeAsync(5000)
      })

      await waitFor(() => {
        expect(screen.getByText('Online: 1')).toBeInTheDocument()
      })

      expect(nmApi.getNodeStatusBatch).toHaveBeenNthCalledWith(2, ['node-1', 'node-2'])
    })

    it('does not start a second batch poll while a previous poll is still in flight', async () => {
      vi.mocked(nmApi.listNodes).mockResolvedValue({
        items: mockNodes,
        total: 2,
        page: 1,
        page_size: 10,
        total_pages: 1,
      })

      let resolveFirstPoll: ((value: Parameters<typeof Promise.resolve>[0]) => void) | undefined
      const firstPollPromise = new Promise((resolve) => {
        resolveFirstPoll = resolve
      })

      vi.mocked(nmApi.getNodeStatusBatch)
        .mockReturnValueOnce(firstPollPromise as Promise<any>)
        .mockResolvedValueOnce([
          {
            node_id: 'node-1',
            install_phase: 'COMPLETED',
            binding_state: 'BOUND',
            online_status: 'OFFLINE',
            updated_at: '2026-06-07T00:00:05Z',
            status_reason: 'Recovered after first poll',
          },
        ])

      render(<MemoryRouter><NodeManagePage /></MemoryRouter>)

      await waitFor(() => {
        expect(screen.getByText('Total Nodes: 2')).toBeInTheDocument()
      })

      await act(async () => {
        await vi.advanceTimersByTimeAsync(5000)
      })

      expect(nmApi.getNodeStatusBatch).toHaveBeenCalledTimes(1)

      await act(async () => {
        await vi.advanceTimersByTimeAsync(5000)
      })

      expect(nmApi.getNodeStatusBatch).toHaveBeenCalledTimes(1)

      resolveFirstPoll?.([
        {
          node_id: 'node-1',
          install_phase: 'COMPLETED',
          binding_state: 'BOUND',
          online_status: 'ONLINE',
          updated_at: '2026-06-07T00:00:00Z',
          status_reason: 'First poll resolved',
        },
      ])

      await act(async () => {
        await Promise.resolve()
      })

      await act(async () => {
        await vi.advanceTimersByTimeAsync(5000)
      })

      expect(nmApi.getNodeStatusBatch).toHaveBeenCalledTimes(2)
    })
  })
})
