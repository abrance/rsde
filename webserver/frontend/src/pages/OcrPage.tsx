import { useState } from 'react'
import './ToolPage.css'

export default function OcrPage() {
    const [activeTab, setActiveTab] = useState<'overview' | 'recognize' | 'history'>('overview')
    const [imagePath, setImagePath] = useState('')
    const [uploadedPath, setUploadedPath] = useState('')
    const [includePosition, setIncludePosition] = useState(false)
    const [result, setResult] = useState<string>('')
    const [loading, setLoading] = useState(false)
    const [uploading, setUploading] = useState(false)

    const handleFileUpload = async (event: React.ChangeEvent<HTMLInputElement>) => {
        const file = event.target.files?.[0]
        if (!file) return

        setUploading(true)
        setResult('')
        setUploadedPath('')

        try {
            const formData = new FormData()
            formData.append('file', file)

            const response = await fetch('/api/image/upload', {
                method: 'POST',
                body: formData,
            })

            const data = await response.json()

            if (data.success && data.path) {
                setUploadedPath(data.path)
                setImagePath(data.path)
                setResult(`✅ 图片上传成功: ${data.path}`)
            } else {
                setResult(`❌ 上传失败: ${data.error || '未知错误'}`)
            }
        } catch (error) {
            setResult(`❌ 上传请求失败: ${error}`)
        } finally {
            setUploading(false)
        }
    }

    const handleRecognize = async () => {
        if (!imagePath.trim()) {
            alert('请输入图片路径')
            return
        }

        setLoading(true)
        setResult('')

        try {
            const response = await fetch('/api/ocr/single_pic', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({
                    image_path: imagePath,
                    include_position: includePosition,
                }),
            })

            const data = await response.json()

            if (data.success) {
                setResult(data.text || JSON.stringify(data, null, 2))
            } else {
                setResult(`错误: ${data.error}`)
            }
        } catch (error) {
            setResult(`请求失败: ${error}`)
        } finally {
            setLoading(false)
        }
    }

    return (
        <div className="tool-page">
            <div className="page-header">
                <h1 className="page-title">
                    <span className="page-icon">📝</span>
                    OCR - 图片文字识别
                </h1>
                <p className="page-description">
                    基于远程 OCR 服务的图片文字识别，支持多种语言和格式
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
                    className={`tab ${activeTab === 'recognize' ? 'active' : ''}`}
                    onClick={() => setActiveTab('recognize')}
                >
                    文字识别
                </button>
                <button
                    className={`tab ${activeTab === 'history' ? 'active' : ''}`}
                    onClick={() => setActiveTab('history')}
                >
                    识别历史
                </button>
            </div>

            <div className="tab-content">
                {activeTab === 'overview' && (
                    <div className="overview">
                        <div className="card">
                            <h2>功能特性</h2>
                            <ul className="feature-list">
                                <li>🌐 远程 OCR 服务</li>
                                <li>🌍 多语言支持（中文、英文、日文等）</li>
                                <li>📍 坐标信息提取</li>
                                <li>📦 批量处理</li>
                                <li>🖼️ 支持多种图片格式</li>
                                <li>⚡ 高性能识别</li>
                            </ul>
                        </div>

                        <div className="card">
                            <h2>API 使用示例</h2>
                            <div className="code-block">
                                <pre>{`# 识别图片文字（仅文本）
curl -X POST http://localhost:8080/ocr/single_pic \\
  -H "Content-Type: application/json" \\
  -d '{
    "image_path": "/path/to/image.png",
    "include_position": false
  }'

# 识别图片文字（含坐标）
curl -X POST http://localhost:8080/ocr/single_pic \\
  -H "Content-Type: application/json" \\
  -d '{
    "image_path": "/path/to/image.png",
    "include_position": true
  }'`}</pre>
                            </div>
                        </div>

                        <div className="card">
                            <h2>支持的图片格式</h2>
                            <div className="format-grid">
                                <span className="format-badge">PNG</span>
                                <span className="format-badge">JPG/JPEG</span>
                                <span className="format-badge">BMP</span>
                                <span className="format-badge">GIF</span>
                                <span className="format-badge">TIFF</span>
                                <span className="format-badge">WEBP</span>
                            </div>
                        </div>
                    </div>
                )}

                {activeTab === 'recognize' && (
                    <div className="recognize-panel">
                        <div className="card">
                            <h2>图片上传与识别</h2>

                            <div className="form-group">
                                <label htmlFor="imageFile">选择图片</label>
                                <input
                                    id="imageFile"
                                    type="file"
                                    className="input"
                                    accept="image/*"
                                    onChange={handleFileUpload}
                                    disabled={uploading}
                                />
                                {uploading && <p className="loading-text">上传中...</p>}
                            </div>

                            {uploadedPath && (
                                <>
                                    <div className="form-group">
                                        <label htmlFor="imagePath">已上传图片路径</label>
                                        <input
                                            id="imagePath"
                                            type="text"
                                            className="input"
                                            value={imagePath}
                                            readOnly
                                        />
                                    </div>

                                    <div className="form-group">
                                        <label className="checkbox-label">
                                            <input
                                                type="checkbox"
                                                checked={includePosition}
                                                onChange={(e) => setIncludePosition(e.target.checked)}
                                            />
                                            包含坐标信息
                                        </label>
                                    </div>

                                    <button
                                        className="btn"
                                        onClick={handleRecognize}
                                        disabled={loading}
                                    >
                                        {loading ? '识别中...' : '📝 文字识别'}
                                    </button>
                                </>
                            )}

                            {result && (
                                <div className="result-box">
                                    <h3>结果</h3>
                                    <pre className="result-content">{result}</pre>
                                </div>
                            )}
                        </div>
                    </div>
                )}

                {activeTab === 'history' && (
                    <div className="history-panel">
                        <div className="card">
                            <h2>识别历史</h2>
                            <p className="placeholder-text">
                                识别历史功能开发中...
                                <br />
                                将支持查看历史识别记录、导出结果等功能
                            </p>
                        </div>
                    </div>
                )}
            </div>
        </div>
    )
}
