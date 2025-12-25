import { useState } from 'react'
import './ToolPage.css'

export default function RcPage() {
    const [activeTab, setActiveTab] = useState<'overview' | 'configs' | 'environments'>('overview')

    return (
        <div className="tool-page">
            <div className="page-header">
                <h1 className="page-title">
                    <span className="page-icon">⚙️</span>
                    RC - 远程配置管理
                </h1>
                <p className="page-description">
                    统一管理分布式系统配置，支持多环境和版本控制
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
                    className={`tab ${activeTab === 'configs' ? 'active' : ''}`}
                    onClick={() => setActiveTab('configs')}
                >
                    配置列表
                </button>
                <button
                    className={`tab ${activeTab === 'environments' ? 'active' : ''}`}
                    onClick={() => setActiveTab('environments')}
                >
                    环境管理
                </button>
            </div>

            <div className="tab-content">
                {activeTab === 'overview' && (
                    <div className="overview">
                        <div className="card">
                            <h2>核心功能</h2>
                            <ul className="feature-list">
                                <li>📦 集中式配置管理</li>
                                <li>🔄 配置版本控制</li>
                                <li>⚡ 动态配置更新</li>
                                <li>🌍 多环境支持（dev/staging/prod）</li>
                                <li>🔐 配置加密存储</li>
                                <li>📝 配置变更审计</li>
                            </ul>
                        </div>

                        <div className="card">
                            <h2>使用场景</h2>
                            <div className="use-cases">
                                <div className="use-case">
                                    <h3>微服务配置</h3>
                                    <p>统一管理多个微服务的配置文件</p>
                                </div>
                                <div className="use-case">
                                    <h3>功能开关</h3>
                                    <p>动态控制功能的开启和关闭</p>
                                </div>
                                <div className="use-case">
                                    <h3>A/B 测试</h3>
                                    <p>不同用户组使用不同的配置</p>
                                </div>
                            </div>
                        </div>

                        <div className="card">
                            <h2>快速开始</h2>
                            <div className="code-block">
                                <pre>{`# 启动 RC 服务
./rc --port 8081

# 获取配置
curl http://localhost:8081/config/my-service`}</pre>
                            </div>
                        </div>
                    </div>
                )}

                {activeTab === 'configs' && (
                    <div className="configs-panel">
                        <div className="card">
                            <h2>配置列表</h2>
                            <p className="placeholder-text">
                                配置列表界面开发中...
                                <br />
                                将支持查看、编辑、导入导出配置文件
                            </p>
                        </div>
                    </div>
                )}

                {activeTab === 'environments' && (
                    <div className="environments-panel">
                        <div className="card">
                            <h2>环境管理</h2>
                            <p className="placeholder-text">
                                环境管理界面开发中...
                                <br />
                                将支持多环境配置切换和环境变量管理
                            </p>
                        </div>
                    </div>
                )}
            </div>
        </div>
    )
}
