import { render, screen, fireEvent, waitFor, act } from '@testing-library/react'
import { describe, expect, it, beforeEach, afterEach, vi, type Mock } from 'vitest'
import App from '../App'
import { fetchNodes, createNode, fetchNodeDetail, fetchNodeBinding, fetchLatestInstallTask, installNodeAgent, rebindNode, fetchNodeStatusBatch } from '../data/nodemanage'
import NodeDetailPanel from '../components/nodemanage/NodeDetailPanel'

vi.mock('../data/nodemanage', () => ({
    fetchNodes: vi.fn(),
    createNode: vi.fn(),
    fetchNodeDetail: vi.fn(),
    fetchNodeBinding: vi.fn(),
    fetchLatestInstallTask: vi.fn(),
    installNodeAgent: vi.fn(),
    rebindNode: vi.fn(),
    fetchNodeStatusBatch: vi.fn()
}))

describe('NodeManagePage Integration', () => {
    beforeEach(() => {
        window.history.pushState({}, '', '/')
        vi.clearAllMocks()
    })

    it('renders the NodeManage nav link inside layout', () => {
        window.history.pushState({}, '', '/')
        render(<App />)
        expect(screen.getByRole('link', { name: 'NodeManage' })).toBeInTheDocument()
    })

    it('renders the NodeManage route heading via App routing', async () => {
        ;(fetchNodes as Mock).mockResolvedValue({ nodes: [], total: 0 })
        window.history.pushState({}, '', '/node-manage')
        render(<App />)
        expect(await screen.findByRole('heading', { name: '节点管理 (NodeManage)' })).toBeInTheDocument()
    })

    it('renders the empty state with CTA and correct instruction copy', async () => {
        ;(fetchNodes as Mock).mockResolvedValue({ nodes: [], total: 0 })
        window.history.pushState({}, '', '/node-manage')
        render(<App />)
        expect(await screen.findByRole('button', { name: '纳管节点' })).toBeInTheDocument()
        expect(screen.getByText(/纳管节点 → 安装 agent → 等待\/确认 binding → 节点可用于作业平台/)).toBeInTheDocument()
    })

    it('renders the error state on fetch failure and allows retry', async () => {
        ;(fetchNodes as Mock)
            .mockRejectedValueOnce(new Error('Network disconnected'))
            .mockResolvedValueOnce({ nodes: [], total: 0 })
        
        window.history.pushState({}, '', '/node-manage')
        render(<App />)
        
        expect(await screen.findByText(/Network disconnected/)).toBeInTheDocument()
        
        const retryBtn = screen.getByRole('button', { name: '重试' })
        expect(retryBtn).toBeInTheDocument()
        
        fireEvent.click(retryBtn)
        
        expect(await screen.findByRole('button', { name: '纳管节点' })).toBeInTheDocument()
        expect(fetchNodes).toHaveBeenCalledTimes(2)
    })

    it('renders data-present state when nodes exist with correct table columns and rows', async () => {
        ;(fetchNodes as Mock).mockResolvedValue({ 
            nodes: [{
                id: 'n1',
                name: 'node-1',
                endpoint: 'http://node-1:8080',
                onlineStatus: 'online',
                bindingState: 'BOUND',
                updatedAt: '2026-06-07T12:00:00Z'
            }], 
            total: 1 
        })
        window.history.pushState({}, '', '/node-manage')
        render(<App />)
        expect(await screen.findByRole('heading', { name: '节点列表' })).toBeInTheDocument()
        expect(screen.getByText('共 1 个节点')).toBeInTheDocument()
        expect(screen.getByRole('button', { name: '纳管节点' })).toBeInTheDocument()

        // Check table headers
        expect(screen.getByRole('columnheader', { name: '节点名称' })).toBeInTheDocument()
        expect(screen.getByRole('columnheader', { name: '节点地址' })).toBeInTheDocument()
        expect(screen.getByRole('columnheader', { name: '在线状态' })).toBeInTheDocument()
        expect(screen.getByRole('columnheader', { name: '绑定状态' })).toBeInTheDocument()
        expect(screen.getByRole('columnheader', { name: '最近更新时间' })).toBeInTheDocument()
        expect(screen.getByRole('columnheader', { name: '操作' })).toBeInTheDocument()

        // Check table row data
        expect(screen.getByRole('cell', { name: 'node-1' })).toBeInTheDocument()
        expect(screen.getByRole('cell', { name: 'http://node-1:8080' })).toBeInTheDocument()
        expect(screen.getByRole('cell', { name: '在线' })).toBeInTheDocument() // mapping 'online' -> '在线'
        expect(screen.getByRole('cell', { name: '已绑定' })).toBeInTheDocument() // mapping 'BOUND' -> '已绑定'
        expect(screen.getByText(/2026-06-07/)).toBeInTheDocument() // The formatted date
        expect(screen.getByRole('button', { name: '查看详情' })).toBeInTheDocument()
    })
    
    it('exposes the NodeManage tool card on the home page', async () => {
        render(<App />)
        const heading = screen.getByRole('heading', { name: 'NodeManage' })
        const card = heading.closest('.tool-card')
        expect(card).not.toBeNull()
    })

    describe('Batch Status Polling', () => {
        beforeEach(() => {
            vi.useFakeTimers({ shouldAdvanceTime: true })
        })

        afterEach(() => {
            vi.useRealTimers()
        })
        
        it('requests batch status ONLY for visible node IDs derived from loaded list', async () => {
            const mockNodes = Array.from({ length: 55 }).map((_, i) => ({
                id: `n${i + 1}`,
                name: `node-${i + 1}`,
                onlineStatus: 'offline',
                bindingState: 'UNBOUND'
            }))
            
            ;(fetchNodes as Mock).mockResolvedValue({ 
                nodes: mockNodes,
                total: 55 
            })
            ;(fetchNodeStatusBatch as Mock).mockResolvedValue({
                'n1': { id: 'n1', onlineStatus: 'online', bindingState: 'BOUND', updatedAt: '2026-06-07T13:00:00Z' }
            })
            
            window.history.pushState({}, '', '/node-manage')
            render(<App />)
            
            expect(await screen.findByRole('cell', { name: 'node-1' })).toBeInTheDocument()
            
            vi.useFakeTimers({ shouldAdvanceTime: true })
            await act(async () => {
                vi.advanceTimersByTime(10000)
            })
            vi.useRealTimers()
            
            const expectedPolledIds = mockNodes.slice(0, 50).map(n => n.id)
            expect(fetchNodeStatusBatch).toHaveBeenCalledWith(expectedPolledIds)
            
            await waitFor(() => {
                const n1Row = screen.getByRole('cell', { name: 'node-1' }).closest('tr')!
                expect(n1Row).toHaveTextContent('在线')
                expect(n1Row).toHaveTextContent('已绑定')
                expect(n1Row).toHaveTextContent('2026-06-07')
            })
            
            const n2Row = screen.getByRole('cell', { name: 'node-2' }).closest('tr')!
            expect(n2Row).toHaveTextContent('离线')
        })
        
        it('batch refresh failure preserves existing rows and shows non-fatal warning', async () => {
            ;(fetchNodes as Mock).mockResolvedValue({ 
                nodes: [{ id: 'n1', name: 'node-1', onlineStatus: 'offline' }], 
                total: 1 
            })
            ;(fetchNodeStatusBatch as Mock).mockRejectedValue(new Error('Batch polling failed'))
            
            window.history.pushState({}, '', '/node-manage')
            render(<App />)
            
            expect(await screen.findByRole('cell', { name: 'node-1' })).toBeInTheDocument()
            
            await act(async () => {
                vi.advanceTimersByTime(10000)
            })
            
            expect(fetchNodeStatusBatch).toHaveBeenCalled()
            // Existing row is preserved
            expect(screen.getByRole('cell', { name: 'node-1' })).toBeInTheDocument()
            // Warning is shown
            expect(await screen.findByText(/节点列表状态刷新失败: Batch polling failed/)).toBeInTheDocument()
            
            // On next successful poll, warning should be cleared
            ;(fetchNodeStatusBatch as Mock).mockResolvedValue({
                'n1': { id: 'n1', onlineStatus: 'online' }
            })
            
            await act(async () => {
                vi.advanceTimersByTime(10000)
            })
            
            await waitFor(() => {
                const n1Row = screen.getByRole('cell', { name: 'node-1' }).closest('tr')!
                expect(n1Row).toHaveTextContent('在线')
            })
            
            expect(screen.queryByText(/节点列表状态刷新失败: Batch polling failed/)).not.toBeInTheDocument()
        })
        
        it('suppresses overlapping polls if the previous poll is still in flight', async () => {
            ;(fetchNodes as Mock).mockResolvedValue({ 
                nodes: [{ id: 'n1', name: 'node-1', onlineStatus: 'offline' }], 
                total: 1 
            })
            
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            let resolveFirstPoll: (val: any) => void
            const firstPollPromise = new Promise(resolve => {
                resolveFirstPoll = resolve
            })
            
            ;(fetchNodeStatusBatch as Mock).mockReturnValueOnce(firstPollPromise)
            
            window.history.pushState({}, '', '/node-manage')
            render(<App />)
            
            expect(await screen.findByRole('cell', { name: 'node-1' })).toBeInTheDocument()
            
            // Advance time to trigger the first poll
            await act(async () => {
                vi.advanceTimersByTime(10000)
            })
            
            expect(fetchNodeStatusBatch).toHaveBeenCalledTimes(1)
            
            // Advance time again, but first poll hasn't resolved
            await act(async () => {
                vi.advanceTimersByTime(10000)
            })
            
            // Should NOT have called it a second time due to single-flight protection
            expect(fetchNodeStatusBatch).toHaveBeenCalledTimes(1)
            
            // Resolve the first poll
            await act(async () => {
                resolveFirstPoll({
                    'n1': { id: 'n1', onlineStatus: 'online' }
                })
                await firstPollPromise // Ensure tick
            })
            
            await waitFor(() => {
                const n1Row = screen.getByRole('cell', { name: 'node-1' }).closest('tr')!
                expect(n1Row).toHaveTextContent('在线')
            })
        })
        
        it('does not poll when there are no nodes', async () => {
            ;(fetchNodes as Mock).mockResolvedValue({ nodes: [], total: 0 })
            
            window.history.pushState({}, '', '/node-manage')
            render(<App />)
            
            expect(await screen.findByRole('button', { name: '纳管节点' })).toBeInTheDocument()
            
            await act(async () => {
                vi.advanceTimersByTime(10000)
            })
            
            expect(fetchNodeStatusBatch).not.toHaveBeenCalled()
        })
        
        it('cleans up polling interval on unmount', async () => {
            ;(fetchNodes as Mock).mockResolvedValue({ 
                nodes: [{ id: 'n1', name: 'node-1' }], 
                total: 1 
            })
            
            window.history.pushState({}, '', '/node-manage')
            const { unmount } = render(<App />)
            
            expect(await screen.findByRole('cell', { name: 'node-1' })).toBeInTheDocument()
            
            unmount()
            
            await act(async () => {
                vi.advanceTimersByTime(10000)
            })
            
            expect(fetchNodeStatusBatch).not.toHaveBeenCalled()
        })
    })

    describe('Create Node Onboarding', () => {
        it('empty-state CTA opens create entry', async () => {
            ;(fetchNodes as Mock).mockResolvedValue({ nodes: [], total: 0 })
            window.history.pushState({}, '', '/node-manage')
            render(<App />)
            
            const ctaBtn = await screen.findByRole('button', { name: '纳管节点' })
            fireEvent.click(ctaBtn)
            
            const dialog = screen.getByRole('dialog', { name: '纳管节点' })
            expect(dialog).toBeInTheDocument()
            expect(screen.getByRole('textbox', { name: /节点名称/i })).toBeInTheDocument()
            expect(screen.getByRole('textbox', { name: /节点地址/i })).toBeInTheDocument()
        })

        it('non-empty state shows a top-level 纳管节点 action which opens create entry', async () => {
            ;(fetchNodes as Mock).mockResolvedValue({ 
                nodes: [{ id: 'n1', name: 'node-1' }], 
                total: 1 
            })
            window.history.pushState({}, '', '/node-manage')
            render(<App />)
            
            const createBtn = await screen.findByRole('button', { name: '纳管节点' })
            expect(createBtn).toBeInTheDocument()
            
            fireEvent.click(createBtn)
            expect(screen.getByRole('dialog', { name: '纳管节点' })).toBeInTheDocument()
        })

        it('non-empty state list view allows selecting an existing node to see details', async () => {
            ;(fetchNodes as Mock).mockResolvedValue({ 
                nodes: [{ id: 'n1', name: 'existing-node' }], 
                total: 1 
            })
            ;(fetchNodeDetail as Mock).mockResolvedValue({ id: 'n1', name: 'existing-node' })
            ;(fetchNodeBinding as Mock).mockResolvedValue({ state: 'UNBOUND' })
            ;(fetchLatestInstallTask as Mock).mockResolvedValue(null)
            
            window.history.pushState({}, '', '/node-manage')
            render(<App />)
            
            const detailBtn = await screen.findByRole('button', { name: '查看详情' })
            expect(detailBtn).toBeInTheDocument()
            
            fireEvent.click(detailBtn)
            
            expect(await screen.findByRole('heading', { name: 'existing-node' })).toBeInTheDocument()
            expect(screen.getByRole('button', { name: '安装 agent' })).toBeInTheDocument()
        })

        it('non-empty state list view allows selecting an existing node via row click to see details', async () => {
            ;(fetchNodes as Mock).mockResolvedValue({ 
                nodes: [{ id: 'n1', name: 'existing-node-row-click' }], 
                total: 1 
            })
            ;(fetchNodeDetail as Mock).mockResolvedValue({ id: 'n1', name: 'existing-node-row-click' })
            ;(fetchNodeBinding as Mock).mockResolvedValue({ state: 'UNBOUND' })
            ;(fetchLatestInstallTask as Mock).mockResolvedValue(null)
            
            window.history.pushState({}, '', '/node-manage')
            render(<App />)
            
            const nameCell = await screen.findByRole('cell', { name: 'existing-node-row-click' })
            fireEvent.click(nameCell)
            
            expect(await screen.findByRole('heading', { name: 'existing-node-row-click' })).toBeInTheDocument()
            expect(screen.getByRole('button', { name: '安装 agent' })).toBeInTheDocument()
        })

        it('create validation failure is shown', async () => {
            ;(fetchNodes as Mock).mockResolvedValue({ nodes: [], total: 0 })
            window.history.pushState({}, '', '/node-manage')
            render(<App />)
            
            const ctaBtn = await screen.findByRole('button', { name: '纳管节点' })
            fireEvent.click(ctaBtn)
            
            const submitBtn = screen.getByRole('button', { name: '确认' })
            fireEvent.click(submitBtn)
            
            expect(await screen.findByText(/节点名称不能为空/)).toBeInTheDocument()
            expect(createNode).not.toHaveBeenCalled()
        })

        it('create validation requires endpoint', async () => {
            ;(fetchNodes as Mock).mockResolvedValue({ nodes: [], total: 0 })
            window.history.pushState({}, '', '/node-manage')
            render(<App />)

            fireEvent.click(await screen.findByRole('button', { name: '纳管节点' }))

            const nameInput = screen.getByRole('textbox', { name: /节点名称/i })
            fireEvent.change(nameInput, { target: { value: 'new-node' } })

            fireEvent.click(screen.getByRole('button', { name: '确认' }))

            expect(await screen.findByText(/节点地址不能为空/)).toBeInTheDocument()
            expect(createNode).not.toHaveBeenCalled()
        })

        it('successful create closes the dialog and transitions into selected-node detail state', async () => {
            ;(fetchNodes as Mock)
                .mockResolvedValueOnce({ nodes: [], total: 0 })
                .mockResolvedValueOnce({ nodes: [{ id: 'n2', name: 'new-node' }], total: 1 })
            ;(createNode as Mock).mockResolvedValue({ id: 'n2', name: 'new-node', endpoint: 'http://new-node:8080' })
            ;(fetchNodeDetail as Mock).mockResolvedValue({ id: 'n2', name: 'new-node', endpoint: 'http://new-node:8080' })
            ;(fetchNodeBinding as Mock).mockResolvedValue({ state: 'UNBOUND' })
            ;(fetchLatestInstallTask as Mock).mockResolvedValue(null)
            
            window.history.pushState({}, '', '/node-manage')
            render(<App />)
            
            const ctaBtn = await screen.findByRole('button', { name: '纳管节点' })
            fireEvent.click(ctaBtn)
            
            const input = screen.getByRole('textbox', { name: /节点名称/i })
            fireEvent.change(input, { target: { value: 'new-node' } })
            const endpointInput = screen.getByRole('textbox', { name: /节点地址/i })
            fireEvent.change(endpointInput, { target: { value: 'http://new-node:8080' } })
            
            const submitBtn = screen.getByRole('button', { name: '确认' })
            fireEvent.click(submitBtn)
            
            await waitFor(() => {
                expect(createNode).toHaveBeenCalledWith({ name: 'new-node', endpoint: 'http://new-node:8080' })
            })
            
            await waitFor(() => {
                expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
            })
            
            expect(await screen.findByRole('heading', { name: 'new-node' })).toBeInTheDocument()
            expect(screen.getByRole('button', { name: '安装 agent' })).toBeInTheDocument()
        })

        it('shows error if create API fails', async () => {
            ;(fetchNodes as Mock).mockResolvedValue({ nodes: [], total: 0 })
            ;(createNode as Mock).mockRejectedValue(new Error('Backend validation failed'))
            
            window.history.pushState({}, '', '/node-manage')
            render(<App />)
            
            fireEvent.click(await screen.findByRole('button', { name: '纳管节点' }))
            
            const input = screen.getByRole('textbox', { name: /节点名称/i })
            fireEvent.change(input, { target: { value: 'fail-node' } })
            const endpointInput = screen.getByRole('textbox', { name: /节点地址/i })
            fireEvent.change(endpointInput, { target: { value: 'http://fail-node:8080' } })
            
            const submitBtn = screen.getByRole('button', { name: '确认' })
            fireEvent.click(submitBtn)
            
            expect(await screen.findByText('Backend validation failed')).toBeInTheDocument()
            expect(createNode).toHaveBeenCalledTimes(1)
            expect(submitBtn).not.toBeDisabled()
        })

        it('prevents duplicate submission while create is in-flight', async () => {
            ;(fetchNodes as Mock).mockResolvedValue({ nodes: [], total: 0 })
            
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            let resolveCreate: any
            const createPromise = new Promise(resolve => {
                resolveCreate = resolve
            })
            ;(createNode as Mock).mockReturnValue(createPromise)
            
            window.history.pushState({}, '', '/node-manage')
            render(<App />)
            
            fireEvent.click(await screen.findByRole('button', { name: '纳管节点' }))
            
            const input = screen.getByRole('textbox', { name: /节点名称/i })
            fireEvent.change(input, { target: { value: 'dup-node' } })
            const endpointInput = screen.getByRole('textbox', { name: /节点地址/i })
            fireEvent.change(endpointInput, { target: { value: 'http://dup-node:8080' } })
            
            const submitBtn = screen.getByRole('button', { name: '确认' })
            fireEvent.click(submitBtn)
            fireEvent.click(submitBtn)
            
            expect(createNode).toHaveBeenCalledTimes(1)
            expect(submitBtn).toBeDisabled()
            
            await act(async () => {
                resolveCreate({ id: 'n2', name: 'dup-node' })
                await createPromise
            })
        })

        it('handles create success but refresh failure gracefully', async () => {
            ;(fetchNodes as Mock)
                .mockResolvedValueOnce({ nodes: [], total: 0 })
                .mockRejectedValueOnce(new Error('Refresh failed'))
            ;(createNode as Mock).mockResolvedValue({ id: 'n3', name: 'semi-node' })
            ;(fetchNodeDetail as Mock).mockResolvedValue({ id: 'n3', name: 'semi-node' })
            ;(fetchNodeBinding as Mock).mockResolvedValue({ state: 'UNBOUND' })
            ;(fetchLatestInstallTask as Mock).mockResolvedValue(null)
            
            window.history.pushState({}, '', '/node-manage')
            render(<App />)
            
            fireEvent.click(await screen.findByRole('button', { name: '纳管节点' }))
            
            const input = screen.getByRole('textbox', { name: /节点名称/i })
            fireEvent.change(input, { target: { value: 'semi-node' } })
            const endpointInput = screen.getByRole('textbox', { name: /节点地址/i })
            fireEvent.change(endpointInput, { target: { value: 'http://semi-node:8080' } })
            
            fireEvent.click(screen.getByRole('button', { name: '确认' }))
            
            await waitFor(() => {
                expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
            })
            
            expect(await screen.findByRole('heading', { name: 'semi-node' })).toBeInTheDocument()
            
            fireEvent.click(screen.getByRole('button', { name: '← 返回列表' }))
            expect(await screen.findByText(/节点列表刷新失败: Refresh failed/)).toBeInTheDocument()
        })

        it('clears modal state when closed and reopened', async () => {
            ;(fetchNodes as Mock).mockResolvedValue({ nodes: [], total: 0 })
            
            window.history.pushState({}, '', '/node-manage')
            render(<App />)
            
            fireEvent.click(await screen.findByRole('button', { name: '纳管节点' }))
            
            const input = screen.getByRole('textbox', { name: /节点名称/i }) as HTMLInputElement
            fireEvent.change(input, { target: { value: 'test-node' } })
            expect(input.value).toBe('test-node')
            const endpointInput = screen.getByRole('textbox', { name: /节点地址/i }) as HTMLInputElement
            fireEvent.change(endpointInput, { target: { value: 'http://test-node:8080' } })
            expect(endpointInput.value).toBe('http://test-node:8080')
            
            fireEvent.click(screen.getByRole('button', { name: '取消' }))
            
            fireEvent.click(await screen.findByRole('button', { name: '纳管节点' }))
            
            const reopenedInput = screen.getByRole('textbox', { name: /节点名称/i }) as HTMLInputElement
            expect(reopenedInput.value).toBe('')
            const reopenedEndpointInput = screen.getByRole('textbox', { name: /节点地址/i }) as HTMLInputElement
            expect(reopenedEndpointInput.value).toBe('')
        })
    })

    describe('NodeDetailPanel Isolated', () => {
        const mockOnBack = vi.fn()
        
        beforeEach(() => {
            ;(fetchNodeDetail as Mock).mockResolvedValue({ id: 'n1', name: 'node-1' })
            ;(fetchNodeBinding as Mock).mockResolvedValue({ state: 'UNBOUND' })
            ;(fetchLatestInstallTask as Mock).mockResolvedValue(null)
            ;(installNodeAgent as Mock).mockResolvedValue({ id: 't1', status: 'RUNNING' })
            ;(rebindNode as Mock).mockResolvedValue({})
        })

        it('handles detail load failure gracefully and allows retry', async () => {
            ;(fetchNodeDetail as Mock)
                .mockRejectedValueOnce(new Error('Detail component load error'))
                .mockResolvedValueOnce({ id: 'n1', name: 'node-1' })
            
            render(<NodeDetailPanel nodeId="n1" onBack={mockOnBack} />)
            
            expect(await screen.findByText('Detail component load error')).toBeInTheDocument()
            expect(screen.getByRole('button', { name: '返回列表' })).toBeInTheDocument()
            
            const retryBtn = screen.getByRole('button', { name: '重试' })
            expect(retryBtn).toBeInTheDocument()
            
            fireEvent.click(retryBtn)
            
            expect(await screen.findByText('node-1')).toBeInTheDocument()
            expect(fetchNodeDetail).toHaveBeenCalledTimes(2)
        })

        it('handles install failure correctly', async () => {
            render(<NodeDetailPanel nodeId="n1" onBack={mockOnBack} />)
            expect(await screen.findByText('node-1')).toBeInTheDocument()
            
            fireEvent.click(screen.getByRole('button', { name: '安装 agent' }))
            
            ;(installNodeAgent as Mock).mockRejectedValueOnce(new Error('Failed to create install task'))
            fireEvent.click(screen.getByRole('button', { name: '确认安装' }))
            
            expect(await screen.findByText('Failed to create install task')).toBeInTheDocument()
            expect(fetchLatestInstallTask).toHaveBeenCalledTimes(1)
        })

        it('handles rebind failure correctly', async () => {
            render(<NodeDetailPanel nodeId="n1" onBack={mockOnBack} />)
            expect(await screen.findByText('node-1')).toBeInTheDocument()
            
            fireEvent.click(screen.getByRole('button', { name: '需要重新绑定？' }))
            
            const rebindInput = screen.getByLabelText('目标 Agent ID')
            fireEvent.change(rebindInput, { target: { value: 'agent-bad' } })
            
            ;(rebindNode as Mock).mockRejectedValueOnce(new Error('Agent ID invalid'))
            fireEvent.click(screen.getByRole('button', { name: '确认重绑' }))
            
            expect(await screen.findByText('Agent ID invalid')).toBeInTheDocument()
            expect(fetchNodeBinding).toHaveBeenCalledTimes(1)
        })
    })

    describe('Detail Panel & Actions', () => {
        beforeEach(() => {
            ;(fetchNodes as Mock).mockResolvedValue({ 
                nodes: [{ id: 'n1', name: 'node-1' }], 
                total: 1 
            })
            ;(fetchNodeDetail as Mock).mockResolvedValue({ id: 'n1', name: 'node-1' })
            ;(fetchNodeBinding as Mock).mockResolvedValue({ state: 'UNBOUND' })
            ;(fetchLatestInstallTask as Mock).mockResolvedValue(null)
            ;(installNodeAgent as Mock).mockResolvedValue({ id: 't1', status: 'RUNNING' })
            ;(rebindNode as Mock).mockResolvedValue({})
        })

        const renderAndSelectNode = async () => {
            ;(fetchNodes as Mock)
                .mockResolvedValueOnce({ nodes: [], total: 0 })
                .mockResolvedValueOnce({ nodes: [{ id: 'n1', name: 'node-1' }], total: 1 })
            ;(createNode as Mock).mockResolvedValue({ id: 'n1', name: 'node-1', endpoint: 'http://node-1:8080' })

            window.history.pushState({}, '', '/node-manage')
            render(<App />)
            
            const ctaBtn = await screen.findByRole('button', { name: '纳管节点' })
            fireEvent.click(ctaBtn)
            const input = screen.getByRole('textbox', { name: /节点名称/i })
            fireEvent.change(input, { target: { value: 'node-1' } })
            const endpointInput = screen.getByRole('textbox', { name: /节点地址/i })
            fireEvent.change(endpointInput, { target: { value: 'http://node-1:8080' } })
            fireEvent.click(screen.getByRole('button', { name: '确认' }))
            
            await screen.findByRole('button', { name: '安装 agent' })
        }

        it('detail view shows correct binding language and install CTA', async () => {
            await renderAndSelectNode()
            expect(screen.getByText('未绑定')).toBeInTheDocument()
            expect(screen.getByRole('button', { name: '安装 agent' })).toBeInTheDocument()
        })

        it('install CTA opens install form for selected node and triggers reread', async () => {
            await renderAndSelectNode()
            const installBtn = screen.getByRole('button', { name: '安装 agent' })
            fireEvent.click(installBtn)
            
            const confirmBtn = screen.getByRole('button', { name: '确认安装' })
            expect(confirmBtn).toBeInTheDocument()

            ;(fetchLatestInstallTask as Mock).mockResolvedValueOnce({ id: 't1', status: 'RUNNING' })
            
            fireEvent.click(confirmBtn)
            
            await waitFor(() => {
                expect(installNodeAgent).toHaveBeenCalledWith('n1')
                expect(fetchLatestInstallTask).toHaveBeenCalled()
            })
        })

        it('rebind is present as a secondary repair path and triggers rebind action', async () => {
            await renderAndSelectNode()
            const rebindToggleBtn = screen.getByRole('button', { name: '需要重新绑定？' })
            fireEvent.click(rebindToggleBtn)

            const rebindInput = screen.getByLabelText('目标 Agent ID')
            fireEvent.change(rebindInput, { target: { value: 'agent-123' } })

            const confirmRebindBtn = screen.getByRole('button', { name: '确认重绑' })
            
            ;(fetchNodeBinding as Mock).mockResolvedValueOnce({ state: 'BOUND', agentId: 'agent-123' })
            
            fireEvent.click(confirmRebindBtn)

            await waitFor(() => {
                expect(rebindNode).toHaveBeenCalledWith('n1', { targetAgentId: 'agent-123', reason: '' })
                expect(fetchNodeBinding).toHaveBeenCalled()
            })
            
            expect(await screen.findByText('已绑定')).toBeInTheDocument()
        })
    })
})
