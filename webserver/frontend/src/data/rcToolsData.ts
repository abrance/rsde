export interface RcClient {
    name: string
    description: string
    endpoint: string
    features: string[]
}

export interface RcApiExample {
    title: string
    description: string
    code: string
}

export interface RcToolMetadata {
    icon: string
    title: string
    subtitle: string
    path: string,
    description: string,
    coreCapabilities: string[]
    useCases: {
        title: string
        description: string
    }[]
    quickStart: {
        cli: string
        api: string
    }
    clients: {
        [key: string]: RcClient
    }
    apiExamples: {
        [key: string]: RcApiExample[]
    }
}

export const rcToolData: RcToolMetadata = {
    icon: '🛠️',
    title: 'RC - Remote Control',
    subtitle: '瑞士军刀型工具集，集成多种服务客户端和协议服务器',
    path: '/rc',
    description: '瑞士军刀型工具集，集成多种服务客户端和协议服务器',

    coreCapabilities: [
        '📨 消息队列客户端（Kafka）',
        '💾 数据库客户端（MySQL、Redis）',
        '📊 时序数据库（InfluxDB、VictoriaMetrics）',
        '🌐 协议服务器（HTTP、TCP、gRPC）',
        '📁 文件服务器（Samba）',
        '🔧 CLI 工具集成',
    ],

    useCases: [
        {
            title: '服务连通性测试',
            description: '快速检测 Kafka、MySQL、Redis 等服务连接状态',
        },
        {
            title: '数据操作',
            description: '执行数据查询、消息发送、键值操作等常见操作',
        },
        {
            title: '快速原型',
            description: '快速搭建测试用的 HTTP/TCP 服务器',
        },
    ],

    quickStart: {
        cli: `# CLI 方式：测试 Kafka 连通性
 rc kafka ping --brokers localhost:9092

 # CLI 方式：测试 Redis 连通性  
 rc redis ping -H localhost:6379

 # CLI 方式：测试 MySQL 连通性
 rc mysql ping -H localhost:3306`,
        api: `# HTTP API 方式 - Kafka
 curl -X POST http://localhost:3000/api/rc/kafka/ping \\
   -H "Content-Type: application/json" \\
   -d '{"brokers": ["localhost:9092"]}'

 # HTTP API 方式 - Redis
 curl -X POST http://localhost:3000/api/rc/redis/ping \\
   -H "Content-Type: application/json" \\
   -d '{"host": "localhost:6379"}'

 # HTTP API 方式 - MySQL  
 curl -X POST http://localhost:3000/api/rc/mysql/ping \\
   -H "Content-Type: application/json" \\
   -d '{"host": "localhost:3306"}'`,
    },

    clients: {
        kafka: {
            name: 'Kafka',
            description: 'Apache Kafka 消息队列客户端',
            endpoint: '/api/rc/kafka',
            features: [
                '✅ Ping 测试 - 检测集群连通性',
                '🔄 Metadata 查询 - 获取集群和 Topic 信息',
                '📊 支持 SASL 认证（PLAIN、SCRAM-SHA-256、SCRAM-SHA-512）',
                '🔐 支持 SSL/TLS 加密连接',
            ],
        },
        redis: {
            name: 'Redis',
            description: 'Redis 键值存储数据库客户端',
            endpoint: '/api/rc/redis',
            features: [
                '✅ Ping 测试 - 检测 Redis 服务连通性',
                '🔑 键值操作 - GET/SET/DEL 等基本操作',
                '📊 服务器信息 - 获取 Redis 版本和配置信息',
                '🔍 键搜索 - KEYS 命令支持模式匹配',
                '🔐 支持密码认证和 ACL 用户名',
                '🔐 支持 TLS/SSL 加密连接',
                '🔢 多数据库支持 - 可指定 DB 索引',
            ],
        },
        mysql: {
            name: 'MySQL',
            description: 'MySQL 关系型数据库客户端',
            endpoint: '/api/rc/mysql',
            features: [
                '✅ Ping 测试 - 检测 MySQL 服务连通性',
                '📊 SQL 查询 - 执行 DDL/DML 语句',
                '🔐 支持用户名密码认证',
                '🔐 支持 SSL/TLS 加密连接',
                '📋 数据库选择 - 可指定目标数据库',
                '📈 获取服务器版本信息',
            ],
        },
    },

    apiExamples: {
        kafka: [
            {
                title: 'Kafka 连通性测试',
                description: '测试 Kafka 集群连接状态，支持 SASL 认证',
                code: `POST /api/rc/kafka/ping
 Content-Type: application/json

 {
   "brokers": ["localhost:9092"],
   "client_id": "test-client",
   "timeout": 10,
   "sasl": false
 }

 // 使用 SASL 认证
 {
   "brokers": ["kafka:9093"],
   "sasl": true,
   "username": "admin",
   "password": "secret",
   "security_protocol": "SASL_SSL",
   "mechanism": "SCRAM-SHA-256"
 }`,
            },
        ],
        redis: [
            {
                title: 'Redis 连通性测试',
                description: '测试 Redis 服务器连接状态，支持密码认证和 TLS',
                code: `POST /api/rc/redis/ping
 Content-Type: application/json

 {
   "host": "localhost:6379",
   "password": "secret",
   "db": 0,
   "tls": false,
   "timeout": 10
 }`,
            },
            {
                title: 'Redis 键值操作',
                description: '执行 Redis GET/SET/DEL 等基本操作',
                code: `# 获取键值
 POST /api/rc/redis/get
 Content-Type: application/json

 {
   "host": "localhost:6379",
   "key": "my_key"
 }

 # 设置键值
 POST /api/rc/redis/set  
 Content-Type: application/json

 {
   "host": "localhost:6379",
   "key": "my_key",
   "value": "my_value",
   "ttl": 3600
 }

 # 删除键
 POST /api/rc/redis/del
 Content-Type: application/json

 {
   "host": "localhost:6379", 
   "key": "my_key"
 }`,
            },
        ],
        mysql: [
            {
                title: 'MySQL 连通性测试',
                description: '测试 MySQL 服务器连接状态，支持 SSL 和数据库选择',
                code: `POST /api/rc/mysql/ping
 Content-Type: application/json

 {
   "host": "localhost:3306",
   "username": "root",
   "password": "secret",
   "database": "test_db",
   "ssl": false,
   "timeout": 10
 }`,
            },
            {
                title: 'MySQL SQL 查询',
                description: '执行 MySQL DDL/DML 语句',
                code: `# 执行 DML 查询 (SELECT/INSERT/UPDATE/DELETE)
 POST /api/rc/mysql/query
 Content-Type: application/json

 {
   "host": "localhost:3306",
   "username": "root", 
   "password": "secret",
   "database": "test_db",
   "query": "SELECT * FROM users WHERE id = 1",
   "query_type": "dml"
 }

 # 执行 DDL 查询 (CREATE/ALTER/DROP)
 POST /api/rc/mysql/query
 Content-Type: application/json

 {
   "host": "localhost:3306",
   "username": "root",
   "password": "secret", 
   "database": "test_db",
   "query": "CREATE TABLE test (id INT, name VARCHAR(255))",
   "query_type": "ddl"
 }`,
            },
        ],
    },
}

export type RcTabType = 'overview' | 'kafka' | 'redis' | 'mysql'
