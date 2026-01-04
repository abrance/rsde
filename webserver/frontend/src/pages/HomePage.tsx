import { Link } from 'react-router-dom'
import './HomePage.css'

export default function HomePage() {
    const tools = [
        // {
        //     name: 'Rsync',
        //     path: '/rsync',
        //     icon: '🔄',
        //     description: '高性能数据同步工具，支持规则引擎和多种传输协议',
        //     features: [
        //         '基于规则的文件同步',
        //         '支持本地和远程同步',
        //         '实时文件监控',
        //         '灵活的配置管理',
        //     ],
        // },
        // {
        //     name: 'RC',
        //     path: '/rc',
        //     icon: '⚙️',
        //     description: '远程配置管理工具，统一管理分布式系统配置',
        //     features: [
        //         '集中式配置管理',
        //         '配置版本控制',
        //         '动态配置更新',
        //         '多环境支持',
        //     ],
        // },
        {
            name: 'Anybox',
            path: '/anybox',
            icon: '📦',
            description: '多功能文件存储和分享服务，支持多种存储后端',
            features: [
                '匿名发帖',
                '文件分享和权限管理',
            ],
        },
        {
            name: 'OCR',
            path: '/ocr',
            icon: '📝',
            description: '图片文字识别服务，支持多种语言和格式',
            features: [
                '远程 OCR 服务',
                '多语言支持',
                '坐标信息提取',
                '批量处理',
            ],
        },
    ]

    return (
        <div className="home-page">
            <section className="hero">
                <h1 className="hero-title">
                    <span className="hero-icon">&#127758;</span>
                    xy planet
                </h1>
                <p className="hero-subtitle">xy planet</p>
                <p className="hero-description">
                    这里是小歪星球，提供数据同步、配置管理、OCR 识别等功能
                </p>
            </section>

            <section className="tools-section">
                <h2 className="section-title">工具集</h2>
                <div className="tools-grid">
                    {tools.map((tool) => (
                        <div key={tool.name} className="tool-card">
                            <div className="tool-icon">{tool.icon}</div>
                            <h3 className="tool-name">{tool.name}</h3>
                            <p className="tool-description">{tool.description}</p>
                            <ul className="tool-features">
                                {tool.features.map((feature, index) => (
                                    <li key={index}>{feature}</li>
                                ))}
                            </ul>
                            <Link to={tool.path} className="btn tool-link">
                                开始使用 →
                            </Link>
                        </div>
                    ))}
                </div>
            </section>

            <section className="features-section">
                <h2 className="section-title">核心特性</h2>
                <div className="features-grid">
                    <div className="feature-item">
                        <div className="feature-icon">⚡</div>
                        <h3>高性能</h3>
                        <p>使用 Rust 编写，零成本抽象，内存安全</p>
                    </div>
                    <div className="feature-item">
                        <div className="feature-icon">🔧</div>
                        <h3>易配置</h3>
                        <p>TOML 配置文件，简单直观，易于维护</p>
                    </div>
                    <div className="feature-item">
                        <div className="feature-icon">🐳</div>
                        <h3>容器化</h3>
                        <p>支持 Docker 部署，开箱即用</p>
                    </div>
                    <div className="feature-item">
                        <div className="feature-icon">🔒</div>
                        <h3>可靠性</h3>
                        <p>完善的错误处理和日志系统</p>
                    </div>
                </div>
            </section>
        </div>
    )
}
