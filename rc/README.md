## 概述

rc 目的是开发一个远程测试一些中间件连接功能的工具。

特性:

- 用户友好
- 轻量级
- 支持设置超时
- 支持benchmark模式(类似ab)

## 架构

对接分为多个阶段

- dns解析
- ping
- 连接组件
- 读写数据

每个阶段会有独立的超时设置。


## 预计组件

Protocols:
- PureTCP
- HTTP
- SSH
- FTP
- SFTP

Databases:
- PostgreSQL
- MySQL
- Redis
- MongoDB
- ES
- Doris
- InfluxDB
- VictoriaMetrics
- ClickHouse

Message Queues:
- RabbitMQ
- Kafka
