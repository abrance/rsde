import { useState, useEffect } from 'react'
import './ToolPage.css'

interface TextBox {
    id: string
    author: string
    title?: string
    format: string
    content: string
    metadata: {
        created_at: string
        updated_at: string
        expires_at?: string
        view_count: number
        is_public: boolean
        language?: string
        tags: string[]
    }
}

export default function AnyboxPage() {
    const [activeTab, setActiveTab] = useState<'overview' | 'create' | 'list' | 'view'>('overview')

    // Create form state
    const [author, setAuthor] = useState('')
    const [title, setTitle] = useState('')
    const [content, setContent] = useState('')
    const [format, setFormat] = useState('plain')
    const [language, setLanguage] = useState('')
    const [tags, setTags] = useState('')
    const [expireHours, setExpireHours] = useState('')
    const [isPublic, setIsPublic] = useState(true)
    const [creating, setCreating] = useState(false)
    const [createResult, setCreateResult] = useState('')
    const [createdId, setCreatedId] = useState('')

    // List state
    const [textBoxes, setTextBoxes] = useState<TextBox[]>([])
    const [currentPage, setCurrentPage] = useState(1)
    const [pageSize] = useState(10)
    const [totalPages, setTotalPages] = useState(1)
    const [total, setTotal] = useState(0)
    const [loading, setLoading] = useState(false)

    // View state
    const [viewId, setViewId] = useState('')
    const [viewedTextBox, setViewedTextBox] = useState<TextBox | null>(null)
    const [viewing, setViewing] = useState(false)

    useEffect(() => {
        if (activeTab === 'list') {
            fetchTextBoxes()
        }
    }, [activeTab, currentPage])

    const fetchTextBoxes = async () => {
        setLoading(true)
        try {
            const response = await fetch(`/api/anybox/textbox?page=${currentPage}&page_size=${pageSize}`)
            const data = await response.json()
            if (data.success && data.data) {
                setTextBoxes(data.data.items)
                setTotal(data.data.total)
                setTotalPages(data.data.total_pages)
            }
        } catch (error) {
            console.error('获取列表失败:', error)
        } finally {
            setLoading(false)
        }
    }

    const handleCreate = async () => {
        if (!author.trim() || !content.trim()) {
            alert('作者和内容不能为空')
            return
        }

        setCreating(true)
        setCreateResult('')
        setCreatedId('')

        try {
            const requestBody: any = {
                author: author.trim(),
                content: content.trim(),
                format,
                is_public: isPublic,
            }

            if (title.trim()) requestBody.title = title.trim()
            if (language.trim()) requestBody.language = language.trim()
            if (tags.trim()) requestBody.tags = tags.split(',').map(t => t.trim()).filter(t => t)
            if (expireHours.trim()) requestBody.expire_hours = parseInt(expireHours)

            const response = await fetch('/api/anybox/textbox', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(requestBody),
            })

            const data = await response.json()

            if (data.success && data.data) {
                setCreatedId(data.data.id)
                setCreateResult(`✅ 创建成功！\n\nID: ${data.data.id}\n创建时间: ${new Date(data.data.metadata.created_at).toLocaleString()}`)
                // 清空表单
                setContent('')
                setTitle('')
                setLanguage('')
                setTags('')
                setExpireHours('')
            } else {
                setCreateResult(`❌ 创建失败: ${data.error || '未知错误'}`)
            }
        } catch (error) {
            setCreateResult(`❌ 请求失败: ${error}`)
        } finally {
            setCreating(false)
        }
    }

    const handleView = async (id?: string) => {
        const targetId = id || viewId
        if (!targetId.trim()) {
            alert('请输入 TextBox ID')
            return
        }

        setViewing(true)
        setViewedTextBox(null)

        try {
            const response = await fetch(`/api/anybox/textbox/${targetId.trim()}`)
            const data = await response.json()

            if (data.success && data.data) {
                setViewedTextBox(data.data)
            } else {
                alert(`获取失败: ${data.error || '未知错误'}`)
            }
        } catch (error) {
            alert(`请求失败: ${error}`)
        } finally {
            setViewing(false)
        }
    }

    const handleDelete = async (id: string) => {
        if (!confirm('确定要删除这个 TextBox 吗？')) {
            return
        }

        try {
            const response = await fetch(`/api/anybox/textbox/${id}`, {
                method: 'DELETE',
            })
            const data = await response.json()

            if (data.success) {
                alert('删除成功')
                fetchTextBoxes()
            } else {
                alert(`删除失败: ${data.error || '未知错误'}`)
            }
        } catch (error) {
            alert(`请求失败: ${error}`)
        }
    }

    const formatDateTime = (dateStr: string) => {
        return new Date(dateStr).toLocaleString('zh-CN')
    }

    return (
        <div className="tool-page">
            <div className="page-header">
                <h1 className="page-title">
                    <span className="page-icon">📦</span>
                    Anybox - 文本分享
                </h1>
                <p className="page-description">
                    类似 Pastebin 的文本分享服务，支持多种格式和自动过期
                </p>
            </div>

            <div className="tabs">
                <button
                    className={`tab ${activeTab === 'overview' ? 'active' : ''}`}
                    onClick={() => setActiveTab('overview')}
                >
                    概览
                </button>
                <button
                    className={`tab ${activeTab === 'create' ? 'active' : ''}`}
                    onClick={() => setActiveTab('create')}
                >
                    创建帖子
                </button>
                <button
                    className={`tab ${activeTab === 'list' ? 'active' : ''}`}
                    onClick={() => setActiveTab('list')}
                >
                    帖子列表
                </button>
                <button
                    className={`tab ${activeTab === 'view' ? 'active' : ''}`}
                    onClick={() => setActiveTab('view')}
                >
                    查看帖子
                </button>
            </div>

            <div className="tab-content">
                {activeTab === 'overview' && (
                    <div className="overview">
                        <div className="card">
                            <h2>功能特性</h2>
                            <ul className="feature-list">
                                <li>📝 创建文本帖子</li>
                                <li>🎨 多种格式支持（Plain, Markdown, Code, JSON, etc.）</li>
                                <li>🏷️ 标签分类</li>
                                <li>⏰ 自动过期清理</li>
                                <li>👁️ 浏览次数统计</li>
                                <li>🌐 公开/私有设置</li>
                                <li>💻 代码语法高亮支持</li>
                            </ul>
                        </div>

                        <div className="card">
                            <h2>支持的格式</h2>
                            <div className="format-grid">
                                <span className="format-badge">Plain</span>
                                <span className="format-badge">Markdown</span>
                                <span className="format-badge">Code</span>
                                <span className="format-badge">JSON</span>
                                <span className="format-badge">XML</span>
                                <span className="format-badge">HTML</span>
                                <span className="format-badge">YAML</span>
                            </div>
                        </div>

                        <div className="card">
                            <h2>使用说明</h2>
                            <ol style={{ lineHeight: '1.8' }}>
                                <li>在"创建帖子"标签页填写内容并提交</li>
                                <li>系统会返回一个唯一的 ID</li>
                                <li>使用 ID 可以查看或分享帖子</li>
                                <li>设置过期时间后帖子会自动删除</li>
                            </ol>
                        </div>
                    </div>
                )}

                {activeTab === 'create' && (
                    <div className="create-panel">
                        <div className="card">
                            <h2>创建新帖子</h2>

                            <div className="form-group">
                                <label htmlFor="author">作者姓名 *</label>
                                <input
                                    id="author"
                                    type="text"
                                    className="input"
                                    placeholder="请输入作者姓名"
                                    value={author}
                                    onChange={(e) => setAuthor(e.target.value)}
                                />
                            </div>

                            <div className="form-group">
                                <label htmlFor="title">标题（可选）</label>
                                <input
                                    id="title"
                                    type="text"
                                    className="input"
                                    placeholder="请输入标题"
                                    value={title}
                                    onChange={(e) => setTitle(e.target.value)}
                                />
                            </div>

                            <div className="form-group">
                                <label htmlFor="content">内容 *</label>
                                <textarea
                                    id="content"
                                    className="input"
                                    placeholder="请输入文本内容..."
                                    value={content}
                                    onChange={(e) => setContent(e.target.value)}
                                    rows={10}
                                    style={{ fontFamily: 'monospace' }}
                                />
                            </div>

                            <div className="form-group">
                                <label htmlFor="format">文本格式</label>
                                <select
                                    id="format"
                                    className="input"
                                    value={format}
                                    onChange={(e) => setFormat(e.target.value)}
                                >
                                    <option value="plain">Plain Text</option>
                                    <option value="markdown">Markdown</option>
                                    <option value="code">Code</option>
                                    <option value="json">JSON</option>
                                    <option value="xml">XML</option>
                                    <option value="html">HTML</option>
                                    <option value="yaml">YAML</option>
                                </select>
                            </div>

                            <div className="form-group">
                                <label htmlFor="language">代码语言（可选）</label>
                                <input
                                    id="language"
                                    type="text"
                                    className="input"
                                    placeholder="例如: rust, python, javascript"
                                    value={language}
                                    onChange={(e) => setLanguage(e.target.value)}
                                />
                            </div>

                            <div className="form-group">
                                <label htmlFor="tags">标签（可选，逗号分隔）</label>
                                <input
                                    id="tags"
                                    type="text"
                                    className="input"
                                    placeholder="例如: rust, example, tutorial"
                                    value={tags}
                                    onChange={(e) => setTags(e.target.value)}
                                />
                            </div>

                            <div className="form-group">
                                <label htmlFor="expireHours">过期时间（小时，可选）</label>
                                <input
                                    id="expireHours"
                                    type="number"
                                    className="input"
                                    placeholder="例如: 24 表示24小时后过期"
                                    value={expireHours}
                                    onChange={(e) => setExpireHours(e.target.value)}
                                    min="1"
                                />
                            </div>

                            <div className="form-group">
                                <label className="checkbox-label">
                                    <input
                                        type="checkbox"
                                        checked={isPublic}
                                        onChange={(e) => setIsPublic(e.target.checked)}
                                    />
                                    公开帖子
                                </label>
                            </div>

                            <button
                                className="btn"
                                onClick={handleCreate}
                                disabled={creating}
                            >
                                {creating ? '创建中...' : '📝 创建帖子'}
                            </button>

                            {createResult && (
                                <div className="result-box">
                                    <pre className="result-content">{createResult}</pre>
                                    {createdId && (
                                        <button
                                            className="btn"
                                            onClick={() => {
                                                setViewId(createdId)
                                                setActiveTab('view')
                                                setTimeout(() => handleView(createdId), 100)
                                            }}
                                            style={{ marginTop: '10px' }}
                                        >
                                            👁️ 查看帖子
                                        </button>
                                    )}
                                </div>
                            )}
                        </div>
                    </div>
                )}

                {activeTab === 'list' && (
                    <div className="list-panel">
                        <div className="card">
                            <h2>帖子列表</h2>
                            <p style={{ marginBottom: '20px' }}>
                                共 {total} 个帖子 | 第 {currentPage}/{totalPages} 页
                            </p>

                            {loading ? (
                                <p className="loading-text">加载中...</p>
                            ) : textBoxes.length === 0 ? (
                                <p className="placeholder-text">暂无帖子</p>
                            ) : (
                                <>
                                    <div className="textbox-list">
                                        {textBoxes.map((textBox) => (
                                            <div key={textBox.id} className="textbox-item card" style={{ marginBottom: '15px', padding: '15px' }}>
                                                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'start' }}>
                                                    <div style={{ flex: 1 }}>
                                                        <h3 style={{ margin: '0 0 10px 0' }}>
                                                            {textBox.title || '(无标题)'}
                                                        </h3>
                                                        <div style={{ fontSize: '0.9em', color: '#666', marginBottom: '10px' }}>
                                                            <span>📝 {textBox.author}</span>
                                                            <span style={{ margin: '0 10px' }}>|</span>
                                                            <span>🔤 {textBox.format}</span>
                                                            <span style={{ margin: '0 10px' }}>|</span>
                                                            <span>👁️ {textBox.metadata.view_count} 次浏览</span>
                                                            <span style={{ margin: '0 10px' }}>|</span>
                                                            <span>🕒 {formatDateTime(textBox.metadata.created_at)}</span>
                                                        </div>
                                                        {textBox.metadata.tags.length > 0 && (
                                                            <div style={{ marginBottom: '10px' }}>
                                                                {textBox.metadata.tags.map((tag, i) => (
                                                                    <span key={i} className="format-badge" style={{ marginRight: '5px' }}>
                                                                        {tag}
                                                                    </span>
                                                                ))}
                                                            </div>
                                                        )}
                                                        <div style={{
                                                            maxHeight: '100px',
                                                            overflow: 'hidden',
                                                            textOverflow: 'ellipsis',
                                                            backgroundColor: '#f5f5f5',
                                                            padding: '10px',
                                                            borderRadius: '4px',
                                                            fontFamily: 'monospace',
                                                            fontSize: '0.9em'
                                                        }}>
                                                            {textBox.content.substring(0, 200)}
                                                            {textBox.content.length > 200 && '...'}
                                                        </div>
                                                    </div>
                                                    <div style={{ marginLeft: '15px', display: 'flex', flexDirection: 'column', gap: '5px' }}>
                                                        <button
                                                            className="btn"
                                                            onClick={() => {
                                                                setViewId(textBox.id)
                                                                setActiveTab('view')
                                                                setTimeout(() => handleView(textBox.id), 100)
                                                            }}
                                                            style={{ fontSize: '0.9em', padding: '5px 10px' }}
                                                        >
                                                            查看
                                                        </button>
                                                        <button
                                                            className="btn"
                                                            onClick={() => handleDelete(textBox.id)}
                                                            style={{ fontSize: '0.9em', padding: '5px 10px', backgroundColor: '#dc3545' }}
                                                        >
                                                            删除
                                                        </button>
                                                    </div>
                                                </div>
                                            </div>
                                        ))}
                                    </div>

                                    <div style={{ display: 'flex', justifyContent: 'center', gap: '10px', marginTop: '20px' }}>
                                        <button
                                            className="btn"
                                            onClick={() => setCurrentPage(p => Math.max(1, p - 1))}
                                            disabled={currentPage === 1}
                                        >
                                            上一页
                                        </button>
                                        <span style={{ lineHeight: '40px' }}>
                                            {currentPage} / {totalPages}
                                        </span>
                                        <button
                                            className="btn"
                                            onClick={() => setCurrentPage(p => Math.min(totalPages, p + 1))}
                                            disabled={currentPage === totalPages}
                                        >
                                            下一页
                                        </button>
                                    </div>
                                </>
                            )}
                        </div>
                    </div>
                )}

                {activeTab === 'view' && (
                    <div className="view-panel">
                        <div className="card">
                            <h2>查看帖子</h2>

                            <div className="form-group">
                                <label htmlFor="viewId">TextBox ID</label>
                                <div style={{ display: 'flex', gap: '10px' }}>
                                    <input
                                        id="viewId"
                                        type="text"
                                        className="input"
                                        placeholder="请输入 TextBox ID"
                                        value={viewId}
                                        onChange={(e) => setViewId(e.target.value)}
                                        style={{ flex: 1 }}
                                    />
                                    <button
                                        className="btn"
                                        onClick={() => handleView()}
                                        disabled={viewing}
                                    >
                                        {viewing ? '加载中...' : '👁️ 查看'}
                                    </button>
                                </div>
                            </div>

                            {viewedTextBox && (
                                <div className="result-box">
                                    <h3>{viewedTextBox.title || '(无标题)'}</h3>
                                    <div style={{ fontSize: '0.9em', color: '#666', marginBottom: '15px' }}>
                                        <p>📝 作者: {viewedTextBox.author}</p>
                                        <p>🆔 ID: {viewedTextBox.id}</p>
                                        <p>🔤 格式: {viewedTextBox.format}</p>
                                        {viewedTextBox.metadata.language && (
                                            <p>💻 语言: {viewedTextBox.metadata.language}</p>
                                        )}
                                        <p>👁️ 浏览次数: {viewedTextBox.metadata.view_count}</p>
                                        <p>🕒 创建时间: {formatDateTime(viewedTextBox.metadata.created_at)}</p>
                                        <p>🔄 更新时间: {formatDateTime(viewedTextBox.metadata.updated_at)}</p>
                                        {viewedTextBox.metadata.expires_at && (
                                            <p>⏰ 过期时间: {formatDateTime(viewedTextBox.metadata.expires_at)}</p>
                                        )}
                                        <p>🌐 公开: {viewedTextBox.metadata.is_public ? '是' : '否'}</p>
                                        {viewedTextBox.metadata.tags.length > 0 && (
                                            <div style={{ marginTop: '10px' }}>
                                                <span>🏷️ 标签: </span>
                                                {viewedTextBox.metadata.tags.map((tag, i) => (
                                                    <span key={i} className="format-badge" style={{ marginRight: '5px' }}>
                                                        {tag}
                                                    </span>
                                                ))}
                                            </div>
                                        )}
                                    </div>
                                    <div style={{ marginTop: '20px' }}>
                                        <h4>内容:</h4>
                                        <pre className="result-content" style={{
                                            whiteSpace: 'pre-wrap',
                                            wordBreak: 'break-word'
                                        }}>
                                            {viewedTextBox.content}
                                        </pre>
                                    </div>
                                </div>
                            )}
                        </div>
                    </div>
                )}
            </div>
        </div>
    )
}
