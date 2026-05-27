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

    it('does not crash when object timestamps are invalid', async () => {
        const consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined)
        mockFetchWith({
            success: true,
            data: {
                current_prefix: '',
                marker: null,
                has_more: false,
                prefixes: [],
                items: [
                    {
                        key: '1008.png',
                        name: '1008.png',
                        is_directory: false,
                        size: 17196,
                        mime_type: 'image/png',
                        updated_at: '17440973018030518',
                        hash: 'hash-1008',
                    },
                ],
            },
        })

        render(<ObjectStoragePage />)

        expect(await screen.findByText('1008.png')).toBeInTheDocument()
        expect(screen.getByText('-')).toBeInTheDocument()
        expect(consoleError).not.toHaveBeenCalled()
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

    it('creates a directory and refreshes the current listing', async () => {
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
            .mockResolvedValueOnce(
                jsonResponse({
                    success: true,
                    data: { key: 'reports/', name: 'reports', is_directory: true },
                }),
            )
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
        vi.stubGlobal('fetch', fetchMock)
        vi.stubGlobal('prompt', vi.fn(() => 'reports'))

        render(<ObjectStoragePage />)

        fireEvent.click(await screen.findByRole('button', { name: '新建目录' }))

        await waitFor(() => {
            expect(fetchMock).toHaveBeenCalledWith('/api/object-storage/directories', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ name: 'reports' }),
            })
        })
        expect(await screen.findByText('已创建目录 reports/')).toBeInTheDocument()
        expect(fetchMock).toHaveBeenLastCalledWith('/api/object-storage/objects')
    })

    it('uploads a selected file with an upload token and refreshes the listing', async () => {
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
            .mockResolvedValueOnce(
                jsonResponse({
                    success: true,
                    data: {
                        upload_token: 'upload-token',
                        object_key: 'demo.txt',
                        upload_key: 'team-a/demo.txt',
                        upload_url: 'https://upload.example.com',
                        expires_at: '2026-05-25T00:00:00Z',
                        bucket: 'test-bucket',
                    },
                }),
            )
            .mockResolvedValueOnce(new Response(JSON.stringify({ key: 'demo.txt', hash: 'hash-demo' })))
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
        vi.stubGlobal('fetch', fetchMock)

        render(<ObjectStoragePage />)

        const file = new File(['hello'], 'demo.txt', { type: 'text/plain' })
        fireEvent.change(await screen.findByLabelText('选择上传文件'), {
            target: { files: [file] },
        })

        await waitFor(() => {
            expect(fetchMock).toHaveBeenCalledWith('/api/object-storage/upload-token', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ filename: 'demo.txt' }),
            })
        })

        const uploadCall = fetchMock.mock.calls.find(([url]) => url === 'https://upload.example.com')
        expect(uploadCall).toBeDefined()
        const uploadBody = uploadCall?.[1]?.body as FormData
        expect(uploadBody.get('token')).toBe('upload-token')
        expect(uploadBody.get('key')).toBe('team-a/demo.txt')
        expect(uploadBody.get('file')).toBe(file)
        expect(await screen.findByText('已上传 demo.txt')).toBeInTheDocument()
        expect(fetchMock).toHaveBeenLastCalledWith('/api/object-storage/objects')
    })

    it('requests the download url with the full object_key after upload succeeds', async () => {
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
            .mockResolvedValueOnce(
                jsonResponse({
                    success: true,
                    data: {
                        upload_token: 'upload-token',
                        object_key: 'images/demo.txt',
                        upload_key: 'team-a/images/demo.txt',
                        upload_url: 'https://upload.example.com',
                        expires_at: '2026-05-27T12:00:00Z',
                        bucket: 'test-bucket',
                    },
                }),
            )
            .mockResolvedValueOnce(new Response(JSON.stringify({ key: 'images/demo.txt', hash: 'hash-demo' })))
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
        vi.stubGlobal('fetch', fetchMock)

        render(<ObjectStoragePage />)

        const file = new File(['hello'], 'demo.txt', { type: 'text/plain' })
        fireEvent.change(await screen.findByLabelText('选择上传文件'), {
            target: { files: [file] },
        })

        await waitFor(() => {
            expect(fetchMock).toHaveBeenCalledWith(
                '/api/object-storage/download-url?key=images%2Fdemo.txt',
            )
        })
    })

    it('does not restore a cleared upload result when refresh happens during link lookup', async () => {
        const downloadUrlRequest = deferredResponse()
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
            .mockResolvedValueOnce(
                jsonResponse({
                    success: true,
                    data: {
                        upload_token: 'upload-token',
                        object_key: 'demo.txt',
                        upload_key: 'team-a/demo.txt',
                        upload_url: 'https://upload.example.com',
                        expires_at: '2026-05-27T12:00:00Z',
                        bucket: 'test-bucket',
                    },
                }),
            )
            .mockResolvedValueOnce(new Response(JSON.stringify({ key: 'demo.txt', hash: 'hash-demo' })))
            .mockReturnValueOnce(downloadUrlRequest.promise)
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
            .mockImplementation(() => Promise.reject(new Error('unexpected fetch')))
        vi.stubGlobal('fetch', fetchMock)

        render(<ObjectStoragePage />)

        const file = new File(['hello'], 'demo.txt', { type: 'text/plain' })
        fireEvent.change(await screen.findByLabelText('选择上传文件'), {
            target: { files: [file] },
        })

        await waitFor(() => {
            expect(fetchMock).toHaveBeenCalledWith('/api/object-storage/download-url?key=demo.txt')
        })

        fireEvent.click(screen.getByRole('button', { name: '刷新' }))

        downloadUrlRequest.resolve(
            jsonResponse({
                success: true,
                data: {
                    key: 'demo.txt',
                    download_url: 'https://cdn.example.com/demo.txt',
                    expires_at: null,
                },
            }),
        )

        await waitFor(() => {
            expect(fetchMock).toHaveBeenCalledTimes(5)
        })
        expect(screen.queryByRole('region', { name: '最近上传结果' })).not.toBeInTheDocument()
    })

    it('keeps upload success visible when download url lookup fails', async () => {
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
            .mockResolvedValueOnce(
                jsonResponse({
                    success: true,
                    data: {
                        upload_token: 'upload-token',
                        object_key: 'images/demo.txt',
                        upload_key: 'team-a/images/demo.txt',
                        upload_url: 'https://upload.example.com',
                        expires_at: '2026-05-27T12:00:00Z',
                        bucket: 'test-bucket',
                    },
                }),
            )
            .mockResolvedValueOnce(new Response(JSON.stringify({ key: 'images/demo.txt', hash: 'hash-demo' })))
            .mockResolvedValueOnce(jsonResponse({ success: false, error: 'download lookup failed' }))
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
        vi.stubGlobal('fetch', fetchMock)

        render(<ObjectStoragePage />)

        const file = new File(['hello'], 'demo.txt', { type: 'text/plain' })
        fireEvent.change(await screen.findByLabelText('选择上传文件'), {
            target: { files: [file] },
        })

        expect(await screen.findByText('已上传 images/demo.txt')).toBeInTheDocument()
        expect(screen.getByText('下载链接获取失败：download lookup failed')).toBeInTheDocument()
    })

    it('renders a dedicated recent upload result section with link and expiry', async () => {
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
            .mockResolvedValueOnce(
                jsonResponse({
                    success: true,
                    data: {
                        upload_token: 'upload-token',
                        object_key: 'reports/demo.txt',
                        upload_key: 'team-a/reports/demo.txt',
                        upload_url: 'https://upload.example.com',
                        expires_at: '2026-05-27T12:00:00Z',
                        bucket: 'test-bucket',
                    },
                }),
            )
            .mockResolvedValueOnce(new Response(JSON.stringify({ key: 'reports/demo.txt', hash: 'hash-demo' })))
            .mockResolvedValueOnce(
                jsonResponse({
                    success: true,
                    data: {
                        key: 'reports/demo.txt',
                        download_url: 'https://cdn.example.com/reports/demo.txt',
                        expires_at: '2026-05-27T13:00:00Z',
                    },
                }),
            )
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
        vi.stubGlobal('fetch', fetchMock)

        render(<ObjectStoragePage />)

        const file = new File(['hello'], 'demo.txt', { type: 'text/plain' })
        fireEvent.change(await screen.findByLabelText('选择上传文件'), {
            target: { files: [file] },
        })

        expect(await screen.findByRole('region', { name: '最近上传结果' })).toBeInTheDocument()
        expect(screen.getByText('已上传 reports/demo.txt')).toBeInTheDocument()
        expect(screen.getByText('https://cdn.example.com/reports/demo.txt')).toBeInTheDocument()
        expect(screen.getByText(/链接有效期至/)).toBeInTheDocument()
        expect(screen.getByRole('button', { name: '复制链接' })).toBeInTheDocument()
        expect(screen.getByRole('link', { name: '打开链接' })).toHaveAttribute(
            'href',
            'https://cdn.example.com/reports/demo.txt',
        )
    })

    it('treats an unsafe download url as unavailable instead of rendering a link', async () => {
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
            .mockResolvedValueOnce(
                jsonResponse({
                    success: true,
                    data: {
                        upload_token: 'upload-token',
                        object_key: 'reports/demo.txt',
                        upload_key: 'team-a/reports/demo.txt',
                        upload_url: 'https://upload.example.com',
                        expires_at: '2026-05-27T12:00:00Z',
                        bucket: 'test-bucket',
                    },
                }),
            )
            .mockResolvedValueOnce(new Response(JSON.stringify({ key: 'reports/demo.txt', hash: 'hash-demo' })))
            .mockResolvedValueOnce(
                jsonResponse({
                    success: true,
                    data: {
                        key: 'reports/demo.txt',
                        download_url: 'javascript:alert(1)',
                        expires_at: null,
                    },
                }),
            )
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
        vi.stubGlobal('fetch', fetchMock)

        render(<ObjectStoragePage />)

        const file = new File(['hello'], 'demo.txt', { type: 'text/plain' })
        fireEvent.change(await screen.findByLabelText('选择上传文件'), {
            target: { files: [file] },
        })

        expect(await screen.findByText('已上传 reports/demo.txt')).toBeInTheDocument()
        expect(screen.getByText('下载链接不可用：返回了不安全的链接')).toBeInTheDocument()
        expect(screen.queryByRole('button', { name: '复制链接' })).not.toBeInTheDocument()
        expect(screen.queryByRole('link', { name: '打开链接' })).not.toBeInTheDocument()
    })

    it('clears the recent upload result when the user manually refreshes', async () => {
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
            .mockResolvedValueOnce(
                jsonResponse({
                    success: true,
                    data: {
                        upload_token: 'upload-token',
                        object_key: 'images/demo.txt',
                        upload_key: 'team-a/images/demo.txt',
                        upload_url: 'https://upload.example.com',
                        expires_at: '2026-05-27T12:00:00Z',
                        bucket: 'test-bucket',
                    },
                }),
            )
            .mockResolvedValueOnce(new Response(JSON.stringify({ key: 'images/demo.txt', hash: 'hash-demo' })))
            .mockResolvedValueOnce(
                jsonResponse({
                    success: true,
                    data: {
                        key: 'images/demo.txt',
                        download_url: 'https://cdn.example.com/images/demo.txt',
                        expires_at: null,
                    },
                }),
            )
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
        vi.stubGlobal('fetch', fetchMock)

        render(<ObjectStoragePage />)

        const file = new File(['hello'], 'demo.txt', { type: 'text/plain' })
        fireEvent.change(await screen.findByLabelText('选择上传文件'), {
            target: { files: [file] },
        })

        expect(await screen.findByRole('region', { name: '最近上传结果' })).toBeInTheDocument()

        fireEvent.click(screen.getByRole('button', { name: '刷新' }))

        await waitFor(() => {
            expect(screen.queryByRole('region', { name: '最近上传结果' })).not.toBeInTheDocument()
        })
    })

    it('clears the recent upload result when navigating to another directory', async () => {
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(populatedListResponse ? jsonResponse(populatedListResponse) : jsonResponse(emptyListResponse))
            .mockResolvedValueOnce(
                jsonResponse({
                    success: true,
                    data: {
                        upload_token: 'upload-token',
                        object_key: 'images/demo.txt',
                        upload_key: 'team-a/images/demo.txt',
                        upload_url: 'https://upload.example.com',
                        expires_at: '2026-05-27T12:00:00Z',
                        bucket: 'test-bucket',
                    },
                }),
            )
            .mockResolvedValueOnce(new Response(JSON.stringify({ key: 'images/demo.txt', hash: 'hash-demo' })))
            .mockResolvedValueOnce(
                jsonResponse({
                    success: true,
                    data: {
                        key: 'images/demo.txt',
                        download_url: 'https://cdn.example.com/images/demo.txt',
                        expires_at: null,
                    },
                }),
            )
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

        const file = new File(['hello'], 'demo.txt', { type: 'text/plain' })
        fireEvent.change(await screen.findByLabelText('选择上传文件'), {
            target: { files: [file] },
        })

        expect(await screen.findByRole('region', { name: '最近上传结果' })).toBeInTheDocument()

        fireEvent.click(await screen.findByRole('button', { name: '进入 images 目录' }))

        await waitFor(() => {
            expect(screen.queryByRole('region', { name: '最近上传结果' })).not.toBeInTheDocument()
        })
    })

    it('falls back to a manual-copy prompt when clipboard copy fails', async () => {
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
            .mockResolvedValueOnce(
                jsonResponse({
                    success: true,
                    data: {
                        upload_token: 'upload-token',
                        object_key: 'images/demo.txt',
                        upload_key: 'team-a/images/demo.txt',
                        upload_url: 'https://upload.example.com',
                        expires_at: '2026-05-27T12:00:00Z',
                        bucket: 'test-bucket',
                    },
                }),
            )
            .mockResolvedValueOnce(new Response(JSON.stringify({ key: 'images/demo.txt', hash: 'hash-demo' })))
            .mockResolvedValueOnce(
                jsonResponse({
                    success: true,
                    data: {
                        key: 'images/demo.txt',
                        download_url: 'https://cdn.example.com/images/demo.txt',
                        expires_at: null,
                    },
                }),
            )
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
        vi.stubGlobal('fetch', fetchMock)

        const writeText = vi.fn().mockRejectedValue(new Error('clipboard unavailable'))
        Object.assign(navigator, { clipboard: { writeText } })
        vi.stubGlobal('prompt', vi.fn(() => null))

        render(<ObjectStoragePage />)

        const file = new File(['hello'], 'demo.txt', { type: 'text/plain' })
        fireEvent.change(await screen.findByLabelText('选择上传文件'), {
            target: { files: [file] },
        })

        fireEvent.click(await screen.findByRole('button', { name: '复制链接' }))

        await waitFor(() => {
            expect(window.prompt).toHaveBeenCalledWith(
                '请手动复制下载链接',
                'https://cdn.example.com/images/demo.txt',
            )
        })
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
