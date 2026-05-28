# rsagent 详细设计

## 文档目标

这篇文档用于在 `doc/rsagent概述.md` 的基础上，把 `rsagent` 从“组件概述”细化为“可执行的详细设计草案”。

本文主要回答以下问题：

- `rsagent` 第一阶段到底负责什么；
- `rsagent` 与 `nodemanage`、`datalink-engine`、`query-engine`、`job-manage` 的边界如何稳定；
- `rsagent` 的主协议应该怎样组织；
- `rsagent` 内部运行时模块如何拆分；
- `rsagent` 安装包如何构建、命名、打包和发布；
- 安装包里必须包含哪些自述信息和默认生命周期脚本。

本文是第一阶段详细设计文档，不等同于最终 API 契约、代码实现或发布流水线配置的冻结版本。凡是现有仓库文档尚未证明的内容，本文会明确标注为 **Assumption** 或 **TBD**。

## 组件定位

`rsagent` 的定位保持与概述文档一致：

> `rsagent` 是 NodeManage 的节点侧执行与通信代理，是节点被平台稳定纳管的基础设施。

展开后，`rsagent` 的角色可以收敛为：

- 被安装到目标机器上的常驻代理；
- 节点侧注册与握手主体；
- heartbeat 上报主体；
- 配置同步消费者；
- 后续节点侧执行能力承接层。

它不是：

- 控制面；
- 链路元数据管理服务；
- 状态解释服务；
- 通用任务编排中心。

## 设计目标

`rsagent` 第一阶段不追求一开始就做成重型远程控制平台，而是优先打通“节点稳定接入并被观测”的主闭环。

因此本文的设计目标是：

1. 让 `rsagent` 能够被标准化安装到目标节点；
2. 让 `rsagent` 启动后能够向 `nodemanage` 稳定注册；
3. 让 `rsagent` 能够基于 `data_link_id` 上报 heartbeat；
4. 让 `rsagent` 能够周期性同步配置或链路信息；
5. 在第一阶段明确 `job-manage`（`jm`）与 `rsagent` 的通信方式；
6. 在第一阶段支持由 `jm` 向节点下发脚本并执行、回传结果；
7. 不在第一阶段写死重型执行框架或复杂任务编排系统；
8. 定义一套可重复构建、可分发、可审计的安装包规范。

## 与其他组件的边界

### 与 nodemanage 的边界

`nodemanage` 负责：

- 节点主体管理；
- 安装流程编排；
- 安装参数与链路引用下发；
- 注册结果接收；
- 节点管理态聚合。

`rsagent` 负责：

- 启动自检；
- 向 `nodemanage` 注册；
- 消费安装时下发的配置；
- 维持节点侧常驻通信与 heartbeat 上报。

### 与 datalink-engine 的边界

`datalink-engine` 拥有链路定义真相，负责：

- `data_link_id`
- `datasource`
- `etl_pipeline`
- `result_table`

`rsagent` 不定义 heartbeat 链路，只消费链路信息。

第一阶段默认约束：

- `nodemanage` 安装 `rsagent` 时下发 heartbeat 的 `data_link_id`；
- `rsagent` 后续按约定周期同步链路信息或配置；
- `rsagent` 只负责按链路要求上报 heartbeat 事实，不解释状态语义。

### 与 query-engine 的边界

`query-engine` 负责：

- 查询底层存储；
- 返回 heartbeat 等指标的统一查询结果。

`rsagent` 不直接判断“节点是否在线”，它只负责上报节点侧事实数据。

### 与 job-manage 的边界

`job-manage` 是远程任务编排与执行管理服务。

第一阶段里，`job-manage` 与 `rsagent` 的边界需要明确到可落地程度：

- `job-manage` 负责创建作业、选择节点、生成脚本执行任务；
- `rsagent` 负责通过约定通信通道接收任务，在节点侧执行脚本，并回传结果；
- `job-manage` 仍然拥有任务编排、任务级状态、结果聚合和审计记录；
- `rsagent` 不应被扩成完整 job runtime，只实现第一阶段所需的最小脚本执行承接能力。

## 第一阶段职责

### 1. 启动与本地自检

`rsagent` 启动后应完成：

- 读取本地配置；
- 校验配置完整性；
- 校验运行环境是否满足要求；
- 识别本机基础平台信息，例如 OS / 发行版 / 架构；
- 准备注册所需身份信息。

### 2. 注册与重注册

`rsagent` 应在首次启动和后续重启时，向 `nodemanage` 发起注册或握手。

目标是让平台知道：

- 这个 agent 是谁；
- 它当前运行在哪台节点上；
- 它当前版本是什么；
- 它当前具备哪些基础能力。

### 3. heartbeat 上报

`rsagent` 第一阶段最重要的持续职责，是按 heartbeat 链路定义上报节点心跳事实。

这里强调：

- `rsagent` 负责上报事实；
- `query-engine` 负责查询 heartbeat 数据；
- `nodemanage` 负责计算和消费节点状态。

### 4. 配置同步

`rsagent` 应支持周期性拉取配置或链路信息，以感知：

- heartbeat 链路变化；
- 配置版本变化；
- 后续能力配置变化。

### 5. 脚本执行承接层

第一阶段里，`rsagent` 不再只是“预留执行承接模块”，而是要至少支持一条最小任务执行主线：

- 接收 `job-manage` 下发的脚本执行任务或命令执行任务；
- 在节点侧执行脚本或命令；
- 回传执行状态与完整结果。

这个约束的意义是：

- 第一阶段就把 `jm ↔ rsagent` 主链路打通；
- 但不把 `rsagent` 扩展成完整通用任务平台。

## 通信模式建议

当前建议采用“**agent 主动注册 + 主动 heartbeat + 主动拉配置 + 主动拉任务**”作为第一阶段默认模式。

原因：

- 更符合当前 `rsagent概述.md`；
- 更适合先打通稳定纳管闭环；
- 也更适合先打通 `jm ↔ rsagent` 的任务分发主线；
- 对 NAT、复杂网络和节点侧部署环境更友好；
- 避免第一阶段就引入长期平台到 agent 的反向连接要求。

这意味着第一阶段默认通信方式建议是：

- `rsagent` 主动向 `nodemanage` 注册；
- `rsagent` 主动上报 heartbeat；
- `rsagent` 主动拉取配置；
- `rsagent` 主动向 `job-manage` 拉取待执行任务；
- 任务执行完成后，`rsagent` 主动回传执行结果。

后续如需要引入推送控制信令、长连接或流式任务通道，统一视为后续增强项。

## 协议设计

### 1. 注册协议

### 目标

注册协议用于建立以下关系：

- agent 身份；
- 节点身份；
- 当前运行版本；
- 当前基础能力；
- 平台返回的配置上下文。

### 请求建议字段

第一阶段建议至少包含：

- `agent_id`
- `node_id` 或节点身份材料
- `agent_version`
- `hostname`
- `os_family`
- `os_distribution`
- `arch`
- `capabilities`
- `started_at`

### 响应建议字段

建议至少包含：

- 注册是否成功；
- 当前绑定的 `node_id`；
- 当前配置版本；
- heartbeat `data_link_id`；
- heartbeat 上报间隔；
- 配置同步间隔。

### 设计约束

- 注册成功不等于节点状态一定在线；
- 注册只是“节点侧代理已被平台识别”；
- 节点是否在线，仍然要通过 heartbeat 查询结果由 `nodemanage` 计算判断。

### 2. heartbeat 协议

### 目标

heartbeat 协议用于持续上报节点存活事实。

### 建议上报内容

建议至少包含：

- `node_id`
- `agent_id`
- `agent_version`
- `timestamp`
- `status = alive`
- 可选基础标签，如 `environment`、`cluster`

### 设计约束

- `rsagent` 不上报“我是在线/离线”这种解释性结论；
- 它只上报 heartbeat 事实和必要维度；
- 状态判断逻辑留在 `nodemanage`。

### 3. 配置同步协议

### 目标

配置同步协议用于让 `rsagent` 周期性拉取最新配置和链路信息。

### 建议能力

- 按版本号拉取配置；
- 无变更时返回 not modified 语义；
- 支持同步 heartbeat 相关链路配置；
- 后续可扩展能力配置同步。

### 设计约束

- 第一阶段优先支持“定期拉取”；
- 不要求一开始就支持复杂配置推送或双向流。

### 4. job-manage 与 rsagent 的任务执行协议

第一阶段虽然不冻结完整任务系统，但必须冻结一条最小任务执行协议主线，用于让 `job-manage` 真正把脚本或命令下发到节点执行。

具体接口与状态约束，见 `doc/job-manage 与 rsagent 通信协议.md`。

#### 目标

- 让 `job-manage` 能向 `rsagent` 下发脚本执行任务或命令执行任务；
- 让 `rsagent` 能在节点侧执行脚本或命令；
- 让执行结果可被稳定回传给 `job-manage`。

#### 通信主线建议

当前建议第一阶段采用：

- `rsagent` 主动拉取任务；
- `job-manage` 返回当前待执行任务；
- `rsagent` 显式确认接单；
- `rsagent` 执行过程中可选上报 `running`；
- `rsagent` 执行后主动回传最终结果。

这种方式与注册、heartbeat、配置同步的主动拉模型保持一致，也能把“任务已下发”和“agent 已接单”区分开。

#### 任务下发建议字段

建议最小任务模型至少包含：

- `task_id`
- `task_type`
- `script_content`
- `args`
- `env`
- `working_dir`
- `timeout`
- `issued_at`

其中：

- `task_type` 第一阶段建议至少支持 `script` 和 `command`；
- 当 `task_type = script` 时，`script_content` 表示脚本文本；
- 当 `task_type = command` 时，`script_content` 表示命令文本或统一可执行文本内容；
- `script_ref` 不作为第一阶段默认能力；
- 更复杂的文件分发、二进制分发和高级命令编排，统一留待后续扩展。

#### 结果回传建议字段

建议至少包含：

- `task_id`
- `execution_state`
- `stdout`
- `stderr`
- `exit_code`
- `started_at`
- `finished_at`
- `error_message`

必要时也可以补充：

- `duration_ms`

这里要明确：

- 第一阶段必须存在任务协议；
- 但本文不把 `rsagent` 第一阶段定义成完整任务执行引擎，只冻结“脚本/命令下发执行”这条最小主线。

## 内部运行时结构建议

为了让后续实现保持清晰，建议 `rsagent` 运行时至少拆成以下模块。

### 1. bootstrap

负责：

- 配置加载；
- 环境检查；
- 平台信息识别；
- 启动阶段依赖准备。

### 2. registration

负责：

- 注册；
- 重注册；
- 握手状态维护；
- 节点身份与 agent 身份绑定。

### 3. heartbeat

负责：

- heartbeat 定时调度；
- heartbeat 上报；
- 上报重试与失败状态记录。

### 4. config_sync

负责：

- 配置拉取；
- 版本比较；
- 配置变更感知；
- 后续配置热更新承接。

### 5. executor

负责：

- 接收 `job-manage` 下发的脚本执行任务或命令执行任务；
- 调用本地脚本执行器；
- 采集执行状态与结果；
- 回传结果给 `job-manage`。

这里建议第一阶段就实现最小可用版本，但不扩展到复杂任务编排能力。

## 状态机建议

### 1. agent 生命周期状态

- `starting`
- `registering`
- `registered`
- `degraded`
- `stopped`

### 2. 配置状态

- `config_unknown`
- `config_synced`
- `config_stale`
- `config_error`

### 3. heartbeat 状态

- `idle`
- `sending`
- `healthy`
- `retrying`
- `failed`

这些状态名是当前详细设计里的建议集合；是否完全采用这些名称，可在后续协议文档或实现设计里继续收敛。

## rsagent 安装包设计

这是本文新增的重点部分。`rsagent` 不是只需要“一个二进制文件”，而是需要一套可安装、可重启、可卸载、可辨识版本和平台信息的标准化交付包。

### 安装包目标

`rsagent` 安装包应满足：

1. 可被安装器或安装脚本直接下载；
2. 可在目标节点直接解压和执行；
3. 包内自带基础生命周期脚本；
4. 包内包含版本自述文件，明确描述版本和平台信息；
5. 让安装过程不依赖外部“猜测包内容”。

### 包格式建议

第一阶段建议统一使用：

- `*.tar.gz`

原因：

- 与 Linux/Unix 环境兼容性最好；
- 便于通过 `wget` + `tar` 完成最小安装流程；
- 与当前 `nodemanage` 安装制品设计文档保持一致。

后续如支持 Windows，可新增等价分发形式，但不改变“包必须自描述且具备默认生命周期脚本”这一原则。

### 包命名建议

建议包名至少包含：

- 组件名
- 版本
- OS
- 架构

例如：

- `rsagent-v1.2.3-linux-amd64.tar.gz`
- `rsagent-v1.2.3-linux-arm64.tar.gz`

如果后续需要引入发行版粒度，也可以在文件名里增加 `os_distribution`。

### 包内目录结构建议

建议最小目录结构如下：

```text
rsagent/
├── bin/
│   └── rsagent
├── scripts/
│   ├── install.sh
│   ├── restart.sh
│   └── uninstall.sh
├── conf/
│   └── rsagent.example.toml
└── manifest/
    └── version.json
```

这里的设计意图是：

- `bin/` 放 agent 主程序；
- `scripts/` 放默认生命周期脚本；
- `conf/` 放默认配置示例；
- `manifest/` 放版本自述文件。

## 版本自述文件设计

### 目标

版本自述文件用于让安装器、平台和运维人员能够明确知道：

- 这个包是什么；
- 版本是多少；
- 适用于什么平台；
- 包内带了哪些脚本；
- 构建时间和构建来源是什么。

### 文件位置建议

- `manifest/version.json`

### 建议字段

建议至少包含：

- `name`
- `version`
- `os_family`
- `os_distribution`（可选）
- `arch`
- `build_time`
- `git_commit`
- `included_scripts`

### 示例

```json
{
  "name": "rsagent",
  "version": "1.2.3",
  "os_family": "linux",
  "os_distribution": "ubuntu",
  "arch": "amd64",
  "build_time": "2026-05-28T12:00:00Z",
  "git_commit": "abc123def456",
  "included_scripts": [
    "install.sh",
    "restart.sh",
    "uninstall.sh"
  ]
}
```

### 设计约束

- `version.json` 不是给人类写长文档的 README，而是给安装器和平台读取的结构化自述文件；
- `os_family` / `arch` 是必填信息；
- `included_scripts` 用于明确包内默认可用脚本集合。

## 默认生命周期脚本设计

第一阶段建议在包内固定提供以下三个默认脚本：

- `install.sh`
- `restart.sh`
- `uninstall.sh`

### 1. install.sh

职责建议：

- 解压或整理安装目录；
- 安放可执行文件；
- 渲染或落盘配置文件；
- 创建 systemd 或等价启动单元（如适用）；
- 启动 `rsagent`。

### 2. restart.sh

职责建议：

- 重启 `rsagent` 服务；
- 保留现有配置；
- 不负责全量重装。

### 3. uninstall.sh

职责建议：

- 停止 `rsagent`；
- 删除安装文件；
- 清理启动单元；
- 是否删除配置和日志，建议通过参数控制。

### 设计原则

- 这三个脚本属于包内默认生命周期脚本；
- 它们服务于安装与基础运维闭环；
- 不应被理解为通用任务脚本；
- 后续如平台需要显式脚本覆盖，应由 `nodemanage` 的安装制品模型中的外部脚本对象来表达。

## rsagent 安装包构建过程

这是本文必须明确的一部分：不仅要定义“包长什么样”，还要定义“包怎么被构建出来”。

### 构建输入

建议构建输入至少包括：

- `rsagent` 源码；
- 目标 `os_family`；
- 目标 `arch`；
- 版本号；
- 生命周期脚本模板或脚本文件；
- 默认配置模板。

### 构建步骤建议

1. 编译 `rsagent` 可执行文件；
2. 生成目标平台对应的目录结构；
3. 拷贝 `bin/rsagent`；
4. 写入 `scripts/install.sh`、`scripts/restart.sh`、`scripts/uninstall.sh`；
5. 写入 `conf/rsagent.example.toml`；
6. 生成 `manifest/version.json`；
7. 对目录进行归档，生成 `*.tar.gz`；
8. 计算归档包 checksum；
9. 发布到可被安装器直接下载的位置。

### 构建产物要求

构建完成后，产物至少应满足：

- 文件名包含版本、OS、架构；
- 包内存在 `version.json`；
- 包内存在 3 个默认生命周期脚本；
- 包可以被 `wget` 直接下载；
- 下载后可直接用标准归档工具解压。

### 发布要求（Assumption）

当前默认建议：

- 构建后的包发布到一个可直接下载的 URL；
- `nodemanage` 通过 `rsagent_package_url` 或后续制品模型引用该 URL；
- 安装器不依赖手工网页跳转或登录后点击下载。

完整发布流水线、对象存储、制品仓库、签名验证流程，当前统一视为 **TBD**。

## 与 nodemanage 安装制品模型的对齐

`rsagent` 安装包设计必须和 `doc/nodemanage安装制品模型设计.md` 保持一致。

对齐点包括：

1. agent 包默认是 `*.tar.gz`；
2. 包地址应可被 `wget` 直接下载；
3. 包内默认包含 `install.sh`、`restart.sh`、`uninstall.sh`；
4. `nodemanage` 可以把包内默认脚本视为“制品自带默认行为”；
5. 如需显式覆盖脚本，应通过 `InstallScript` 一类外部脚本对象实现，而不是修改 `rsagent` 包语义。

## 第一阶段建议冻结的内容

第一阶段建议先冻结以下内容：

1. `rsagent` 的控制面边界；
2. 注册协议主线；
3. heartbeat 上报主线；
4. 配置同步主线；
5. `jm ↔ rsagent` 通信主线；
6. 第一阶段脚本下发执行主线；
7. 基础运行时模块拆分；
8. `*.tar.gz` 安装包格式；
9. `manifest/version.json` 的存在和核心字段；
10. 包内默认生命周期脚本集合；
11. 包文件名包含版本、OS、架构。

## 第一阶段不冻结的内容

以下内容统一留作后续增强项或 TBD：

1. 长连接推送控制信令；
2. 完整任务执行协议的扩展模型（除脚本执行主线外）；
3. 插件系统的正式生命周期；
4. 复杂配置热更新模型；
5. Windows 平台分发细节；
6. 安装包签名、验签、供应链安全机制；
7. 完整 CI/CD 打包流水线配置。

## Assumptions / TBD 汇总

以下内容在当前仓库文档中尚未被正式定义，因此在本文中仍视为默认设计方向：

1. 注册协议的最终字段集合；
2. 配置同步协议的最终路径和返回格式；
3. heartbeat 上报协议与底层写入方式的具体实现；
4. `jm ↔ rsagent` 任务拉取接口的最终路径和返回格式；
5. `version.json` 的最终字段全集；
6. 生命周期脚本是否允许平台侧模板化生成；
7. 发布包 URL 的存储位置和发布流程；
8. 是否对不同 `os_distribution` 生成独立包；
9. 脚本任务里 `script_content` 与脚本引用两种形式的最终取舍。

## 后续拆分建议

如果继续细化 `rsagent`，建议后续拆出以下文档：

1. `rsagent注册与配置协议.md`
2. `rsagent heartbeat与状态上报设计.md`
3. `rsagent安装包与发布规范.md`
4. `rsagent运行时架构设计.md`

## 对齐参考文档

本文在以下文档约束下编写：

- `doc/rsagent概述.md`
- `doc/nodemanage详细设计.md`
- `doc/nodemanage安装制品模型设计.md`
- `doc/query-engine概述.md`

后续如果 `nodemanage` 的安装制品模型继续演进，本文中的安装包规范也应同步调整，避免两个文档对“agent 包是什么”产生分叉。
