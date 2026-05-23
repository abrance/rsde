import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import ObjectStoragePage from './ObjectStoragePage'

const emptyListResponse = {
    success: true,
    data: {
        current_prefix: '',
        marker: null,
        has_more: false,
        prefixes: [],
        items: [],
    },
}

const populatedListResponse = {
    success: true,
    data: {
        current_prefix: '',
        marker: null,
        has_more: false,
        prefixes: [{ key: 'images/', name: 'images', is_directory: true }],
        items: [
            {
                key: 'demo.png',
                name: 'demo.png',
                is_directory: false,
                size: 42,
                mime_type: 'image/png',
                updated_at: '2026-05-22T00:00:00Z',
                hash: 'hash-demo',
            },
        ],
    },
}

function jsonResponse(body: unknown): Response {
    return new Response(JSON.stringify(body), {
        headers: { 'Content-Type': 'application/json' },
    })
}

function mockFetchWith(body: unknown) {
    const fetchMock = vi.fn(() => Promise.resolve(jsonResponse(body)))
    vi.stubGlobal('fetch', fetchMock)
    return fetchMock
}

function deferredResponse() {
    let resolve: (response: Response) => void = () => undefined
    const promise = new Promise<Response>((resolver) => {
        resolve = resolver
    })

    return { promise, resolve }
}

describe('ObjectStoragePage', () => {
    beforeEach(() => {
        mockFetchWith(emptyListResponse)
    })

    afterEach(() => {
        vi.unstubAllGlobals()
        vi.restoreAllMocks()
    })

    it('renders the page title and action placeholders', async () => {
        render(<ObjectStoragePage />)

        expect(screen.getByRole('heading', { name: '对象存储文件管理' })).toBeInTheDocument()
        expect(screen.getByRole('button', { name: '刷新' })).toBeInTheDocument()
        expect(screen.getByRole('button', { name: '上传文件' })).toBeInTheDocument()
        expect(screen.getByRole('button', { name: '新建目录' })).toBeInTheDocument()
        await screen.findByRole('heading', { name: '当前目录暂无文件' })
    })

    it('renders the empty state', async () => {
        render(<ObjectStoragePage />)

        expect(await screen.findByRole('heading', { name: '当前目录暂无文件' })).toBeInTheDocument()
        expect(screen.getByText('选择上传文件或新建目录，开始管理对象存储内容。')).toBeInTheDocument()
    })

    it('renders the breadcrumb placeholder', async () => {
        render(<ObjectStoragePage />)

        expect(screen.getByRole('navigation', { name: '当前路径' })).toBeInTheDocument()
        expect(screen.getByText('存储桶')).toBeInTheDocument()
        expect(screen.getByText('根目录')).toBeInTheDocument()
        await screen.findByRole('heading', { name: '当前目录暂无文件' })
    })

    it('shows loading while fetching objects', () => {
        const pendingFetch = new Promise<Response>(() => undefined)
        vi.stubGlobal('fetch', vi.fn(() => pendingFetch))

        render(<ObjectStoragePage />)

        expect(screen.getByText('加载对象列表中...')).toBeInTheDocument()
    })

    it('renders directories and files from the object list', async () => {
        mockFetchWith(populatedListResponse)

        render(<ObjectStoragePage />)

        expect(await screen.findByText('images')).toBeInTheDocument()
        expect(screen.getByText('demo.png')).toBeInTheDocument()
        expect(screen.getByText('42 B')).toBeInTheDocument()
    })

    it('navigates into a directory and updates breadcrumbs', async () => {
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(populatedListResponse))
            .mockResolvedValueOnce(
                jsonResponse({
                    success: true,
                    data: {
                        current_prefix: 'images/',
                        marker: null,
                        has_more: false,
                        prefixes: [],
                        items: [],
                    },
                }),
            )
        vi.stubGlobal('fetch', fetchMock)

        render(<ObjectStoragePage />)

        fireEvent.click(await screen.findByRole('button', { name: '进入 images 目录' }))

        await waitFor(() => {
            expect(fetchMock).toHaveBeenLastCalledWith('/api/object-storage/objects?prefix=images%2F')
        })
        expect(screen.getByRole('button', { name: '返回根目录' })).toBeInTheDocument()
        expect(screen.getByText('images')).toBeInTheDocument()
    })

    it('opens object detail panel', async () => {
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(populatedListResponse))
            .mockResolvedValueOnce(
                jsonResponse({
                    success: true,
                    data: {
                        key: 'demo.png',
                        name: 'demo.png',
                        is_directory: false,
                        size: 42,
                        hash: 'hash-demo',
                        mime_type: 'image/png',
                        updated_at: '2026-05-22T00:00:00Z',
                        download_url: 'https://cdn.example.com/demo.png',
                        storage_class: '0',
                    },
                }),
            )
        vi.stubGlobal('fetch', fetchMock)

        render(<ObjectStoragePage />)

        fireEvent.click(await screen.findByRole('button', { name: '查看 demo.png 详情' }))

        expect(await screen.findByRole('heading', { name: '对象详情' })).toBeInTheDocument()
        expect(screen.getByText('hash-demo')).toBeInTheDocument()
    })

    it('clears stale object detail when the next detail request fails', async () => {
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(populatedListResponse))
            .mockResolvedValueOnce(
                jsonResponse({
                    success: true,
                    data: {
                        key: 'demo.png',
                        name: 'demo.png',
                        is_directory: false,
                        size: 42,
                        hash: 'hash-demo',
                        mime_type: 'image/png',
                        updated_at: '2026-05-22T00:00:00Z',
                        download_url: 'https://cdn.example.com/demo.png',
                        storage_class: '0',
                    },
                }),
            )
            .mockResolvedValueOnce(jsonResponse({ success: false, error: 'detail lookup failed' }))
        vi.stubGlobal('fetch', fetchMock)

        render(<ObjectStoragePage />)

        fireEvent.click(await screen.findByRole('button', { name: '查看 demo.png 详情' }))
        expect(await screen.findByText('hash-demo')).toBeInTheDocument()

        fireEvent.click(screen.getByRole('button', { name: '查看 demo.png 详情' }))

        expect(await screen.findByText('detail lookup failed')).toBeInTheDocument()
        expect(screen.queryByRole('heading', { name: '对象详情' })).not.toBeInTheDocument()
        expect(screen.queryByText('hash-demo')).not.toBeInTheDocument()
    })

    it('keeps the newest object detail when detail requests finish out of order', async () => {
        const slowDetailRequest = deferredResponse()
        const fastDetailRequest = deferredResponse()
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(populatedListResponse))
            .mockReturnValueOnce(slowDetailRequest.promise)
            .mockReturnValueOnce(fastDetailRequest.promise)
        vi.stubGlobal('fetch', fetchMock)

        render(<ObjectStoragePage />)

        const detailButton = await screen.findByRole('button', { name: '查看 demo.png 详情' })
        fireEvent.click(detailButton)
        fireEvent.click(detailButton)

        fastDetailRequest.resolve(
            jsonResponse({
                success: true,
                data: {
                    key: 'demo.png',
                    name: 'demo.png',
                    is_directory: false,
                    size: 42,
                    hash: 'hash-new',
                    mime_type: 'image/png',
                    updated_at: '2026-05-22T00:00:00Z',
                    download_url: 'https://cdn.example.com/demo.png',
                    storage_class: '0',
                },
            }),
        )

        expect(await screen.findByText('hash-new')).toBeInTheDocument()

        slowDetailRequest.resolve(
            jsonResponse({
                success: true,
                data: {
                    key: 'demo.png',
                    name: 'demo.png',
                    is_directory: false,
                    size: 42,
                    hash: 'hash-old',
                    mime_type: 'image/png',
                    updated_at: '2026-05-22T00:00:00Z',
                    download_url: 'https://cdn.example.com/demo.png',
                    storage_class: '0',
                },
            }),
        )

        expect(screen.getByText('hash-new')).toBeInTheDocument()
        expect(screen.queryByText('hash-old')).not.toBeInTheDocument()
    })

    it('confirms before deleting a single object', async () => {
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(populatedListResponse))
            .mockResolvedValueOnce(jsonResponse({ success: true, data: { deleted_key: 'demo.png' } }))
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
        vi.stubGlobal('fetch', fetchMock)
        vi.stubGlobal('confirm', vi.fn(() => true))

        render(<ObjectStoragePage />)

        fireEvent.click(await screen.findByRole('button', { name: '删除 demo.png' }))

        await waitFor(() => {
            expect(fetchMock).toHaveBeenCalledWith('/api/object-storage/objects/delete', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ key: 'demo.png' }),
            })
        })
    })

    it('deletes selected objects and displays partial failures', async () => {
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(populatedListResponse))
            .mockResolvedValueOnce(
                jsonResponse({
                    success: true,
                    data: {
                        deleted_keys: [],
                        failed: [{ key: 'demo.png', error: 'failed to delete object' }],
                    },
                }),
            )
            .mockResolvedValueOnce(jsonResponse(populatedListResponse))
        vi.stubGlobal('fetch', fetchMock)
        vi.stubGlobal('confirm', vi.fn(() => true))

        render(<ObjectStoragePage />)

        const row = await screen.findByRole('row', { name: /demo.png/ })
        fireEvent.click(within(row).getByRole('checkbox', { name: '选择 demo.png' }))
        fireEvent.click(screen.getByRole('button', { name: '删除已选' }))

        expect(await screen.findByText(/demo.png: failed to delete object/)).toBeInTheDocument()
    })
})
