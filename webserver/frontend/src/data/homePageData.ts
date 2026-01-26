import { rcToolData } from "./rcToolsData"


export interface ToolCardData {
    name: string
    path: string
    icon: string
    description: string
    features: string[]
}

export const toolsData: ToolCardData[] = [
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
    {
        name: rcToolData.title,
        path: rcToolData.path,
        icon: rcToolData.icon,
        description: rcToolData.description,
        features: rcToolData.coreCapabilities,
    },
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
        name: 'Prompt',
        path: '/prompt',
        icon: '💬',
        description: 'AI Prompt 模板管理工具，支持版本控制和分类',
        features: [
            'Prompt 模板 CRUD',
            '分类和标签管理',
            '版本控制',
            '变量占位符支持',
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

export interface FeatureCardData {
    icon: string
    title: string
    description: string
}

export const featuresData: FeatureCardData[] = [
    {
        icon: '⚡',
        title: '高性能',
        description: '使用 Rust 编写，零成本抽象，内存安全',
    },
    {
        icon: '🔧',
        title: '易配置',
        description: 'TOML 配置文件，简单直观，易于维护',
    },
    {
        icon: '🐳',
        title: '容器化',
        description: '支持 Docker 部署，开箱即用',
    },
    {
        icon: '🔒',
        title: '可靠性',
        description: '完善的错误处理和日志系统',
    },
]
