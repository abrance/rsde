import { Link } from 'react-router-dom'
import { toolsData, featuresData } from '../data'
import './HomePage.css'

export default function HomePage() {
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
                    {toolsData.map((tool) => (
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
                    {featuresData.map((feature, index) => (
                        <div key={index} className="feature-item">
                            <div className="feature-icon">{feature.icon}</div>
                            <h3>{feature.title}</h3>
                            <p>{feature.description}</p>
                        </div>
                    ))}
                </div>
            </section>
        </div>
    )
}
