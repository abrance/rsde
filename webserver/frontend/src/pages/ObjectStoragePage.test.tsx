import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
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

type MockUploadProgressEvent = Pick<ProgressEvent<EventTarget>, 'lengthComputable' | 'loaded' | 'total'>

class MockXMLHttpRequest {
    static instances: MockXMLHttpRequest[] = []

    method: string | null = null
    url: string | null = null
    body: Document | XMLHttpRequestBodyInit | null = null
    status = 0
    onload: (() => void) | null = null
    onerror: (() => void) | null = null
    upload = {
        onprogress: null as ((event: MockUploadProgressEvent) => void) | null,
    }

    open(method: string, url: string) {
        this.method = method
        this.url = url
    }

    send(body?: Document | XMLHttpRequestBodyInit | null) {
        this.body = body ?? null
        MockXMLHttpRequest.instances.push(this)
    }

    emitProgress(loaded: number, total: number) {
        this.upload.onprogress?.({
            lengthComputable: true,
            loaded,
            total,
        })
    }

    respond(status: number) {
        this.status = status
        this.onload?.()
    }

    failNetwork() {
        this.onerror?.()
    }
}

function installMockXMLHttpRequest() {
    MockXMLHttpRequest.instances = []
    vi.stubGlobal('XMLHttpRequest', MockXMLHttpRequest)

    return {
        latest() {
            const instance =
                MockXMLHttpRequest.instances[MockXMLHttpRequest.instances.length - 1]
            expect(instance).toBeDefined()
            return instance as MockXMLHttpRequest
        },
    }
}

async function waitForLatestXMLHttpRequest(xhr: ReturnType<typeof installMockXMLHttpRequest>) {
    await waitFor(() => {
        expect(MockXMLHttpRequest.instances.length).toBeGreaterThan(0)
    })

    return xhr.latest()
}

function createUploadTokenResponse(objectKey = 'demo.txt') {
    return jsonResponse({
        success: true,
        data: {
            upload_token: 'upload-token',
            object_key: objectKey,
            upload_key: `team-a/${objectKey}`,
            upload_url: 'https://upload.example.com',
            expires_at: '2026-05-27T12:00:00Z',
            bucket: 'test-bucket',
        },
    })
}

function createDownloadUrlResponse(objectKey = 'demo.txt', downloadUrl?: string | null) {
    return jsonResponse({
        success: true,
        data: {
            key: objectKey,
            download_url: downloadUrl ?? `https://cdn.example.com/${objectKey}`,
            expires_at: null,
        },
    })
}

function createMockLargeFile(name: string, reportedSize: number, chunkSize = 4): File {
    return {
        name,
        size: reportedSize,
        type: 'application/octet-stream',
        slice(start = 0, end = reportedSize) {
            const length = Math.max(0, Math.min(chunkSize, end - start))
            return {
                arrayBuffer: async () => new Uint8Array(length || 1).buffer,
            } as Blob
        },
    } as File
}

function createUploadSessionResponse(objectKey = 'large-demo.bin', partSizeBytes = 4, partCount = 3) {
    return jsonResponse({
        success: true,
        data: {
            session_id: 'session-1',
            object_key: objectKey,
            upload_key: `team-a/${objectKey}`,
            part_size_bytes: partSizeBytes,
            part_count: partCount,
            expires_at: '2026-05-27T12:00:00Z',
        },
    })
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
        const xhr = installMockXMLHttpRequest()
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
            .mockResolvedValueOnce(createUploadTokenResponse('demo.txt'))
            .mockResolvedValueOnce(createDownloadUrlResponse('demo.txt'))
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

        const uploadBody = (await waitForLatestXMLHttpRequest(xhr)).body as FormData
        expect(uploadBody.get('token')).toBe('upload-token')
        expect(uploadBody.get('key')).toBe('team-a/demo.txt')
        expect(uploadBody.get('file')).toBe(file)

        act(() => {
            MockXMLHttpRequest.instances[MockXMLHttpRequest.instances.length - 1]?.respond(200)
        })

        expect(await screen.findByText('已上传 demo.txt')).toBeInTheDocument()
        expect(fetchMock).toHaveBeenLastCalledWith('/api/object-storage/objects')
    })

    it('shows visible upload progress while the direct upload is in flight', async () => {
        const xhr = installMockXMLHttpRequest()
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
            .mockResolvedValueOnce(
                jsonResponse({
                    success: true,
                    data: {
                        key: 'demo.txt',
                        download_url: 'https://cdn.example.com/demo.txt',
                        expires_at: null,
                    },
                }),
            )
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
        vi.stubGlobal('fetch', fetchMock)

        render(<ObjectStoragePage />)

        const file = new File(['hello world'], 'demo.txt', { type: 'text/plain' })
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

        const uploadRequest = xhr.latest()
        expect(uploadRequest.method).toBe('POST')
        expect(uploadRequest.url).toBe('https://upload.example.com')
        const uploadBody = uploadRequest.body as FormData
        expect(uploadBody.get('token')).toBe('upload-token')
        expect(uploadBody.get('key')).toBe('team-a/demo.txt')
        expect(uploadBody.get('file')).toBe(file)

        act(() => {
            uploadRequest.emitProgress(5, 10)
        })

        expect(await screen.findByRole('region', { name: '上传进度' })).toBeInTheDocument()
        expect(screen.getByText('50%')).toBeInTheDocument()

        act(() => {
            uploadRequest.respond(200)
        })

        await waitFor(() => {
            expect(screen.queryByRole('region', { name: '上传进度' })).not.toBeInTheDocument()
        })
        expect(await screen.findByText('已上传 demo.txt')).toBeInTheDocument()
    })

    it('requests the download url with the full object_key after upload succeeds', async () => {
        const xhr = installMockXMLHttpRequest()
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
            .mockResolvedValueOnce(createUploadTokenResponse('images/demo.txt'))
            .mockResolvedValueOnce(createDownloadUrlResponse('images/demo.txt'))
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
        vi.stubGlobal('fetch', fetchMock)

        render(<ObjectStoragePage />)

        const file = new File(['hello'], 'demo.txt', { type: 'text/plain' })
        fireEvent.change(await screen.findByLabelText('选择上传文件'), {
            target: { files: [file] },
        })

        await waitForLatestXMLHttpRequest(xhr)

        act(() => {
            MockXMLHttpRequest.instances[MockXMLHttpRequest.instances.length - 1]?.respond(200)
        })

        await waitFor(() => {
            expect(fetchMock).toHaveBeenCalledWith(
                '/api/object-storage/download-url?key=images%2Fdemo.txt',
            )
        })
    })

    it('uses upload sessions instead of upload-token for large files', async () => {
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
            .mockResolvedValueOnce(createUploadSessionResponse('large-demo.bin', 5 * 1024 * 1024, 2))
            .mockResolvedValueOnce(
                jsonResponse({ success: true, data: { session_id: 'session-1', part_number: 1, uploaded_parts: [1] } }),
            )
            .mockResolvedValueOnce(
                jsonResponse({ success: true, data: { session_id: 'session-1', part_number: 2, uploaded_parts: [1, 2] } }),
            )
            .mockResolvedValueOnce(
                jsonResponse({ success: true, data: { session_id: 'session-1', object_key: 'large-demo.bin' } }),
            )
            .mockResolvedValueOnce(createDownloadUrlResponse('large-demo.bin'))
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
        vi.stubGlobal('fetch', fetchMock)

        render(<ObjectStoragePage />)

        const file = createMockLargeFile('large-demo.bin', 6 * 1024 * 1024, 4)
        fireEvent.change(await screen.findByLabelText('选择上传文件'), {
            target: { files: [file] },
        })

        await waitFor(() => {
            expect(fetchMock).toHaveBeenCalledWith('/api/object-storage/upload-sessions', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    filename: 'large-demo.bin',
                    file_size_bytes: file.size,
                }),
            })
        })

        expect(fetchMock).not.toHaveBeenCalledWith('/api/object-storage/upload-token', expect.anything())
        await waitFor(() => {
            expect(fetchMock).toHaveBeenCalledWith('/api/object-storage/download-url?key=large-demo.bin')
        })
        expect(await screen.findByText('已上传 large-demo.bin')).toBeInTheDocument()
    })

    it('shows aggregated upload progress for large file session uploads', async () => {
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
            .mockResolvedValueOnce(createUploadSessionResponse('large-demo.bin', 4, 3))
            .mockResolvedValueOnce(
                jsonResponse({ success: true, data: { session_id: 'session-1', part_number: 1, uploaded_parts: [1] } }),
            )
            .mockResolvedValueOnce(
                jsonResponse({ success: true, data: { session_id: 'session-1', part_number: 2, uploaded_parts: [1, 2] } }),
            )
            .mockResolvedValueOnce(
                jsonResponse({ success: true, data: { session_id: 'session-1', part_number: 3, uploaded_parts: [1, 2, 3] } }),
            )
            .mockResolvedValueOnce(
                jsonResponse({ success: true, data: { session_id: 'session-1', object_key: 'large-demo.bin' } }),
            )
            .mockResolvedValueOnce(createDownloadUrlResponse('large-demo.bin'))
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
        vi.stubGlobal('fetch', fetchMock)

        render(<ObjectStoragePage />)

        const file = createMockLargeFile('large-demo.bin', 6 * 1024 * 1024, 4)
        fireEvent.change(await screen.findByLabelText('选择上传文件'), {
            target: { files: [file] },
        })

        await waitFor(() => {
            expect(fetchMock).toHaveBeenCalledWith('/api/object-storage/upload-sessions/session-1/parts/1', expect.objectContaining({ method: 'PUT' }))
            expect(fetchMock).toHaveBeenCalledWith('/api/object-storage/upload-sessions/session-1/parts/2', expect.objectContaining({ method: 'PUT' }))
            expect(fetchMock).toHaveBeenCalledWith('/api/object-storage/upload-sessions/session-1/parts/3', expect.objectContaining({ method: 'PUT' }))
            expect(fetchMock).toHaveBeenCalledWith('/api/object-storage/upload-sessions/session-1/complete', {
                method: 'POST',
            })
        })
        expect(await screen.findByText('已上传 large-demo.bin')).toBeInTheDocument()
        await waitFor(() => {
            expect(screen.queryByRole('region', { name: '上传进度' })).not.toBeInTheDocument()
        })
    })

    it('clears in-flight upload progress when the user refreshes', async () => {
        const xhr = installMockXMLHttpRequest()
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
            .mockResolvedValueOnce(createUploadTokenResponse('demo.txt'))
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
        vi.stubGlobal('fetch', fetchMock)

        render(<ObjectStoragePage />)

        const file = new File(['hello world'], 'demo.txt', { type: 'text/plain' })
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

        act(() => {
            xhr.latest().emitProgress(5, 10)
        })

        expect(await screen.findByRole('region', { name: '上传进度' })).toBeInTheDocument()

        fireEvent.click(screen.getByRole('button', { name: '刷新' }))

        await waitFor(() => {
            expect(screen.queryByRole('region', { name: '上传进度' })).not.toBeInTheDocument()
        })
    })

    it('clears upload progress and shows an error when the direct upload fails', async () => {
        const xhr = installMockXMLHttpRequest()
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
            .mockResolvedValueOnce(createUploadTokenResponse('demo.txt'))
        vi.stubGlobal('fetch', fetchMock)

        render(<ObjectStoragePage />)

        const file = new File(['hello world'], 'demo.txt', { type: 'text/plain' })
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

        act(() => {
            xhr.latest().emitProgress(5, 10)
        })
        expect(await screen.findByRole('region', { name: '上传进度' })).toBeInTheDocument()

        act(() => {
            xhr.latest().respond(500)
        })

        expect(await screen.findByText('上传失败：500')).toBeInTheDocument()
        expect(screen.queryByRole('region', { name: '上传进度' })).not.toBeInTheDocument()
        expect(screen.queryByRole('region', { name: '最近上传结果' })).not.toBeInTheDocument()
    })

    it('does not restore a cleared upload result when refresh happens during link lookup', async () => {
        const xhr = installMockXMLHttpRequest()
        const downloadUrlRequest = deferredResponse()
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
            .mockResolvedValueOnce(createUploadTokenResponse('demo.txt'))
            .mockReturnValueOnce(downloadUrlRequest.promise)
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
            .mockImplementation(() => Promise.reject(new Error('unexpected fetch')))
        vi.stubGlobal('fetch', fetchMock)

        render(<ObjectStoragePage />)

        const file = new File(['hello'], 'demo.txt', { type: 'text/plain' })
        fireEvent.change(await screen.findByLabelText('选择上传文件'), {
            target: { files: [file] },
        })

        await waitForLatestXMLHttpRequest(xhr)

        act(() => {
            MockXMLHttpRequest.instances[MockXMLHttpRequest.instances.length - 1]?.respond(200)
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
            expect(fetchMock).toHaveBeenCalledTimes(4)
        })
        expect(screen.queryByRole('region', { name: '最近上传结果' })).not.toBeInTheDocument()
    })

    it('keeps upload success visible when download url lookup fails', async () => {
        const xhr = installMockXMLHttpRequest()
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
            .mockResolvedValueOnce(createUploadTokenResponse('images/demo.txt'))
            .mockResolvedValueOnce(jsonResponse({ success: false, error: 'download lookup failed' }))
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
        vi.stubGlobal('fetch', fetchMock)

        render(<ObjectStoragePage />)

        const file = new File(['hello'], 'demo.txt', { type: 'text/plain' })
        fireEvent.change(await screen.findByLabelText('选择上传文件'), {
            target: { files: [file] },
        })

        await waitForLatestXMLHttpRequest(xhr)

        act(() => {
            MockXMLHttpRequest.instances[MockXMLHttpRequest.instances.length - 1]?.respond(200)
        })

        expect(await screen.findByText('已上传 images/demo.txt')).toBeInTheDocument()
        expect(screen.getByText('下载链接获取失败：download lookup failed')).toBeInTheDocument()
    })

    it('renders a dedicated recent upload result section with link and expiry', async () => {
        const xhr = installMockXMLHttpRequest()
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
            .mockResolvedValueOnce(createUploadTokenResponse('reports/demo.txt'))
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

        await waitForLatestXMLHttpRequest(xhr)

        act(() => {
            MockXMLHttpRequest.instances[MockXMLHttpRequest.instances.length - 1]?.respond(200)
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
        const xhr = installMockXMLHttpRequest()
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
            .mockResolvedValueOnce(createUploadTokenResponse('reports/demo.txt'))
            .mockResolvedValueOnce(createDownloadUrlResponse('reports/demo.txt', 'javascript:alert(1)'))
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
        vi.stubGlobal('fetch', fetchMock)

        render(<ObjectStoragePage />)

        const file = new File(['hello'], 'demo.txt', { type: 'text/plain' })
        fireEvent.change(await screen.findByLabelText('选择上传文件'), {
            target: { files: [file] },
        })

        await waitForLatestXMLHttpRequest(xhr)

        act(() => {
            MockXMLHttpRequest.instances[MockXMLHttpRequest.instances.length - 1]?.respond(200)
        })

        expect(await screen.findByText('已上传 reports/demo.txt')).toBeInTheDocument()
        expect(screen.getByText('下载链接不可用：返回了不安全的链接')).toBeInTheDocument()
        expect(screen.queryByRole('button', { name: '复制链接' })).not.toBeInTheDocument()
        expect(screen.queryByRole('link', { name: '打开链接' })).not.toBeInTheDocument()
    })

    it('clears the recent upload result when the user manually refreshes', async () => {
        const xhr = installMockXMLHttpRequest()
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
            .mockResolvedValueOnce(createUploadTokenResponse('images/demo.txt'))
            .mockResolvedValueOnce(createDownloadUrlResponse('images/demo.txt'))
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
        vi.stubGlobal('fetch', fetchMock)

        render(<ObjectStoragePage />)

        const file = new File(['hello'], 'demo.txt', { type: 'text/plain' })
        fireEvent.change(await screen.findByLabelText('选择上传文件'), {
            target: { files: [file] },
        })

        await waitForLatestXMLHttpRequest(xhr)

        act(() => {
            MockXMLHttpRequest.instances[MockXMLHttpRequest.instances.length - 1]?.respond(200)
        })

        expect(await screen.findByRole('region', { name: '最近上传结果' })).toBeInTheDocument()

        fireEvent.click(screen.getByRole('button', { name: '刷新' }))

        await waitFor(() => {
            expect(screen.queryByRole('region', { name: '最近上传结果' })).not.toBeInTheDocument()
        })
    })

    it('clears the recent upload result when navigating to another directory', async () => {
        const xhr = installMockXMLHttpRequest()
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(populatedListResponse ? jsonResponse(populatedListResponse) : jsonResponse(emptyListResponse))
            .mockResolvedValueOnce(createUploadTokenResponse('images/demo.txt'))
            .mockResolvedValueOnce(createDownloadUrlResponse('images/demo.txt'))
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

        await waitForLatestXMLHttpRequest(xhr)

        act(() => {
            MockXMLHttpRequest.instances[MockXMLHttpRequest.instances.length - 1]?.respond(200)
        })

        expect(await screen.findByRole('region', { name: '最近上传结果' })).toBeInTheDocument()

        fireEvent.click(await screen.findByRole('button', { name: '进入 images 目录' }))

        await waitFor(() => {
            expect(screen.queryByRole('region', { name: '最近上传结果' })).not.toBeInTheDocument()
        })
    })

    it('falls back to a manual-copy prompt when clipboard copy fails', async () => {
        const xhr = installMockXMLHttpRequest()
        const fetchMock = vi
            .fn()
            .mockResolvedValueOnce(jsonResponse(emptyListResponse))
            .mockResolvedValueOnce(createUploadTokenResponse('images/demo.txt'))
            .mockResolvedValueOnce(createDownloadUrlResponse('images/demo.txt'))
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

        await waitForLatestXMLHttpRequest(xhr)

        act(() => {
            MockXMLHttpRequest.instances[MockXMLHttpRequest.instances.length - 1]?.respond(200)
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

    it('renders safe download url in object detail as a clickable link and allows copying', async () => {
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
        const writeText = vi.fn().mockResolvedValue(undefined)
        Object.assign(navigator, { clipboard: { writeText } })

        render(<ObjectStoragePage />)

        fireEvent.click(await screen.findByRole('button', { name: '查看 demo.png 详情' }))

        const detailPanel = await screen.findByRole('complementary', { name: '对象详情面板' })
        const link = within(detailPanel).getByRole('link', { name: 'https://cdn.example.com/demo.png' })
        expect(link).toHaveAttribute('href', 'https://cdn.example.com/demo.png')
        expect(link).toHaveAttribute('target', '_blank')
        expect(link).toHaveAttribute('rel', 'noreferrer')

        const copyButton = within(detailPanel).getByRole('button', { name: '复制链接' })
        fireEvent.click(copyButton)

        expect(writeText).toHaveBeenCalledWith('https://cdn.example.com/demo.png')
        expect(await screen.findByText('下载链接已复制')).toBeInTheDocument()
    })

    it('renders missing download url in object detail as fallback text', async () => {
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
                        download_url: null,
                        storage_class: '0',
                    },
                }),
            )
        vi.stubGlobal('fetch', fetchMock)

        render(<ObjectStoragePage />)

        fireEvent.click(await screen.findByRole('button', { name: '查看 demo.png 详情' }))

        const detailPanel = await screen.findByRole('complementary', { name: '对象详情面板' })
        expect(within(detailPanel).getByText('未配置公开访问域名')).toBeInTheDocument()
        expect(within(detailPanel).queryByRole('link')).not.toBeInTheDocument()
        expect(within(detailPanel).queryByRole('button', { name: '复制链接' })).not.toBeInTheDocument()
    })

    it('renders unsafe download url in object detail as fallback text', async () => {
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
                        download_url: 'javascript:alert(1)',
                        storage_class: '0',
                    },
                }),
            )
        vi.stubGlobal('fetch', fetchMock)

        render(<ObjectStoragePage />)

        fireEvent.click(await screen.findByRole('button', { name: '查看 demo.png 详情' }))

        const detailPanel = await screen.findByRole('complementary', { name: '对象详情面板' })
        expect(within(detailPanel).getByText('未配置公开访问域名')).toBeInTheDocument()
        expect(within(detailPanel).queryByRole('link')).not.toBeInTheDocument()
        expect(within(detailPanel).queryByRole('button', { name: '复制链接' })).not.toBeInTheDocument()
    })

    it('renders whitelisted http download url in object detail as a link', async () => {
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
                        download_url: 'http://file.xiaoyxq.top/demo.png?token=test',
                        storage_class: '0',
                    },
                }),
            )
        vi.stubGlobal('fetch', fetchMock)

        render(<ObjectStoragePage />)

        fireEvent.click(await screen.findByRole('button', { name: '查看 demo.png 详情' }))

        const detailPanel = await screen.findByRole('complementary', { name: '对象详情面板' })
        const link = within(detailPanel).getByRole('link', {
            name: 'http://file.xiaoyxq.top/demo.png?token=test',
        })
        expect(link).toHaveAttribute('href', 'http://file.xiaoyxq.top/demo.png?token=test')
        expect(within(detailPanel).getByRole('button', { name: '复制链接' })).toBeInTheDocument()
        expect(within(detailPanel).queryByText('未配置公开访问域名')).not.toBeInTheDocument()
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
