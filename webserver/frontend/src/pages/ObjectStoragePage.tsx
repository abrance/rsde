import { type ChangeEvent, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import './ObjectStoragePage.css'

type ApiEnvelope<T> = {
    success: boolean
    data?: T
    error?: string
}

type ObjectStorageItem = {
    key: string
    name: string
    is_directory: boolean
    size: number
    mime_type?: string | null
    updated_at?: string | null
    hash?: string | null
}

type ObjectStoragePrefix = {
    key: string
    name: string
    is_directory: boolean
}

type ObjectListData = {
    current_prefix: string
    marker: string | null
    has_more: boolean
    prefixes: ObjectStoragePrefix[]
    items: ObjectStorageItem[]
}

type ObjectDetailData = ObjectStorageItem & {
    download_url?: string | null
    storage_class?: string | null
}

type BatchDeleteResult = {
    deleted_keys: string[]
    failed: Array<{ key: string; error: string }>
}

type CreateUploadTokenData = {
    upload_token: string
    object_key: string
    upload_key: string
    upload_url: string
}

type DownloadUrlData = {
    key: string
    download_url: string
    expires_at?: string | null
}

type RecentUploadResult = {
    key: string
    downloadUrl?: string
    expiresAt?: string | null
    linkError?: string | null
}

const rootListData: ObjectListData = {
    current_prefix: '',
    marker: null,
    has_more: false,
    prefixes: [],
    items: [],
}

async function requestJson<T>(url: string, init?: RequestInit): Promise<T> {
    const response = init === undefined ? await fetch(url) : await fetch(url, init)
    const envelope = (await response.json()) as ApiEnvelope<T>

    if (!response.ok || !envelope.success || envelope.data === undefined) {
        throw new Error(envelope.error || `请求失败：${response.status}`)
    }

    return envelope.data
}

function formatSize(size: number): string {
    if (size < 1024) {
        return `${size} B`
    }

    if (size < 1024 * 1024) {
        return `${(size / 1024).toFixed(1)} KB`
    }

    return `${(size / 1024 / 1024).toFixed(1)} MB`
}

function formatDate(value?: string | null): string {
    if (!value) {
        return '-'
    }

    const date = new Date(value)
    if (Number.isNaN(date.getTime())) {
        return '-'
    }

    return new Intl.DateTimeFormat('zh-CN', {
        year: 'numeric',
        month: '2-digit',
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
    }).format(date)
}

function buildBreadcrumbs(prefix: string): Array<{ name: string; prefix: string }> {
    if (!prefix) {
        return []
    }

    const names = prefix.split('/').filter(Boolean)
    return names.map((name, index) => ({
        name,
        prefix: `${names.slice(0, index + 1).join('/')}/`,
    }))
}

function normalizeDownloadUrl(value?: string | null): string | null {
    if (!value) {
        return null
    }

    try {
        const parsed = new URL(value)
        const isWhitelistedHttpDownloadHost =
            parsed.protocol === 'http:' && parsed.hostname === 'file.xiaoyxq.top'
        const isLocalHttp =
            parsed.protocol === 'http:' &&
            (parsed.hostname === 'localhost' || parsed.hostname === '127.0.0.1' || parsed.hostname === '[::1]')

        if (parsed.protocol === 'https:' || isLocalHttp || isWhitelistedHttpDownloadHost) {
            return parsed.toString()
        }

        return null
    } catch {
        return null
    }
}

export default function ObjectStoragePage() {
    const listRequestIdRef = useRef(0)
    const detailRequestIdRef = useRef(0)
    const uploadRequestIdRef = useRef(0)
    const fileInputRef = useRef<HTMLInputElement | null>(null)
    const [currentPrefix, setCurrentPrefix] = useState('')
    const [listData, setListData] = useState<ObjectListData>(rootListData)
    const [selectedKeys, setSelectedKeys] = useState<Set<string>>(() => new Set())
    const [detailItem, setDetailItem] = useState<ObjectDetailData | null>(null)
    const [loading, setLoading] = useState(false)
    const [error, setError] = useState('')
    const [operationMessage, setOperationMessage] = useState('')
    const [recentUploadResult, setRecentUploadResult] = useState<RecentUploadResult | null>(null)

    const breadcrumbs = useMemo(() => buildBreadcrumbs(currentPrefix), [currentPrefix])
    const hasObjects = listData.prefixes.length > 0 || listData.items.length > 0

    const loadObjects = useCallback(async (prefix: string) => {
        const requestId = listRequestIdRef.current + 1
        listRequestIdRef.current = requestId
        setLoading(true)
        setError('')

        try {
            const query = prefix ? `?prefix=${encodeURIComponent(prefix)}` : ''
            const data = await requestJson<ObjectListData>(`/api/object-storage/objects${query}`)
            if (listRequestIdRef.current !== requestId) {
                return
            }
            setListData(data)
            setSelectedKeys(new Set())
            setCurrentPrefix(data.current_prefix)
        } catch (requestError) {
            if (listRequestIdRef.current !== requestId) {
                return
            }
            const message = requestError instanceof Error ? requestError.message : '对象列表加载失败'
            setError(message)
        } finally {
            if (listRequestIdRef.current === requestId) {
                setLoading(false)
            }
        }
    }, [])

    useEffect(() => {
        void loadObjects('')
    }, [loadObjects])

    const refreshCurrentDirectory = useCallback(async () => {
        await loadObjects(currentPrefix)
    }, [currentPrefix, loadObjects])

    const openDirectory = async (prefix: string) => {
        setDetailItem(null)
        setOperationMessage('')
        await loadObjects(prefix)
    }

    const clearRecentUploadResult = () => {
        uploadRequestIdRef.current += 1
        setRecentUploadResult(null)
    }

    const handleRefresh = () => {
        clearRecentUploadResult()
        void refreshCurrentDirectory()
    }

    const handleOpenDirectory = (prefix: string) => {
        clearRecentUploadResult()
        void openDirectory(prefix)
    }

    const copyDownloadUrl = (url: string) => {
        if (!navigator.clipboard?.writeText) {
            window.prompt('请手动复制下载链接', url)
            return
        }

        navigator.clipboard.writeText(url).then(
            () => {
                setOperationMessage('下载链接已复制')
            },
            () => {
                window.prompt('请手动复制下载链接', url)
            },
        )
    }

    const openDetail = async (key: string) => {
        const requestId = detailRequestIdRef.current + 1
        detailRequestIdRef.current = requestId
        setError('')
        setOperationMessage('')
        setDetailItem(null)

        try {
            const detail = await requestJson<ObjectDetailData>(
                `/api/object-storage/objects/detail?key=${encodeURIComponent(key)}`,
            )
            if (detailRequestIdRef.current !== requestId) {
                return
            }
            setDetailItem(detail)
        } catch (requestError) {
            if (detailRequestIdRef.current !== requestId) {
                return
            }
            const message = requestError instanceof Error ? requestError.message : '对象详情加载失败'
            setError(message)
        }
    }

    const deleteObject = async (key: string) => {
        if (!window.confirm(`确定删除 ${key} 吗？`)) {
            return
        }

        setError('')
        setOperationMessage('')

        try {
            await requestJson<{ deleted_key: string }>('/api/object-storage/objects/delete', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ key }),
            })
            setOperationMessage(`已删除 ${key}`)
            setDetailItem((current) => (current?.key === key ? null : current))
            await refreshCurrentDirectory()
        } catch (requestError) {
            const message = requestError instanceof Error ? requestError.message : '删除对象失败'
            setError(message)
        }
    }

    const deleteSelectedObjects = async () => {
        const keys = Array.from(selectedKeys)
        if (keys.length === 0) {
            return
        }

        if (!window.confirm(`确定删除已选的 ${keys.length} 个对象吗？`)) {
            return
        }

        setError('')
        setOperationMessage('')

        try {
            const result = await requestJson<BatchDeleteResult>('/api/object-storage/objects/delete-batch', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ keys }),
            })

            if (result.failed.length > 0) {
                setOperationMessage(result.failed.map((item) => `${item.key}: ${item.error}`).join('；'))
            } else {
                setOperationMessage(`已删除 ${result.deleted_keys.length} 个对象`)
            }

            await refreshCurrentDirectory()
        } catch (requestError) {
            const message = requestError instanceof Error ? requestError.message : '批量删除失败'
            setError(message)
        }
    }

    const toggleSelected = (key: string) => {
        setSelectedKeys((current) => {
            const next = new Set(current)
            if (next.has(key)) {
                next.delete(key)
            } else {
                next.add(key)
            }
            return next
        })
    }

    const createDirectory = async () => {
        const name = window.prompt('请输入目录名称')?.trim()
        if (!name) {
            return
        }

        setError('')
        setOperationMessage('')

        try {
            const requestBody = currentPrefix ? { prefix: currentPrefix, name } : { name }
            const result = await requestJson<ObjectStoragePrefix>('/api/object-storage/directories', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(requestBody),
            })
            setOperationMessage(`已创建目录 ${result.key}`)
            await refreshCurrentDirectory()
        } catch (requestError) {
            const message = requestError instanceof Error ? requestError.message : '创建目录失败'
            setError(message)
        }
    }

    const selectUploadFile = () => {
        fileInputRef.current?.click()
    }

    const uploadFile = async (file: File) => {
        const uploadRequestId = uploadRequestIdRef.current + 1
        uploadRequestIdRef.current = uploadRequestId
        const uploadPrefix = currentPrefix

        setError('')
        setOperationMessage('')
        setRecentUploadResult(null)

        try {
            const requestBody = currentPrefix
                ? { prefix: currentPrefix, filename: file.name }
                : { filename: file.name }
            const uploadData = await requestJson<CreateUploadTokenData>('/api/object-storage/upload-token', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(requestBody),
            })

            const formData = new FormData()
            formData.append('token', uploadData.upload_token)
            formData.append('key', uploadData.upload_key)
            formData.append('file', file)

            const uploadResponse = await fetch(uploadData.upload_url, {
                method: 'POST',
                body: formData,
            })
            if (!uploadResponse.ok) {
                throw new Error(`上传失败：${uploadResponse.status}`)
            }

            setOperationMessage(`已上传 ${uploadData.object_key}`)

            try {
                const downloadData = await requestJson<DownloadUrlData>(
                    `/api/object-storage/download-url?key=${encodeURIComponent(uploadData.object_key)}`,
                )
                if (uploadRequestIdRef.current !== uploadRequestId) {
                    return
                }

                const safeDownloadUrl = normalizeDownloadUrl(downloadData.download_url)
                setRecentUploadResult({
                    key: downloadData.key,
                    downloadUrl: safeDownloadUrl ?? undefined,
                    expiresAt: downloadData.expires_at ?? null,
                    linkError: safeDownloadUrl ? null : '下载链接不可用：返回了不安全的链接',
                })
            } catch (downloadError) {
                if (uploadRequestIdRef.current !== uploadRequestId) {
                    return
                }

                const message = downloadError instanceof Error ? downloadError.message : '下载链接获取失败'
                setRecentUploadResult({
                    key: uploadData.object_key,
                    downloadUrl: undefined,
                    expiresAt: null,
                    linkError: `下载链接获取失败：${message}`,
                })
            }

            if (uploadRequestIdRef.current === uploadRequestId) {
                await loadObjects(uploadPrefix)
            }
        } catch (requestError) {
            const message = requestError instanceof Error ? requestError.message : '上传文件失败'
            setError(message)
        }
    }

    const handleUploadFileChange = async (event: ChangeEvent<HTMLInputElement>) => {
        const file = event.target.files?.[0]
        event.target.value = ''
        if (!file) {
            return
        }

        await uploadFile(file)
    }

    const renderDetailDownloadUrl = () => {
        const safeUrl = detailItem?.download_url ? normalizeDownloadUrl(detailItem.download_url) : null

        if (!safeUrl) {
            return '未配置公开访问域名'
        }

        return (
            <div className="object-storage-detail-download">
                <a
                    href={safeUrl}
                    target="_blank"
                    rel="noreferrer"
                    className="object-storage-detail-download-link"
                >
                    {safeUrl}
                </a>
                <button
                    className="object-storage-button secondary object-storage-detail-copy"
                    type="button"
                    onClick={() => copyDownloadUrl(safeUrl)}
                >
                    复制链接
                </button>
            </div>
        )
    }

    return (
        <div className="object-storage-page">
            <section className="object-storage-hero">
                <div>
                    <div className="object-storage-eyebrow">Qiniu Kodo Object Storage</div>
                    <h1>对象存储文件管理</h1>
                    <p className="object-storage-description">
                        通过 apiserver 统一管理七牛云对象存储，支持目录浏览、对象详情、删除和后续上传流程。
                    </p>
                </div>
                <aside className="object-storage-status-card" aria-label="存储状态">
                    <span className="status-dot" aria-hidden="true" />
                    <strong>当前连接</strong>
                    <span>{loading ? '正在同步对象列表' : '配置由后端托管，前端不暴露密钥'}</span>
                </aside>
            </section>

            <section className="object-storage-toolbar" aria-label="对象存储操作栏">
                <nav className="object-storage-breadcrumb" aria-label="当前路径">
                    <span>存储桶</span>
                    <span className="breadcrumb-separator">/</span>
                    <button
                        className="object-storage-link-button"
                        type="button"
                        onClick={() => handleOpenDirectory('')}
                        aria-label="返回根目录"
                    >
                        根目录
                    </button>
                    {breadcrumbs.map((item) => (
                        <span className="breadcrumb-node" key={item.prefix}>
                            <span className="breadcrumb-separator">/</span>
                            <button
                                className="object-storage-link-button"
                                type="button"
                                onClick={() => handleOpenDirectory(item.prefix)}
                            >
                                {item.name}
                            </button>
                        </span>
                    ))}
                </nav>
                <div className="object-storage-actions">
                    <button
                        className="object-storage-button secondary"
                        type="button"
                        onClick={handleRefresh}
                    >
                        刷新
                    </button>
                    <button
                        className="object-storage-button secondary"
                        type="button"
                        onClick={selectUploadFile}
                    >
                        上传文件
                    </button>
                    <input
                        ref={fileInputRef}
                        type="file"
                        aria-label="选择上传文件"
                        className="object-storage-file-input"
                        onChange={(event) => void handleUploadFileChange(event)}
                    />
                    <button
                        className="object-storage-button secondary"
                        type="button"
                        onClick={() => void createDirectory()}
                    >
                        新建目录
                    </button>
                    <button
                        className="object-storage-button primary"
                        type="button"
                        disabled={selectedKeys.size === 0}
                        onClick={() => void deleteSelectedObjects()}
                    >
                        删除已选
                    </button>
                </div>
            </section>

            {error && <div className="object-storage-alert error">{error}</div>}
            {operationMessage && <div className="object-storage-alert">{operationMessage}</div>}
            {recentUploadResult && (
                <section
                    className="object-storage-upload-result-card"
                    aria-labelledby="upload-result-title"
                >
                    <div className="object-storage-upload-result-header">
                        <h2 id="upload-result-title">最近上传结果</h2>
                    </div>

                    <div className="object-storage-upload-result-body">
                        <p className="object-storage-upload-result-key">
                            <strong>文件：</strong> {recentUploadResult.key}
                        </p>

                        {recentUploadResult.linkError ? (
                            <div className="object-storage-upload-result-warning" role="alert">
                                {recentUploadResult.linkError}
                            </div>
                        ) : (
                            <>
                                <div className="object-storage-upload-result-link-group">
                                    <p className="object-storage-upload-result-url">
                                        {recentUploadResult.downloadUrl}
                                    </p>
                                    <div className="object-storage-upload-result-actions">
                                        <button
                                            className="object-storage-button secondary"
                                            type="button"
                                            onClick={() => {
                                                if (recentUploadResult.downloadUrl) {
                                                    copyDownloadUrl(recentUploadResult.downloadUrl)
                                                }
                                            }}
                                            disabled={!recentUploadResult.downloadUrl}
                                        >
                                            复制链接
                                        </button>
                                        {recentUploadResult.downloadUrl && (
                                            <a
                                                href={recentUploadResult.downloadUrl}
                                                target="_blank"
                                                rel="noreferrer"
                                                className="object-storage-button secondary"
                                            >
                                                打开链接
                                            </a>
                                        )}
                                    </div>
                                </div>
                                {recentUploadResult.expiresAt && (
                                    <p className="object-storage-upload-result-meta">
                                        链接有效期至 {formatDate(recentUploadResult.expiresAt)}
                                    </p>
                                )}
                            </>
                        )}
                    </div>
                </section>
            )}

            <section className="object-storage-table-card" aria-label="对象列表">
                <div className="object-storage-table-header" aria-hidden="true">
                    <span>名称</span>
                    <span>大小</span>
                    <span>类型</span>
                    <span>更新时间</span>
                    <span>操作</span>
                </div>

                {loading && <div className="object-storage-loading">加载对象列表中...</div>}

                {!loading && !hasObjects && (
                    <div className="object-storage-empty-state">
                        <div className="empty-state-orb" aria-hidden="true">
                            🗂️
                        </div>
                        <h2>当前目录暂无文件</h2>
                        <p>选择上传文件或新建目录，开始管理对象存储内容。</p>
                    </div>
                )}

                {!loading && hasObjects && (
                    <table className="object-storage-table">
                        <thead>
                            <tr>
                                <th>名称</th>
                                <th>大小</th>
                                <th>类型</th>
                                <th>更新时间</th>
                                <th>操作</th>
                            </tr>
                        </thead>
                        <tbody>
                            {listData.prefixes.map((prefix) => (
                                <tr key={prefix.key}>
                                    <td>
                                        <span className="object-storage-name">
                                            <span aria-hidden="true">📁</span>
                                            <span>{prefix.name}</span>
                                        </span>
                                    </td>
                                    <td>-</td>
                                    <td>目录</td>
                                    <td>-</td>
                                    <td>
                                        <button
                                            className="object-storage-link-button"
                                            type="button"
                                            onClick={() => handleOpenDirectory(prefix.key)}
                                            aria-label={`进入 ${prefix.name} 目录`}
                                        >
                                            进入
                                        </button>
                                    </td>
                                </tr>
                            ))}
                            {listData.items.map((item) => (
                                <tr key={item.key}>
                                    <td>
                                        <label className="object-storage-select-label">
                                            <input
                                                type="checkbox"
                                                checked={selectedKeys.has(item.key)}
                                                onChange={() => toggleSelected(item.key)}
                                                aria-label={`选择 ${item.name}`}
                                            />
                                            <span className="object-storage-name">
                                                <span aria-hidden="true">📄</span>
                                                <span>{item.name}</span>
                                            </span>
                                        </label>
                                    </td>
                                    <td>{formatSize(item.size)}</td>
                                    <td>{item.mime_type || '对象'}</td>
                                    <td>{formatDate(item.updated_at)}</td>
                                    <td>
                                        <div className="object-storage-row-actions">
                                            <button
                                                className="object-storage-link-button"
                                                type="button"
                                                onClick={() => void openDetail(item.key)}
                                                aria-label={`查看 ${item.name} 详情`}
                                            >
                                                详情
                                            </button>
                                            <button
                                                className="object-storage-danger-button"
                                                type="button"
                                                onClick={() => void deleteObject(item.key)}
                                                aria-label={`删除 ${item.name}`}
                                            >
                                                删除
                                            </button>
                                        </div>
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                )}
            </section>

            {detailItem && (
                <aside className="object-storage-detail-card" aria-label="对象详情面板">
                    <div className="object-storage-detail-header">
                        <h2>对象详情</h2>
                        <button
                            className="object-storage-link-button"
                            type="button"
                            onClick={() => setDetailItem(null)}
                        >
                            关闭
                        </button>
                    </div>
                    <dl className="object-storage-detail-list">
                        <div>
                            <dt>对象 Key</dt>
                            <dd>{detailItem.key}</dd>
                        </div>
                        <div>
                            <dt>哈希</dt>
                            <dd>{detailItem.hash || '-'}</dd>
                        </div>
                        <div>
                            <dt>大小</dt>
                            <dd>{formatSize(detailItem.size)}</dd>
                        </div>
                        <div>
                            <dt>存储类型</dt>
                            <dd>{detailItem.storage_class || '-'}</dd>
                        </div>
                        <div>
                            <dt>下载地址</dt>
                            <dd>
                                {renderDetailDownloadUrl()}
                            </dd>
                        </div>
                    </dl>
                </aside>
            )}
        </div>
    )
}
