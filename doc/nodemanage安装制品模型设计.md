# nodemanage 安装制品模型设计

## 文档目标

这篇文档聚焦 `nodemanage` 中“安装制品域”的详细设计。

它回答的问题不是“节点如何被纳管”这一整条主链路，而是更具体的几件事：

- `nodemanage` 需要管理哪些安装制品对象；
- 安装包、安装脚本、安装制品选择规则、安装解析结果之间是什么关系；
- 版本、OS、发行版、架构兼容性应如何落到模型中；
- 哪些内容已经在仓库里有配置面，哪些还只是详细设计方向；
- 安装任务应如何引用这些对象，保证可审计、可回放、可排障。

本文是 `doc/nodemanage详细设计.md` 中“安装制品域设计”章节的拆分细化文档。

## 适用范围

本文只讨论 `nodemanage` 为首次纳管和后续安装类流程所需要的“制品元数据模型”。

它不展开：

- 通用远程执行平台能力；
- `job-manage` 的脚本分发和任务执行模型；
- `rsagent` 内部执行协议；
- 制品仓库实现细节；
- 完整供应链签名与验签系统。

也就是说，本文中的“脚本”和“安装包”都属于 **安装控制面元数据**，不是“平台通用执行任务”。

## 与现有仓库能力的关系

在进入模型前，先明确当前仓库里已经存在什么、还不存在什么。

### 当前已经明确存在的配置面

从 `common/config/src/nodemanage.rs` 可以确认，当前代码层已存在：

- `table_prefix`
- `mysql`
- `rsagent_package_url`
- `ssh_connect_timeout_secs`

其中：

- `rsagent_package_url` 说明仓库已经承认“安装 rsagent 时需要一个包地址”；
- 这个“包地址”在详细设计里不应只是泛化 URL，而应代表一个可被安装器直接拉取的制品入口；
- 当前默认约束建议是：该地址应能被 `wget` 或等价下载工具直接下载，下载结果是一个 `*.tar.gz` 形式的归档包；
- 该归档包内默认应包含生命周期脚本，例如 `install.sh`、`restart.sh`、`uninstall.sh`；
- 也就是说，仓库已经在配置面上承认“安装 rsagent 时需要一个包地址”，但还没有把“包协议、归档格式、包内默认脚本布局”正式建模为领域对象；
- 但当前还没有独立的 `InstallPackage` 领域对象；
- 也没有版本矩阵、OS 兼容矩阵或安装脚本对象。

### README 与代码之间的现状差异

`common/config/README.md` 里还描述了以下 `NodeManage` 配置项：

- `install_root`
- `register_callback_url`
- `install_plugins`
- `register_wait_timeout_secs`

但这些字段目前没有全部出现在 `common/config/src/nodemanage.rs` 中。

因此本文统一采用以下原则：

1. **代码里已存在的配置面**，视为已落地事实；
2. **README 已描述但代码未落地的配置面**，视为“已表达意图但尚未完全收敛”；
3. **本文新增的安装制品对象**，统一视为详细设计方向，不伪装成现有实现。

## 设计动机

如果 `nodemanage` 只记录“发起了一次安装”，却无法回答下面这些问题，那么纳管流程就很难具备工程可用性：

- 安装时选的是哪个 rsagent 包版本；
- 这个包适用于什么 OS / 发行版 / 架构；
- 这次安装执行的是哪份脚本；
- 脚本版本和包版本是否兼容；
- 默认插件包有哪些；
- 某次安装失败时，当时到底选择了哪组制品和参数。

所以 `nodemanage` 至少需要把“安装制品选择结果”纳入自己的控制面模型中。

这里的重点是：

- `nodemanage` 不一定要保存所有制品二进制内容；
- 但它必须保存足够的制品元数据和选择结果，才能稳定审计安装行为。

## 术语约定

### 1. 安装包（InstallPackage）

安装包表示一个可被纳管流程分发或下载安装的二进制制品，例如：

- `rsagent` 主包；
- 安装时默认附带的插件包。

在本文中，默认的 agent 安装包不是任意文件，而是一个具备约定结构的安装归档包。

当前建议约定：

- 包地址应支持 `wget <package_url>` 这类直接下载方式；
- 下载结果应为 `*.tar.gz` 归档包；
- 包内默认应包含生命周期脚本，例如 `install.sh`、`restart.sh`、`uninstall.sh`；
- 安装器在没有显式外部脚本覆盖时，应优先使用包内默认脚本。

### 2. 安装脚本（InstallScript）

安装脚本表示一次安装流程中所引用的脚本制品元数据。

这里的脚本只服务于“安装/升级/卸载/注册引导”场景，不延伸为通用任务脚本。

需要额外说明的是：

- agent 主包本身可以已经携带默认生命周期脚本；
- `InstallScript` 不一定总是必填的外部对象；
- 当包内默认脚本已经满足需要时，安装流程可以直接引用包内脚本；
- `InstallScript` 更适合表达“外部覆盖脚本”“平台显式指定脚本版本”或“特殊 OS 的替代脚本”。

### 3. 安装制品选择规则（InstallArtifactProfile）

为了避免和 `datalink-engine` 文档里的 `profile` 数据类型冲突，本文统一使用：

- `InstallArtifactProfile`
- 或简称 `install_profile`

它表示：

- 某种 OS / 发行版 / 架构默认应该选哪组安装制品。

### 4. 安装解析结果（InstallResolution）

安装解析结果表示：

- 某次安装任务最终选中了哪组 package / script / plugin / profile。

这是“某次安装当时实际使用了什么”的审计快照。

## 模型设计原则

### 1. 制品元数据与执行行为分离

`nodemanage` 管的是：

- 制品是什么；
- 哪些节点应该用什么制品；
- 某次安装最终选中了什么。

它不直接代表：

- 任意脚本执行平台；
- 节点侧任务调度系统。

### 2. 版本与兼容性必须是一等信息

安装包和安装脚本都必须天然携带：

- `version`
- `os_family`
- `os_distribution`
- `arch`

否则安装选择会退化成一堆难以解释的字符串拼接逻辑。

### 3. 解析结果必须可冻结

安装任务执行时，最终选中的制品集合必须被记录为快照。

否则一旦默认版本、profile 或兼容矩阵发生变化，历史安装任务将无法复原。

### 4. 当前能力和未来设计必须明确区分

本文所有超出当前代码配置面的内容，都应被理解为：

- 为后续实现服务的详细设计；
- 不是当前仓库已实现事实。

## 核心对象

### 1. InstallPackage

`InstallPackage` 表示一个可用于安装流程的包元数据对象。

#### 职责

- 描述一个安装包是什么；
- 描述它适用于哪些平台；
- 提供安装时所需的下载与校验信息。

#### 建议字段

- `package_id`
- `name`
- `package_type`：`agent` / `plugin`
- `version`
- `os_family`
- `os_distribution`
- `arch`
- `package_url`
- `checksum`
- `status`
- `created_at`
- `updated_at`

#### 字段说明

- `name`：逻辑名称，例如 `rsagent`、`shell-plugin`；
- `package_type`：用于区分主包和插件包；
- `version`：包版本；
- `os_family`：例如 `linux` / `windows` / `darwin`；
- `os_distribution`：例如 `ubuntu` / `centos` / `rhel`；如果不区分发行版，可为空或使用通配含义；
- `arch`：例如 `amd64` / `arm64`；
- `package_url`：包下载地址。默认约束是可直接下载，且指向一个 `*.tar.gz` 归档包；
- `checksum`：用于完整性校验；
- `status`：用于控制一个包当前是否可被新安装任务选择。

#### 包协议与归档约定

为了让安装链路稳定可执行，建议对 agent 主包补充以下约束：

1. `package_url` 指向的资源应能被标准下载工具直接获取，不要求额外跳转页面交互；
2. 默认下载协议可以是 `http(s)`，但重点不是协议名本身，而是“安装器可直接拉取”；
3. agent 主包建议统一为 `*.tar.gz` 归档形式；
4. 归档包内建议至少包含：
   - agent 主程序或主程序目录；
   - `install.sh`；
   - `restart.sh`；
   - `uninstall.sh`；
5. 后续如需要 Windows 目标，允许引入等价脚本，例如 `install.ps1`，但不改变“包内应自带默认生命周期脚本”这一原则。

这样定义后，`InstallPackage` 就不只是“一个下载地址”，而是“一个具备默认生命周期行为入口的安装制品”。

#### 状态建议

- `draft`
- `active`
- `deprecated`
- `disabled`

含义建议如下：

- `draft`：已登记但未对外生效；
- `active`：可被新安装任务正常选择；
- `deprecated`：仍可被历史任务引用，但不建议新任务使用；
- `disabled`：禁止新任务选择。

### 2. InstallScript

`InstallScript` 表示安装流程中引用的脚本元数据对象。

#### 职责

- 描述安装脚本的版本、适用平台和解释器；
- 提供脚本内容或脚本内容引用；
- 让安装流程具备“脚本版本可追溯”的能力；
- 在包内默认脚本不够用时，提供显式覆盖入口。

#### 建议字段

- `script_id`
- `name`
- `script_kind`
- `version`
- `os_family`
- `os_distribution`
- `arch`
- `interpreter`
- `script_source`
- `content_ref`
- `status`
- `created_at`
- `updated_at`

#### 字段说明

- `script_kind`：建议枚举为 `bootstrap` / `install` / `upgrade` / `uninstall` / `register`；
- `interpreter`：例如 `bash` / `sh` / `powershell`；
- `script_source`：脚本来源方式，例如 inline / object_storage / git；
- `content_ref`：脚本内容本体或外部引用标识；
- `status`：是否允许被新安装任务使用。

#### 关于脚本类型的边界说明

这里的 `InstallScript` 只服务于安装生命周期。

它和包内默认脚本的关系建议定义为：

- **包内脚本优先表示“制品自带默认行为”**；
- **InstallScript 优先表示“平台显式覆盖或补充行为”**。

因此下面这些能力不应被归到这个对象中：

- 任意运维脚本分发；
- 用户临时命令执行；
- 任务编排；
- 节点巡检任务。

这些能力如果出现，仍然更接近 `job-manage` 域。

### 3. InstallArtifactProfile

`InstallArtifactProfile` 表示一组安装制品选择规则。

#### 职责

- 把平台兼容性规则集中到一个对象；
- 回答“面对某类节点，默认应安装什么”；
- 让安装任务不必手写一堆临时判断逻辑。

#### 建议字段

- `install_profile_id`
- `name`
- `target_os_family`
- `target_os_distribution`
- `target_arch`
- `default_agent_package_id`
- `default_install_script_id`
- `default_plugin_package_ids`
- `status`
- `created_at`
- `updated_at`

#### 字段说明

- `target_os_family` / `target_os_distribution` / `target_arch`：用于声明这个 profile 面向哪类节点；
- `default_agent_package_id`：默认 rsagent 主包；
- `default_install_script_id`：默认安装脚本；
- `default_plugin_package_ids`：默认插件包集合；
- `status`：决定这个 profile 是否还能被解析命中。

#### 设计约束

`install_profile` 是“默认选择中心”，但不必是“唯一真相源”。

也就是说：

- 任务级可以允许临时覆盖；
- 但如果没有覆盖，应该优先通过 profile 做解析。

### 4. InstallResolution

`InstallResolution` 表示一次安装任务最终选中的制品快照。

#### 职责

- 把“这次安装最后到底用了什么”冻结下来；
- 为审计、排障、复现和回滚分析提供依据；
- 避免历史任务随着默认 profile 调整而失真。

#### 建议字段

- `resolution_id`
- `install_task_id`
- `resolved_install_profile_id`
- `resolved_agent_package_id`
- `resolved_install_script_id`
- `resolved_plugin_package_ids`
- `resolved_os_family`
- `resolved_os_distribution`
- `resolved_arch`
- `rendered_parameter_fingerprint`
- `created_at`

#### 字段说明

- `resolved_*` 字段表示当次任务最终命中的对象；
- `rendered_parameter_fingerprint` 用于描述脚本渲染参数的稳定摘要，不要求当前阶段就存储完整最终脚本内容。

这里要避免一个误区：

- `InstallResolution` 不是“新的任务对象”；
- 它是安装任务的解析快照。

## 对 NodeInstallTask 的扩展要求

在引入安装制品域后，`NodeInstallTask` 至少需要引用以下信息：

- `target_os_family`
- `target_os_distribution`
- `target_arch`
- `resolved_install_profile_id`
- `resolved_agent_package_id`
- `resolved_install_script_id`
- `resolved_plugin_package_ids`

这样做的目标是：

1. 安装任务本身具备关键信息检索能力；
2. 即使不单独展开 `InstallResolution`，也能在任务详情里看到足够多的信息；
3. 后续如果做失败重试、回滚分析，也能拿到明确输入。

## OS / 发行版 / 架构兼容性设计

### 1. 兼容性维度

安装制品选择至少要考虑三层：

- `os_family`
- `os_distribution`
- `arch`

推荐含义如下：

- `os_family`：大类系统，例如 `linux` / `windows` / `darwin`；
- `os_distribution`：Linux 下的具体发行版，如 `ubuntu` / `centos` / `rhel`；
- `arch`：CPU 架构，如 `amd64` / `arm64`。

### 2. 节点识别来源

当前推荐采用混合策略：

1. 创建节点时允许先录入候选平台信息；
2. 安装前通过 SSH 或等价机制进行真实探测；
3. 探测结果优先用于安装解析；
4. 如果探测失败，可保留人工值并进入人工确认流程。

### 3. 兼容性匹配原则

建议安装解析遵循以下匹配顺序：

1. 精确匹配 `os_family + os_distribution + arch`
2. 放宽到 `os_family + arch`
3. 若仍找不到，返回“无可用制品”错误

是否允许更复杂的优先级或通配规则，当前统一作为 **TBD**。

## 版本策略建议

### 1. 主包版本

`rsagent` 主包版本建议由 `install_profile` 作为默认选择中心。

这意味着：

- 平台可以按 OS / arch 维度默认不同版本；
- 安装任务如无特殊指定，优先使用 profile 中声明的版本；
- 如果任务级显式指定版本，任务级输入可以覆盖默认 profile。

### 2. 插件版本

插件包建议也沿用相同原则：

- 默认由 `install_profile` 给出；
- 安装任务可以附加覆盖；
- 是否允许插件与主包独立升级，当前视为 **TBD**。

### 3. 脚本版本

安装脚本版本建议独立于包版本管理。

原因是：

- 同一包版本可能在不同 OS 上需要不同脚本；
- 同一包版本的安装逻辑可能随脚本修复而变化；
- 把脚本版本和包版本强绑定会降低后续修复灵活性。

## 脚本设计建议

### 默认方向

当前推荐采用“模板为主，特殊平台固定脚本覆盖”的混合模式。

也就是：

- 大部分安装脚本可以抽象成带参数的模板；
- 少量特殊 OS 或特殊安装流程可以挂固定脚本实现；
- 安装任务最终记录的是“实际命中的脚本对象和参数摘要”。

### 不在本阶段冻结的内容

以下内容先不在本文中冻结：

- 模板语言是什么；
- 是否保存完整渲染后脚本文本；
- 是否把渲染结果做全量审计归档；
- 是否需要脚本签名和验签。

这些都留到后续协议或实现设计再展开。

## 凭据引用设计

当前建议把“凭据”建模为安装上下文的一部分，而不是安装制品的一部分。

也就是说：

- `InstallPackage` / `InstallScript` / `InstallArtifactProfile` 本身不保存凭据；
- 节点或节点组可以绑定 `credential_ref`；
- 安装任务在执行时解析出“当前应使用哪个凭据引用”。

这样设计的原因是：

- 凭据是访问上下文，不是制品内容；
- 把凭据直接塞进制品对象会增加耦合和安全风险。

## 安装解析流程

安装制品域引入后，节点纳管流程中的“安装解析”建议细化为如下步骤：

1. 确定目标节点；
2. 获取或探测目标节点 `os_family / os_distribution / arch`；
3. 读取节点级或任务级输入中的版本/插件覆盖项；
4. 匹配可用 `install_profile`；
5. 从 profile 中选出默认 `agent package`、`install script`、`plugin packages`；
6. 将任务级覆盖项应用到默认结果上；
7. 生成 `InstallResolution`；
8. 将解析结果写回安装任务；
9. 后续安装执行器按该结果执行。

## 与配置面的映射建议

为了兼容当前已经存在的配置项，建议先做以下映射约定。

### 1. `rsagent_package_url`

当前代码里的 `rsagent_package_url` 可以视为：

- 在还没有完整 `InstallPackage` 管理能力前，平台用于指定默认 agent 包地址的临时配置入口。

结合当前详细设计，建议把它进一步解释为：

- 该 URL 应指向一个可直接拉取的 `*.tar.gz` agent 安装包；
- 该安装包内默认应包含 `install.sh`、`restart.sh`、`uninstall.sh` 等生命周期脚本；
- 在没有外部 `InstallScript` 覆盖的情况下，安装器优先执行包内默认脚本。

后续如果 `InstallPackage` 落地，这个配置项可以被重新解释为：

- 默认 agent 包来源的全局兜底配置；
- 或迁移路径上的兼容字段。

### 2. `install_plugins`

README 中提到的 `install_plugins` 可以视为：

- 在 `InstallArtifactProfile.default_plugin_package_ids` 未落地前的简化配置表达。

但由于当前代码尚未完整收敛这一字段，因此不应把它当作稳定契约。

### 3. `register_callback_url`

`register_callback_url` 虽然不是制品对象本身，但它属于安装脚本渲染参数的重要输入。

因此后续如果脚本模板化：

- 它应属于安装解析上下文；
- 不应直接写进 `InstallScript` 对象本体。

如果安装流程默认执行的是包内 `install.sh`，那么 `register_callback_url` 也应作为该默认脚本的渲染参数或环境输入，而不是被硬编码进包本体。

## 第一阶段建议冻结的内容

如果要控制范围，第一阶段建议先冻结以下部分：

1. `InstallPackage` 基本元数据模型；
2. `InstallScript` 基本元数据模型；
3. `InstallArtifactProfile` 作为默认选择中心；
4. `InstallResolution` 作为安装任务解析快照；
5. `NodeInstallTask` 对解析结果的关键引用；
6. `os_family / os_distribution / arch` 三层兼容性维度；
7. `rsagent_package_url` 到 `InstallPackage` 的迁移解释。

## 第一阶段明确不冻结的内容

以下能力当前不建议在本文中做成硬约束：

1. 完整制品仓库实现；
2. 包签名和供应链安全机制；
3. 脚本模板引擎语法；
4. 完整渲染后脚本持久化格式；
5. 插件生态和插件生命周期管理；
6. 复杂通配规则、多级 fallback 策略；
7. 升级任务、回滚任务与首次安装任务的统一模型。

## 对实现结构的影响

如果后续进入代码实现，安装制品域建议从 `nodemanage` 主服务逻辑中独立出一个相对清晰的子域，而不是全部散落在安装任务 service 中。

可参考的职责拆分方向：

- `install_artifact.rs`：制品对象定义
- `install_profile.rs`：兼容性与选择规则
- `install_resolution.rs`：解析结果模型
- `install_resolver.rs`：制品解析逻辑

这样能保证：

- 模型清晰；
- 安装解析逻辑可测试；
- 未来补 API 契约时更容易对齐。

## Assumptions / TBD 汇总

以下内容在当前仓库中尚未正式落地，因此在本文中仍视为默认设计方向：

1. `InstallPackage`、`InstallScript`、`InstallArtifactProfile`、`InstallResolution` 的正式持久化模型；
2. `install_profile` 是否成为唯一默认选择中心；
3. 脚本模板化的具体实现方式；
4. 解析结果是否保留完整渲染脚本；
5. 任务级覆盖规则的精确优先级；
6. 插件包版本与主包版本的约束关系；
7. 兼容性匹配中的通配规则与排序策略；
8. 凭据引用对象的最终归属模型；
9. 安装失败后是否支持基于 `InstallResolution` 直接重试或回滚。

## 对齐参考文档

本文在以下文档约束下编写：

- `doc/nodemanage详细设计.md`
- `doc/nodemanage概述.md`
- `common/config/src/nodemanage.rs`
- `common/config/README.md`
- `config.example.toml`

后续如果 `nodemanage` 配置面继续演进，本文中“与配置面的映射建议”也需要同步更新。
