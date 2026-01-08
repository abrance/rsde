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
            description: '执行数据查询、消息发送等常见操作',
        },
        {
            title: '快速原型',
            description: '快速搭建测试用的 HTTP/TCP 服务器',
        },
    ],

    quickStart: {
        cli: `# CLI 方式：测试 Kafka 连通性
rc kafka ping --brokers localhost:9092`,
        api: `# HTTP API 方式
curl -X POST http://localhost:3000/api/rc/kafka/ping \\
  -H "Content-Type: application/json" \\
  -d '{"brokers": ["localhost:9092"]}'`,
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
        database: {
            name: '数据库',
            description: 'MySQL、Redis、InfluxDB 等数据库客户端',
            endpoint: '/api/rc/database',
            features: [
                '🔄 连接测试',
                '📊 基本查询操作',
                '💾 数据导入导出',
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
    },
}

export type RcTabType = 'overview' | 'kafka' | 'database'
