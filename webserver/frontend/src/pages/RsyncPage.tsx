import { useState } from 'react'
import './ToolPage.css'

export default function RsyncPage() {
    const [activeTab, setActiveTab] = useState<'overview' | 'config' | 'monitor'>('overview')

    return (
        <div className="tool-page">
            <div className="page-header">
                <h1 className="page-title">
                    <span className="page-icon">🔄</span>
                    Rsync - 数据同步工具
                </h1>
                <p className="page-description">
                    高性能数据同步工具，支持规则引擎和多种传输协议
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
                    className={`tab ${activeTab === 'config' ? 'active' : ''}`}
                    onClick={() => setActiveTab('config')}
                >
                    配置
                </button>
                <button
                    className={`tab ${activeTab === 'monitor' ? 'active' : ''}`}
                    onClick={() => setActiveTab('monitor')}
                >
                    监控
                </button>
            </div>

            <div className="tab-content">
                {activeTab === 'overview' && (
                    <div className="overview">
                        <div className="card">
                            <h2>功能特性</h2>
                            <ul className="feature-list">
                                <li>✨ 基于规则的文件同步</li>
                                <li>🚀 支持本地和远程同步</li>
                                <li>👁️ 实时文件监控</li>
                                <li>⚙️ 灵活的配置管理</li>
                                <li>📊 详细的同步日志</li>
                                <li>🔧 支持增量同步</li>
                            </ul>
                        </div>

                        <div className="card">
                            <h2>快速开始</h2>
                            <div className="code-block">
                                <pre>{`# 启动 rsync 服务
./rsync --config config.toml

# 使用 Docker 运行
docker run -v ./config:/app/config rsde/rsync`}</pre>
                            </div>
                        </div>

                        <div className="card">
                            <h2>配置示例</h2>
                            <div className="code-block">
                                <pre>{`[global]
log_level = "info"
watch_interval = 5000

[[pipeline]]
name = "my-sync"
source = "/data/source"
target = "/data/target"
rules = ["*.log", "*.txt"]`}</pre>
                            </div>
                        </div>
                    </div>
                )}

                {activeTab === 'config' && (
                    <div className="config-panel">
                        <div className="card">
                            <h2>配置管理</h2>
                            <p className="placeholder-text">
                                配置管理界面开发中...
                                <br />
                                将支持可视化编辑同步规则、查看配置历史等功能
                            </p>
                        </div>
                    </div>
                )}

                {activeTab === 'monitor' && (
                    <div className="monitor-panel">
                        <div className="card">
                            <h2>实时监控</h2>
                            <p className="placeholder-text">
                                监控面板开发中...
                                <br />
                                将支持查看同步状态、传输速率、错误日志等
                            </p>
                        </div>
                    </div>
                )}
            </div>
        </div>
    )
}
