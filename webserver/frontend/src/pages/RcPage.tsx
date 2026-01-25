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

interface RedisPingRequest {
    host: string
    password?: string
    username?: string
    db: number
    timeout: number
    tls: boolean
}

interface RedisPingResponse {
    success: boolean
    url: string
    db: number
    version?: string
    dbsize?: number
    error?: string
}

interface RedisGetRequest {
    host: string
    key: string
    password?: string
    username?: string
    db: number
    tls: boolean
}

interface RedisGetResponse {
    key: string
    value: string | null
    exists: boolean
}

interface RedisSetRequest {
    host: string
    key: string
    value: string
    password?: string
    username?: string
    db: number
    ttl?: number
    tls: boolean
}

interface RedisSetResponse {
    success: boolean
    key: string
    ttl?: number
}

interface MySqlPingRequest {
    host: string
    username?: string
    password?: string
    database?: string
    timeout: number
    ssl: boolean
}

interface MySqlPingResponse {
    success: boolean
    host: string
    port: number
    database?: string
    version?: string
    error?: string
}

interface MySqlQueryRequest {
    host: string
    query: string
    query_type: string
    username?: string
    password?: string
    database?: string
    timeout: number
    ssl: boolean
}

interface MySqlQueryResponse {
    success: boolean
    rows_affected?: number
    message?: string
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

    // Redis 表单状态
    const [redisHost, setRedisHost] = useState('localhost:6379')
    const [redisPassword, setRedisPassword] = useState('')
    const [redisUsername, setRedisUsername] = useState('')
    const [redisDb, setRedisDb] = useState(0)
    const [redisTimeout, setRedisTimeout] = useState(10)
    const [redisTls, setRedisTls] = useState(false)
    const [redisKey, setRedisKey] = useState('')
    const [redisValue, setRedisValue] = useState('')
    const [redisTtl, setRedisTtl] = useState<number | ''>('')
    
    const [redisPingResult, setRedisPingResult] = useState<RedisPingResponse | null>(null)
    const [redisGetResult, setRedisGetResult] = useState<RedisGetResponse | null>(null)
    const [redisSetResult, setRedisSetResult] = useState<RedisSetResponse | null>(null)

    // MySQL 表单状态
    const [mysqlHost, setMysqlHost] = useState('localhost:3306')
    const [mysqlUsername, setMysqlUsername] = useState('root')
    const [mysqlPassword, setMysqlPassword] = useState('')
    const [mysqlDatabase, setMysqlDatabase] = useState('')
    const [mysqlTimeout, setMysqlTimeout] = useState(10)
    const [mysqlSsl, setMysqlSsl] = useState(false)
    const [mysqlQuery, setMysqlQuery] = useState('SELECT 1')
    const [mysqlQueryType, setMysqlQueryType] = useState('dml')
    
    const [mysqlPingResult, setMysqlPingResult] = useState<MySqlPingResponse | null>(null)
    const [mysqlQueryResult, setMysqlQueryResult] = useState<MySqlQueryResponse | null>(null)

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

    const handleRedisPing = async () => {
        setLoading(true)
        setRedisPingResult(null)

        const requestBody: RedisPingRequest = {
            host: redisHost,
            db: redisDb,
            timeout: redisTimeout,
            tls: redisTls,
        }

        if (redisPassword) {
            requestBody.password = redisPassword
        }

        if (redisUsername) {
            requestBody.username = redisUsername
        }

        try {
            const response = await fetch('/api/rc/redis/ping', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify(requestBody),
            })

            const data = await response.json()
            setRedisPingResult(data)
        } catch (error) {
            setRedisPingResult({
                success: false,
                url: redisHost,
                db: redisDb,
                error: `请求失败: ${error}`,
            })
        } finally {
            setLoading(false)
        }
    }

    const handleRedisGet = async () => {
        setLoading(true)
        setRedisGetResult(null)

        const requestBody: RedisGetRequest = {
            host: redisHost,
            key: redisKey,
            db: redisDb,
            tls: redisTls,
        }

        if (redisPassword) {
            requestBody.password = redisPassword
        }

        if (redisUsername) {
            requestBody.username = redisUsername
        }

        try {
            const response = await fetch('/api/rc/redis/get', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify(requestBody),
            })

            const data = await response.json()
            setRedisGetResult(data)
        } catch (error) {
            setRedisGetResult({
                key: redisKey,
                value: null,
                exists: false,
            })
        } finally {
            setLoading(false)
        }
    }

    const handleRedisSet = async () => {
        setLoading(true)
        setRedisSetResult(null)

        const requestBody: RedisSetRequest = {
            host: redisHost,
            key: redisKey,
            value: redisValue,
            db: redisDb,
            tls: redisTls,
        }

        if (redisPassword) {
            requestBody.password = redisPassword
        }

        if (redisUsername) {
            requestBody.username = redisUsername
        }

        if (redisTtl !== '') {
            requestBody.ttl = Number(redisTtl)
        }

        try {
            const response = await fetch('/api/rc/redis/set', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify(requestBody),
            })

            const data = await response.json()
            setRedisSetResult(data)
        } catch (error) {
            setRedisSetResult({
                success: false,
                key: redisKey,
                ttl: redisTtl !== '' ? Number(redisTtl) : undefined,
            })
        } finally {
            setLoading(false)
        }
    }

    const handleMySqlPing = async () => {
        setLoading(true)
        setMysqlPingResult(null)

        const requestBody: MySqlPingRequest = {
            host: mysqlHost,
            timeout: mysqlTimeout,
            ssl: mysqlSsl,
        }

        if (mysqlUsername) {
            requestBody.username = mysqlUsername
        }

        if (mysqlPassword) {
            requestBody.password = mysqlPassword
        }

        if (mysqlDatabase) {
            requestBody.database = mysqlDatabase
        }

        try {
            const response = await fetch('/api/rc/mysql/ping', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify(requestBody),
            })

            const data = await response.json()
            setMysqlPingResult(data)
        } catch (error) {
            setMysqlPingResult({
                success: false,
                host: mysqlHost,
                port: 3306,
                error: `请求失败: ${error}`,
            })
        } finally {
            setLoading(false)
        }
    }

    const handleMySqlQuery = async () => {
        setLoading(true)
        setMysqlQueryResult(null)

        const requestBody: MySqlQueryRequest = {
            host: mysqlHost,
            query: mysqlQuery,
            query_type: mysqlQueryType,
            timeout: mysqlTimeout,
            ssl: mysqlSsl,
        }

        if (mysqlUsername) {
            requestBody.username = mysqlUsername
        }

        if (mysqlPassword) {
            requestBody.password = mysqlPassword
        }

        if (mysqlDatabase) {
            requestBody.database = mysqlDatabase
        }

        try {
            const response = await fetch('/api/rc/mysql/query', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify(requestBody),
            })

            const data = await response.json()
            setMysqlQueryResult(data)
        } catch (error) {
            setMysqlQueryResult({
                success: false,
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
                    className={`tab ${activeTab === 'redis' ? 'active' : ''}`}
                    onClick={() => setActiveTab('redis')}
                >
                    {rcToolData.clients.redis.name} 客户端
                </button>
                <button
                    className={`tab ${activeTab === 'mysql' ? 'active' : ''}`}
                    onClick={() => setActiveTab('mysql')}
                >
                    {rcToolData.clients.mysql.name} 客户端
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

                {activeTab === 'redis' && (
                    <div className="redis-panel">
                        <div className="card">
                            <h2>Redis 连接测试</h2>
                            <div className="form-container">
                                <div className="form-group">
                                    <label>Redis 服务器地址</label>
                                    <input
                                        type="text"
                                        value={redisHost}
                                        onChange={(e) => setRedisHost(e.target.value)}
                                        placeholder="localhost:6379 或 redis://host:port"
                                        className="form-input"
                                    />
                                </div>

                                <div className="form-row">
                                    <div className="form-group">
                                        <label>数据库索引</label>
                                        <input
                                            type="number"
                                            value={redisDb}
                                            onChange={(e) => setRedisDb(Number(e.target.value))}
                                            className="form-input"
                                        />
                                    </div>
                                    <div className="form-group">
                                        <label>超时时间（秒）</label>
                                        <input
                                            type="number"
                                            value={redisTimeout}
                                            onChange={(e) => setRedisTimeout(Number(e.target.value))}
                                            className="form-input"
                                        />
                                    </div>
                                </div>

                                <div className="form-group">
                                    <label className="checkbox-label">
                                        <input
                                            type="checkbox"
                                            checked={redisTls}
                                            onChange={(e) => setRedisTls(e.target.checked)}
                                        />
                                        <span>启用 TLS/SSL 加密</span>
                                    </label>
                                </div>

                                <div className="form-row">
                                    <div className="form-group">
                                        <label>用户名（Redis 6.0+ ACL）</label>
                                        <input
                                            type="text"
                                            value={redisUsername}
                                            onChange={(e) => setRedisUsername(e.target.value)}
                                            className="form-input"
                                        />
                                    </div>
                                    <div className="form-group">
                                        <label>密码</label>
                                        <input
                                            type="password"
                                            value={redisPassword}
                                            onChange={(e) => setRedisPassword(e.target.value)}
                                            className="form-input"
                                        />
                                    </div>
                                </div>

                                <button
                                    onClick={handleRedisPing}
                                    disabled={loading || !redisHost.trim()}
                                    className="btn btn-primary"
                                >
                                    {loading ? '测试中...' : 'Ping 测试'}
                                </button>
                            </div>

                            {redisPingResult && (
                                <div className={`result-panel ${redisPingResult.success ? 'success' : 'error'}`}>
                                    <h3>{redisPingResult.success ? '✅ 连接成功' : '❌ 连接失败'}</h3>
                                    <div className="result-details">
                                        <div className="result-item">
                                            <strong>服务器地址:</strong> {redisPingResult.url}
                                        </div>
                                        <div className="result-item">
                                            <strong>数据库:</strong> {redisPingResult.db}
                                        </div>
                                        {redisPingResult.version && (
                                            <div className="result-item">
                                                <strong>Redis 版本:</strong> {redisPingResult.version}
                                            </div>
                                        )}
                                        {redisPingResult.dbsize !== undefined && (
                                            <div className="result-item">
                                                <strong>键数量:</strong> {redisPingResult.dbsize}
                                            </div>
                                        )}
                                        {redisPingResult.error && (
                                            <div className="result-item error-message">
                                                <strong>错误信息:</strong> {redisPingResult.error}
                                            </div>
                                        )}
                                    </div>
                                </div>
                            )}
                        </div>

                        <div className="card">
                            <h2>Redis 键值操作</h2>
                            <div className="form-container">
                                <div className="form-group">
                                    <label>键名</label>
                                    <input
                                        type="text"
                                        value={redisKey}
                                        onChange={(e) => setRedisKey(e.target.value)}
                                        placeholder="my_key"
                                        className="form-input"
                                    />
                                </div>

                                <div className="form-row">
                                    <div className="form-group">
                                        <button
                                            onClick={handleRedisGet}
                                            disabled={loading || !redisKey.trim()}
                                            className="btn btn-secondary"
                                        >
                                            {loading ? '获取中...' : 'GET'}
                                        </button>
                                    </div>
                                    <div className="form-group">
                                        <input
                                            type="text"
                                            value={redisValue}
                                            onChange={(e) => setRedisValue(e.target.value)}
                                            placeholder="键值"
                                            className="form-input"
                                        />
                                        <input
                                            type="number"
                                            value={redisTtl as number}
                                            onChange={(e) => setRedisTtl(e.target.value ? Number(e.target.value) : '')}
                                            placeholder="TTL (秒)"
                                            className="form-input mt-2"
                                        />
                                        <button
                                            onClick={handleRedisSet}
                                            disabled={loading || !redisKey.trim() || !redisValue.trim()}
                                            className="btn btn-primary mt-2"
                                        >
                                            {loading ? '设置中...' : 'SET'}
                                        </button>
                                    </div>
                                </div>

                                {(redisGetResult || redisSetResult) && (
                                    <div className="result-panel info">
                                        {redisGetResult && (
                                            <div className="result-item">
                                                <strong>GET 结果:</strong> 
                                                {redisGetResult.exists ? `"${redisGetResult.value}"` : '(nil)'}
                                            </div>
                                        )}
                                        {redisSetResult && (
                                            <div className="result-item">
                                                <strong>SET 结果:</strong> 
                                                {redisSetResult.success ? '✅ 成功' : '❌ 失败'}
                                                {redisSetResult.ttl && ` (TTL: ${redisSetResult.ttl}秒)`}
                                            </div>
                                        )}
                                    </div>
                                )}
                            </div>
                        </div>

                        {rcToolData.apiExamples.redis.map((example, index) => (
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
                                {rcToolData.clients.redis.features.map((feature, index) => (
                                    <li key={index}>{feature}</li>
                                ))}
                            </ul>
                        </div>
                    </div>
                )}

                {activeTab === 'mysql' && (
                    <div className="mysql-panel">
                        <div className="card">
                            <h2>MySQL 连接测试</h2>
                            <div className="form-container">
                                <div className="form-group">
                                    <label>MySQL 服务器地址</label>
                                    <input
                                        type="text"
                                        value={mysqlHost}
                                        onChange={(e) => setMysqlHost(e.target.value)}
                                        placeholder="localhost:3306"
                                        className="form-input"
                                    />
                                </div>

                                <div className="form-row">
                                    <div className="form-group">
                                        <label>用户名</label>
                                        <input
                                            type="text"
                                            value={mysqlUsername}
                                            onChange={(e) => setMysqlUsername(e.target.value)}
                                            className="form-input"
                                        />
                                    </div>
                                    <div className="form-group">
                                        <label>密码</label>
                                        <input
                                            type="password"
                                            value={mysqlPassword}
                                            onChange={(e) => setMysqlPassword(e.target.value)}
                                            className="form-input"
                                        />
                                    </div>
                                </div>

                                <div className="form-row">
                                    <div className="form-group">
                                        <label>数据库</label>
                                        <input
                                            type="text"
                                            value={mysqlDatabase}
                                            onChange={(e) => setMysqlDatabase(e.target.value)}
                                            placeholder="可选"
                                            className="form-input"
                                        />
                                    </div>
                                    <div className="form-group">
                                        <label>超时时间（秒）</label>
                                        <input
                                            type="number"
                                            value={mysqlTimeout}
                                            onChange={(e) => setMysqlTimeout(Number(e.target.value))}
                                            className="form-input"
                                        />
                                    </div>
                                </div>

                                <div className="form-group">
                                    <label className="checkbox-label">
                                        <input
                                            type="checkbox"
                                            checked={mysqlSsl}
                                            onChange={(e) => setMysqlSsl(e.target.checked)}
                                        />
                                        <span>启用 SSL/TLS 加密</span>
                                    </label>
                                </div>

                                <button
                                    onClick={handleMySqlPing}
                                    disabled={loading || !mysqlHost.trim()}
                                    className="btn btn-primary"
                                >
                                    {loading ? '测试中...' : 'Ping 测试'}
                                </button>
                            </div>

                            {mysqlPingResult && (
                                <div className={`result-panel ${mysqlPingResult.success ? 'success' : 'error'}`}>
                                    <h3>{mysqlPingResult.success ? '✅ 连接成功' : '❌ 连接失败'}</h3>
                                    <div className="result-details">
                                        <div className="result-item">
                                            <strong>服务器地址:</strong> {mysqlPingResult.host}:{mysqlPingResult.port}
                                        </div>
                                        {mysqlPingResult.database && (
                                            <div className="result-item">
                                                <strong>数据库:</strong> {mysqlPingResult.database}
                                            </div>
                                        )}
                                        {mysqlPingResult.version && (
                                            <div className="result-item">
                                                <strong>MySQL 版本:</strong> {mysqlPingResult.version}
                                            </div>
                                        )}
                                        {mysqlPingResult.error && (
                                            <div className="result-item error-message">
                                                <strong>错误信息:</strong> {mysqlPingResult.error}
                                            </div>
                                        )}
                                    </div>
                                </div>
                            )}
                        </div>

                        <div className="card">
                            <h2>MySQL SQL 查询</h2>
                            <div className="form-container">
                                <div className="form-group">
                                    <label>SQL 查询</label>
                                    <textarea
                                        value={mysqlQuery}
                                        onChange={(e) => setMysqlQuery(e.target.value)}
                                        placeholder="SELECT * FROM users WHERE id = 1"
                                        className="form-input"
                                        rows={4}
                                    />
                                </div>

                                <div className="form-row">
                                    <div className="form-group">
                                        <label>查询类型</label>
                                        <select
                                            value={mysqlQueryType}
                                            onChange={(e) => setMysqlQueryType(e.target.value)}
                                            className="form-input"
                                        >
                                            <option value="dml">DML (SELECT/INSERT/UPDATE/DELETE)</option>
                                            <option value="ddl">DDL (CREATE/ALTER/DROP)</option>
                                        </select>
                                    </div>
                                    <div className="form-group">
                                        <button
                                            onClick={handleMySqlQuery}
                                            disabled={loading || !mysqlQuery.trim()}
                                            className="btn btn-primary"
                                        >
                                            {loading ? '执行中...' : '执行查询'}
                                        </button>
                                    </div>
                                </div>

                                {mysqlQueryResult && (
                                    <div className={`result-panel ${mysqlQueryResult.success ? 'success' : 'error'}`}>
                                        <h3>{mysqlQueryResult.success ? '✅ 查询成功' : '❌ 查询失败'}</h3>
                                        <div className="result-details">
                                            {mysqlQueryResult.success && mysqlQueryResult.rows_affected !== undefined && (
                                                <div className="result-item">
                                                    <strong>影响行数:</strong> {mysqlQueryResult.rows_affected}
                                                </div>
                                            )}
                                            {mysqlQueryResult.success && mysqlQueryResult.message && (
                                                <div className="result-item">
                                                    <strong>消息:</strong> {mysqlQueryResult.message}
                                                </div>
                                            )}
                                            {mysqlQueryResult.error && (
                                                <div className="result-item error-message">
                                                    <strong>错误信息:</strong> {mysqlQueryResult.error}
                                                </div>
                                            )}
                                        </div>
                                    </div>
                                )}
                            </div>
                        </div>

                        {rcToolData.apiExamples.mysql.map((example, index) => (
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
                                {rcToolData.clients.mysql.features.map((feature, index) => (
                                    <li key={index}>{feature}</li>
                                ))}
                            </ul>
                        </div>
                    </div>
                )}
            </div>
        </div>
    )
}
