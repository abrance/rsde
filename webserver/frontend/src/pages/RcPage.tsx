import { useState } from 'react'
import { rcToolData, RcTabType } from '../data'
import './ToolPage.css'

interface KafkaPingRequest {
    brokers: string[]
    client_id: string
    timeout: number
    sasl: boolean
    username?: string
    password?: string
    security_protocol: string
    mechanism: string
    topic?: string
}

interface KafkaPingResponse {
    success: boolean
    brokers: string[]
    client_id: string
    sasl_enabled: boolean
    username?: string
    security_protocol?: string
    mechanism?: string
    cluster_name?: string
    broker_count?: number
    topic_count?: number
    topic?: string
    partition_count?: number
    error?: string
}

export default function RcPage() {
    const [activeTab, setActiveTab] = useState<RcTabType>('overview')

    // Kafka Ping 表单状态
    const [brokers, setBrokers] = useState('localhost:9092')
    const [clientId, setClientId] = useState('rc-web-client')
    const [timeout, setTimeout] = useState(10)
    const [enableSasl, setEnableSasl] = useState(false)
    const [username, setUsername] = useState('')
    const [password, setPassword] = useState('')
    const [securityProtocol, setSecurityProtocol] = useState('SASL_PLAINTEXT')
    const [mechanism, setMechanism] = useState('PLAIN')
    const [topic, setTopic] = useState('')

    const [loading, setLoading] = useState(false)
    const [pingResult, setPingResult] = useState<KafkaPingResponse | null>(null)

    const handleKafkaPing = async () => {
        setLoading(true)
        setPingResult(null)

        const requestBody: KafkaPingRequest = {
            brokers: brokers.split(',').map(b => b.trim()),
            client_id: clientId,
            timeout,
            sasl: enableSasl,
            security_protocol: securityProtocol,
            mechanism,
        }

        if (enableSasl) {
            requestBody.username = username
            requestBody.password = password
        }

        if (topic.trim()) {
            requestBody.topic = topic.trim()
        }

        try {
            const response = await fetch('/api/rc/kafka/ping', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify(requestBody),
            })

            const data = await response.json()
            setPingResult(data)
        } catch (error) {
            setPingResult({
                success: false,
                brokers: requestBody.brokers,
                client_id: clientId,
                sasl_enabled: enableSasl,
                error: `请求失败: ${error}`,
            })
        } finally {
            setLoading(false)
        }
    }

    return (
        <div className="tool-page">
            <div className="page-header">
                <h1 className="page-title">
                    <span className="page-icon">{rcToolData.icon}</span>
                    {rcToolData.title}
                </h1>
                <p className="page-description">
                    {rcToolData.subtitle}
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
                    className={`tab ${activeTab === 'kafka' ? 'active' : ''}`}
                    onClick={() => setActiveTab('kafka')}
                >
                    {rcToolData.clients.kafka.name} 客户端
                </button>
                <button
                    className={`tab ${activeTab === 'database' ? 'active' : ''}`}
                    onClick={() => setActiveTab('database')}
                >
                    {rcToolData.clients.database.name}工具
                </button>
            </div>

            <div className="tab-content">
                {activeTab === 'overview' && (
                    <div className="overview">
                        <div className="card">
                            <h2>核心能力</h2>
                            <ul className="feature-list">
                                {rcToolData.coreCapabilities.map((capability, index) => (
                                    <li key={index}>{capability}</li>
                                ))}
                            </ul>
                        </div>

                        <div className="card">
                            <h2>使用场景</h2>
                            <div className="use-cases">
                                {rcToolData.useCases.map((useCase, index) => (
                                    <div key={index} className="use-case">
                                        <h3>{useCase.title}</h3>
                                        <p>{useCase.description}</p>
                                    </div>
                                ))}
                            </div>
                        </div>

                        <div className="card">
                            <h2>快速开始</h2>
                            <div className="code-block">
                                <pre>{`${rcToolData.quickStart.cli}\n\n${rcToolData.quickStart.api}`}</pre>
                            </div>
                        </div>
                    </div>
                )}

                {activeTab === 'kafka' && (
                    <div className="kafka-panel">
                        <div className="card">
                            <h2>Kafka Ping 测试</h2>
                            <div className="form-container">
                                <div className="form-group">
                                    <label>Broker 地址（逗号分隔）</label>
                                    <input
                                        type="text"
                                        value={brokers}
                                        onChange={(e) => setBrokers(e.target.value)}
                                        placeholder="localhost:9092,localhost:9093"
                                        className="form-input"
                                    />
                                </div>

                                <div className="form-row">
                                    <div className="form-group">
                                        <label>Client ID</label>
                                        <input
                                            type="text"
                                            value={clientId}
                                            onChange={(e) => setClientId(e.target.value)}
                                            className="form-input"
                                        />
                                    </div>
                                    <div className="form-group">
                                        <label>超时时间（秒）</label>
                                        <input
                                            type="number"
                                            value={timeout}
                                            onChange={(e) => setTimeout(Number(e.target.value))}
                                            className="form-input"
                                        />
                                    </div>
                                </div>

                                <div className="form-group">
                                    <label>Topic（可选，用于获取 Topic 元数据）</label>
                                    <input
                                        type="text"
                                        value={topic}
                                        onChange={(e) => setTopic(e.target.value)}
                                        placeholder="my-topic"
                                        className="form-input"
                                    />
                                </div>

                                <div className="form-group">
                                    <label className="checkbox-label">
                                        <input
                                            type="checkbox"
                                            checked={enableSasl}
                                            onChange={(e) => setEnableSasl(e.target.checked)}
                                        />
                                        <span>启用 SASL 认证</span>
                                    </label>
                                </div>

                                {enableSasl && (
                                    <div className="sasl-config">
                                        <div className="form-row">
                                            <div className="form-group">
                                                <label>用户名</label>
                                                <input
                                                    type="text"
                                                    value={username}
                                                    onChange={(e) => setUsername(e.target.value)}
                                                    className="form-input"
                                                />
                                            </div>
                                            <div className="form-group">
                                                <label>密码</label>
                                                <input
                                                    type="password"
                                                    value={password}
                                                    onChange={(e) => setPassword(e.target.value)}
                                                    className="form-input"
                                                />
                                            </div>
                                        </div>
                                        <div className="form-row">
                                            <div className="form-group">
                                                <label>Security Protocol</label>
                                                <select
                                                    value={securityProtocol}
                                                    onChange={(e) => setSecurityProtocol(e.target.value)}
                                                    className="form-input"
                                                >
                                                    <option value="SASL_PLAINTEXT">SASL_PLAINTEXT</option>
                                                    <option value="SASL_SSL">SASL_SSL</option>
                                                </select>
                                            </div>
                                            <div className="form-group">
                                                <label>Mechanism</label>
                                                <select
                                                    value={mechanism}
                                                    onChange={(e) => setMechanism(e.target.value)}
                                                    className="form-input"
                                                >
                                                    <option value="PLAIN">PLAIN</option>
                                                    <option value="SCRAM-SHA-256">SCRAM-SHA-256</option>
                                                    <option value="SCRAM-SHA-512">SCRAM-SHA-512</option>
                                                </select>
                                            </div>
                                        </div>
                                    </div>
                                )}

                                <button
                                    onClick={handleKafkaPing}
                                    disabled={loading || !brokers.trim()}
                                    className="btn btn-primary"
                                >
                                    {loading ? '测试中...' : 'Ping 测试'}
                                </button>
                            </div>

                            {pingResult && (
                                <div className={`result-panel ${pingResult.success ? 'success' : 'error'}`}>
                                    <h3>{pingResult.success ? '✅ 连接成功' : '❌ 连接失败'}</h3>

                                    <div className="result-details">
                                        <div className="result-item">
                                            <strong>Brokers:</strong> {pingResult.brokers.join(', ')}
                                        </div>
                                        <div className="result-item">
                                            <strong>Client ID:</strong> {pingResult.client_id}
                                        </div>
                                        {pingResult.sasl_enabled && (
                                            <>
                                                <div className="result-item">
                                                    <strong>SASL 用户:</strong> {pingResult.username}
                                                </div>
                                                <div className="result-item">
                                                    <strong>认证协议:</strong> {pingResult.security_protocol}
                                                </div>
                                                <div className="result-item">
                                                    <strong>认证机制:</strong> {pingResult.mechanism}
                                                </div>
                                            </>
                                        )}
                                        {pingResult.cluster_name && (
                                            <div className="result-item">
                                                <strong>集群名称:</strong> {pingResult.cluster_name}
                                            </div>
                                        )}
                                        {pingResult.broker_count !== undefined && (
                                            <div className="result-item">
                                                <strong>Broker 数量:</strong> {pingResult.broker_count}
                                            </div>
                                        )}
                                        {pingResult.topic_count !== undefined && (
                                            <div className="result-item">
                                                <strong>Topic 数量:</strong> {pingResult.topic_count}
                                            </div>
                                        )}
                                        {pingResult.topic && (
                                            <div className="result-item">
                                                <strong>Topic:</strong> {pingResult.topic}
                                            </div>
                                        )}
                                        {pingResult.partition_count !== undefined && (
                                            <div className="result-item">
                                                <strong>Partition 数量:</strong> {pingResult.partition_count}
                                            </div>
                                        )}
                                        {pingResult.error && (
                                            <div className="result-item error-message">
                                                <strong>错误信息:</strong> {pingResult.error}
                                            </div>
                                        )}
                                    </div>
                                </div>
                            )}
                        </div>

                        {rcToolData.apiExamples.kafka.map((example, index) => (
                            <div key={index} className="card">
                                <h2>{example.title}</h2>
                                <p className="info-text">
                                    {example.description}
                                </p>
                                <div className="code-block">
                                    <pre>{example.code}</pre>
                                </div>
                            </div>
                        ))}

                        <div className="card">
                            <h2>功能列表</h2>
                            <ul className="feature-list">
                                {rcToolData.clients.kafka.features.map((feature, index) => (
                                    <li key={index}>{feature}</li>
                                ))}
                            </ul>
                        </div>
                    </div>
                )}

                {activeTab === 'database' && (
                    <div className="database-panel">
                        <div className="card">
                            <h2>{rcToolData.clients.database.name}工具</h2>
                            <p className="placeholder-text">
                                数据库客户端功能开发中...
                                <br />
                                即将支持 MySQL、Redis、InfluxDB 等数据库的连接测试和基本操作
                            </p>
                        </div>
                    </div>
                )}
            </div>
        </div>
    )
}
