# VictoriaMetrics 详细入门指南

## 概述

VictoriaMetrics 是一个快速、经济且可扩展的时序数据库和监控解决方案，兼容 Prometheus 生态系统。它提供了卓越的性能、压缩率和易用性，适用于各种规模的监控场景。

资料链接:
- [VM 中文手册集群规划](https://www.victoriametrics.com.cn/docs/ops/cluster/)
- [VM 性能参考](https://valyala.medium.com/insert-benchmarks-with-inch-influxdb-vs-victoriametrics-e31a41ae2893)
- [VM 性能参考](https://valyala.medium.com/when-size-matters-benchmarking-victoriametrics-vs-timescale-and-influxdb-6035811952d4)
- [VM 性能参考](https://valyala.medium.com/high-cardinality-tsdb-benchmarks-victoriametrics-vs-timescaledb-vs-influxdb-13e6ee64dd6b)
- [VM 性能](https://valyala.medium.com/billy-how-victoriametrics-deals-with-more-than-500-billion-rows-e82ff8f725da)
- [VM 性能](https://blog.csdn.net/alex_yangchuansheng/article/details/108271368)

## 部署模式选择

### 单机版
- **适用场景**: 写入速率低于每秒 100 万数据点
- **特点**: 
  - 部署简单，运维成本低
  - 单一二进制文件，易于管理
  - 适合中小型环境或测试场景

### 集群版
- **适用场景**: 大规模监控需求，需要水平扩容
- **核心组件**:
  - **vmstorage**: 存储组件，负责数据持久化
  - **vminsert**: 写入组件，负责接收和路由数据
  - **vmselect**: 查询组件，负责处理查询请求

## 产品特性

### 核心特性
1. **单值模型**: 仅支持 64 位浮点数值数据，设计简洁高效
2. **高度压缩**: 每个数据点（时间戳+数值）仅占用 0.4-0.8 字节
3. **优秀 I/O 性能**: 
   - 采用 LSM 树结构
   - 数据写入流程：内存 → 小文件 → 异步合并为大文件
4. **高吞吐率支持**: 可处理大量并发写入请求
5. **多租户支持**: 支持数据隔离和资源配额
6. **协议兼容性**: 支持多种 TSDB 写入协议（Prometheus、InfluxDB、OpenTSDB 等）
7. **查询语言**: 支持 PromQL 和增强版 MetricsQL

## 数据压缩算法

VictoriaMetrics 采用了参考 Facebook Gorilla 的高效压缩算法：

1. **浮点数转换**: 将浮点值乘以 10^x 转换为整型
2. **差值编码**: 将 Counter 类型使用差值编码转换为 Gauge 类型
3. **最终压缩**: 对编码后的数据使用 zstd 算法进行压缩

这种多层压缩策略显著减少了存储空间需求。

## 集群架构特点

## 集群架构特点

### 架构图

![VictoriaMetrics 集群架构](https://docs.victoriametrics.com/victoriametrics/Cluster-VictoriaMetrics_cluster-scheme.webp)

### 核心算法：一致性哈希

#### 数据插入流程
- vminsert 对每条时间序列（time series）的标签集合（labels）计算 hash
- 根据 hash 值定位到对应的 vmstorage 节点
- 确保相同的时间序列总是路由到同一个 vmstorage，避免重复存储

#### 数据查询流程
- vmselect 广播查询请求到所有 vmstorage 节点
- 每个 vmstorage 节点：
  - 在本地存储中查找匹配的时间序列
  - 执行部分计算（如原始样本读取、简单聚合）
  - 返回中间结果给 vmselect
- vmselect 聚合所有中间结果并返回最终结果

#### vmstorage 节点扩展机制
- 集群不依赖 ZooKeeper/etcd，节点发现通过静态配置（-storageNode 列表）
- 增减 vmstorage 节点时：
  - 已有数据不会自动迁移
  - 新数据会按新哈希环分布
  - 旧数据仍可查询，但可能造成负载不均
  - 官方建议通过运维工具手动进行 rebalance

### 可扩展性
- **组件资源扩缩**: 支持在 Kubernetes 环境中动态调整资源
- **水平扩容**: 可根据负载需求增加组件实例

### 组件特性
- **vminsert/vmselect**: 
  - 无状态服务
  - 扩容后立即可用
  - 支持负载均衡
- **vmstorage**: 
  - 有状态服务
  - 扩容后需要滚动重启 vminsert/vmselect 组件
  - 同一组 vmstorage 仅支持相同的数据过期时间

### 架构限制
- **副本成本**: 副本（replication）功能使用成本较高
  - **存储成本**: 设置副本因子为N，存储空间需求增加N倍（例如：3副本需要3倍存储空间）
  - **网络带宽**: 每份数据需要在多个节点间同步，网络流量成倍增加
  - **写入延迟**: 需要等待多个副本节点确认写入成功，增加写入延迟
  - **运维复杂度**: 需要管理更多的存储节点，监控和故障处理更复杂
  - **与其他TSDB对比**:
    - **InfluxDB**: 内置集群版本支持更高效的副本机制，写入性能损耗较小
    - **TimescaleDB**: 基于PostgreSQL的流复制，成熟稳定，运维工具丰富
    - **Cassandra**: 天然分布式架构，副本配置灵活，一致性级别可调
    - **ClickHouse**: 支持异步复制，对写入性能影响较小
- **Share-nothing**: 各组件完全独立，无共享状态

## 数据备份策略

### 备份工具
- **社区版**: 提供 vmbackup/vmrestore 工具
- **企业版**: 提供 vmbackupmanager 高级管理工具

### 存储支持
- AWS S3
- 腾讯云 COS
- 其他兼容 S3 协议的对象存储

### 备份特点
- 每个 vmstorage 节点需要单独备份
- 支持全量备份和增量备份
- 恢复时只能按 vmstorage 节点进行全量恢复
  - **含义**: 不能选择性恢复某个时间序列或部分数据，必须恢复整个节点的所有数据
  - **限制**: 无法像关系型数据库那样恢复单个表或特定时间范围的数据
  - **影响**: 如果只需要恢复少量数据，也必须恢复该节点的全部数据，可能覆盖正常数据

## 高可用架构

### 负载均衡要求
- HTTP 负载均衡器需要自动剔除不可用的 vminsert/vmselect 节点
- 确保服务连续性的最低要求：
  - 至少一个 vminsert 节点存活，能够承载写入流量
  - 至少一个 vmselect 节点存活，能够承载查询请求
  - 至少一个 vmstorage 节点存活，能够承载读写操作

### 故障处理机制

#### vmstorage 故障处理
- **写入处理**: vminsert 会自动将新数据路由到其他健康的 vmstorage 节点
- **查询处理**: vmselect 会正常响应查询请求，但在响应中标记 `"isPartial": true`，表示数据可能不完整

### 监控建议
为确保集群健康运行，建议监控以下指标：
- 各组件的健康状态
- 数据写入和查询延迟
- 存储空间使用率
- 网络连接状态

## 资源配置评估与参数优化

### CPU、内存、磁盘评估

#### vmstorage - 资源消耗主力

**CPU 和内存需求主要由以下因素决定：**
- 活跃时间序列数量（active time series）
- 每秒写入样本数（samples/sec）
- 查询负载（尤其是高基数或长范围查询）

**磁盘要求：**
- 使用本地 SSD 或高性能云盘(官方推荐解决数据高可用的最具性价比的方案是使用云存储)
- 吞吐比容量更重要：VictoriaMetrics 是 I/O 密集型，尤其在 compaction 和查询时
- 存储空间估算：
  - 平均每个样本 ≈ 0.4-0.8 字节
  - 示例：100K samples/s × 86400 s/day × 0.7 bytes ≈ 6 GB/天
  - 再乘以 `-retentionPeriod`（如 30 天 → ~180 GB）

### 预留资源

建议按照以下的原则为组件预留资源：

- 所有实例类型预留 50% 的可用内存，避免压力临时激增时出现 OOM（内存不足）崩溃和性能下降。
- 所有实例类型预留 50% 的空闲 CPU，避免压力临时激增时出现 OOM（内存不足）崩溃和性能下降。
- vmstorage -storageDataPath参数指向的目录中至少预留20%的空闲空间。超过阈值后，vmstorage 会进入只读模式，防止数据写入失败。

### 组件扩展策略

| 组件        | 扩展方式                         | 资源关注点                    |
|-----------|------------------------------|--------------------------|
| vmstorage | 增加副本数（通常 3）/ 水平扩展            | CPU、RAM、磁盘 I/O & 容量      |
| vminsert  | 水平扩展（Deployment 增加 replicas） | 网络带宽、CPU（用于分片路由）         |
| vmselect  | 水平扩展（Deployment 增加 replicas） | CPU（PromQL 计算）、RAM（结果缓存） |

### 核心参数配置

#### 通用参数

| 参数                           | 作用      | 资源影响              |
|------------------------------|---------|-------------------|
| `-retentionPeriod`           | 数据保留时间  | 决定磁盘用量            |
| `-search.maxQueryDuration`   | 限制查询时长  | 防止长查询占用过多资源       |
| `-dedup.minScrapeInterval`   | 启用去重    | 增加 CPU 开销，但减少存储   |

#### vmselect 配置参数

| 参数                                    | 推荐值         | 说明                  |
|---------------------------------------|-------------|---------------------|
| `cacheExpireDuration`                 | `5m`        | 缓存过期时间              |
| `search.maxUniqueTimeseries`          | `500000`    | 查询最大唯一时间序列数         |
| `search.maxSamplesPerQuery`           | `1000000000`| 单次查询最大样本数           |
| `search.maxPointsPerTimeseries`       | `500000`    | 每个时间序列最大点数          |
| `search.maxSeries`                    | `200000`    | 查询最大序列数             |
| `memory.allowedPercent`               | `20`        | 允许使用的内存百分比          |
| `search.maxMemoryPerQuery`            | `3GB`       | 单次查询最大内存使用量         |
| `search.logQueryMemoryUsage`          | `1GB`       | 记录内存使用超过此值的查询       |
| `search.logSlowQueryDuration`         | `10s`       | 记录执行时间超过此值的慢查询      |
| `search.queryStats.lastQueriesCount` | `10000`     | 保留最近查询统计数量          |
| `search.queryStats.minQueryDuration` | `3s`        | 统计查询的最小执行时间阈值       |
| `search.maxQueryLen`                  | `4MB`       | 查询请求的最大长度           |
| `dedup.minScrapeInterval`             | `1ms`       | 去重的最小间隔时间           |
| `search.maxConcurrentRequests`        | `16`        | 最大并发请求数             |

#### vminsert 配置参数

| 参数                        | 推荐值        | 说明              |
|---------------------------|------------|-----------------|
| `influxDBLabel`           | `__bk_db__`| InfluxDB 标签标识   |
| `maxLabelsPerTimeseries`  | `100`      | 每个时间序列最大标签数量    |

#### vmstorage 配置参数

| 参数                             | 推荐值  | 说明               |
|--------------------------------|------|------------------|
| `cacheExpireDuration`          | `15m`| 缓存过期时间           |
| `dedup.minScrapeInterval`      | `1ms`| 去重的最小间隔时间        |
| `internStringMaxLen`           | `128`| 内部字符串最大长度        |
| `memory.allowedPercent`        | `50` | 允许使用的内存百分比       |
| `retentionPeriod`              | `6` 或 `180d`  | 数据保留期（月）         |
| `search.maxConcurrentRequests` | `16` | 最大并发请求数          |

### 性能调优建议

1. **内存分配**：vmstorage 建议分配 50% 系统内存，vmselect 分配 20%
2. **并发控制**：根据 CPU 核数调整 `maxConcurrentRequests`
3. **查询限制**：合理设置查询时长和内存限制，防止单个查询影响整体性能
4. **缓存策略**：适当调整缓存过期时间，平衡内存使用和查询性能
5. **去重配置**：根据数据采集频率合理设置去重间隔

## 最佳实践

1. **容量规划**: 根据数据写入速率选择合适的部署模式
2. **监控设置**: 建立完善的监控体系，及时发现问题
3. **备份策略**: 制定定期备份计划，确保数据安全
4. **性能优化**: 根据实际负载调整组件配置和资源分配
5. **升级策略**: 制定安全的集群升级方案

## Q&A 常见问题

### Q: 为什么推荐 vmstorage 实例个数多而轻量?

进行维护操作（如升级、配置更改或迁移）时，如果部分 vmstorage 实例临时不可用，集群更有可能保持高可用性和稳定性。更多的轻量级实例可以分散风险，减少单点故障对整体系统的影响。举例来说，若一个集群拥有10个vmstorage实例，其中一个实例不可用时，其余9个实例的负载将增加约11%（即1/9）。而如果集群仅由3个vmstorage实例构成，其中一个实例不可用时，其余两个实例的负载将激增50%（即1/2）。 

### Q: VictoriaMetrics 如何保证数据的可用性？

**A: VictoriaMetrics 通过多种机制来保证数据可用性：**

#### 1. 数据持久化机制
- **WAL (Write-Ahead Log)**: 数据首先写入预写日志，确保即使发生故障也不会丢失
- **LSM 树存储**: 采用分层存储结构，数据从内存逐步合并到磁盘，提高写入性能和数据安全性
- **定期同步**: 内存中的数据会定期刷写到磁盘，防止系统崩溃导致的数据丢失

#### 2. 集群模式的可用性保障
- **数据分片**: 数据在多个 vmstorage 节点间分布，单节点故障不会导致整个系统不可用，但**该节点上的数据会丢失**（除非配置了副本）
- **自动故障转移**: 
  - vminsert 自动检测 vmstorage 节点状态，将新数据路由到健康节点
  - vmselect 在部分节点故障时仍可提供查询服务，返回可用数据并标记 `isPartial: true`
- **无单点故障**: vminsert 和 vmselect 为无状态服务，可部署多个实例

#### 3. 数据副本策略（企业版）
- **replicationFactor**: 可配置数据副本数量，提高数据冗余度
- **跨节点副本**: 数据副本分布在不同的 vmstorage 节点上
- **注意**: 副本功能会增加存储成本和网络开销

#### 4. 备份与恢复
- **增量备份**: 支持增量备份，减少备份时间和存储空间
- **远程存储**: 备份数据存储在云对象存储中，提供额外的数据保护层
- **快速恢复**: 使用 vmrestore 工具可快速恢复特定时间点的数据

#### 5. 监控与告警
- **健康检查**: 各组件提供健康检查接口，便于监控系统及时发现问题
- **指标暴露**: 详细的内部指标帮助运维团队了解系统状态
- **预警机制**: 建议设置磁盘空间、内存使用率等关键指标的告警

#### 6. 运维最佳实践
- **定期备份**: 建立自动化备份策略，确保数据可恢复
- **容量规划**: 合理规划存储容量，避免磁盘空间不足
- **版本管理**: 保持组件版本一致，避免兼容性问题
- **灾难恢复**: 制定完整的灾难恢复计划和演练方案

通过以上多层次的保护机制，VictoriaMetrics 能够在各种故障场景下最大程度地保证数据的可用性和完整性。

---

## 总结

VictoriaMetrics 提供了一个高性能、高压缩率的时序数据库解决方案。通过合理的架构设计和运维策略，可以构建稳定可靠的监控基础设施，满足 SRE 团队的各种需求。

---
*文档版本: v1.2*  
*更新日期: 2025年11月17日*  
*新增内容: 集群架构详解、资源配置评估与参数优化*
