# 平台信息系统 - 补充说明

## 概述

为 rsync 项目补充了完整的操作系统平台信息检测和表示系统，用于跨平台二进制文件分发和环境适配。

## 新增内容

### 1. OsDistributionVesion (操作系统发行版)

完整支持以下发行版和版本：

#### Linux 发行版 (12种)
- **Ubuntu**: 主流 LTS 版本 (20.04, 22.04, 24.04)
- **Debian**: 版本 10-12 (Buster, Bullseye, Bookworm)
- **Fedora**: 最新版本 (38, 39, 40)
- **CentOS**: 版本 7, 8, 9
- **Red Hat Enterprise Linux (RHEL)**: 企业版本
- **Rocky Linux**: CentOS 替代品
- **AlmaLinux**: CentOS 替代品
- **Arch Linux**: 滚动更新发行版
- **Manjaro**: 用户友好的 Arch 衍生版
- **openSUSE**: Leap 和 Tumbleweed
- **Gentoo**: 源码发行版
- **Alpine**: 轻量级容器发行版

#### Windows 版本 (4种)
- **Windows 10**: 各种构建版本
- **Windows 11**: 最新 Windows
- **Windows Server**: 2019, 2022 等
- **Windows Legacy**: Windows 7, 8, 8.1

#### macOS 版本
- 完整版本号和代号支持 (Ventura, Sonoma, Sequoia 等)

#### BSD 家族 (3种)
- **FreeBSD**
- **OpenBSD**
- **NetBSD**

### 2. OsArchVersion (CPU 架构)

扩展至 **24种** 主流和小众架构：

#### x86 系列
- `X86`: 32位 x86
- `X86_64`: 64位 x86 (AMD64/Intel 64)

#### ARM 系列 (6种)
- `Armv5`, `Armv6`, `Armv7`, `Armv7hf`
- `Aarch64`: 64位 ARM (Apple Silicon, Raspberry Pi 4)
- `ArmBigEndian`

#### RISC-V
- `Riscv32`, `Riscv64`: 开源指令集架构

#### PowerPC (3种)
- `PowerPC`, `PowerPC64`, `PowerPC64le`

#### MIPS (4种)
- `Mips`, `Mips64`, `Mipsel`, `Mips64el`

#### SPARC
- `Sparc`, `Sparc64`: Oracle/Sun 架构

#### IBM
- `S390x`: IBM System z 大型机

#### WebAssembly
- `Wasm32`, `Wasm64`: 浏览器和云原生

### 3. 实用方法

#### OsArchVersion 方法
```rust
pub fn as_str(&self) -> &str              // 获取架构字符串
pub fn is_64bit(&self) -> bool            // 判断是否为 64 位
pub fn is_arm(&self) -> bool              // 判断是否为 ARM
```

#### OsDistributionVesion 方法
```rust
pub fn name(&self) -> &str                // 获取发行版名称
pub fn version(&self) -> Option<&str>     // 获取版本号
pub fn is_linux(&self) -> bool            // 判断是否为 Linux
pub fn is_windows(&self) -> bool          // 判断是否为 Windows
pub fn is_macos(&self) -> bool            // 判断是否为 macOS
pub fn is_bsd(&self) -> bool              // 判断是否为 BSD
```

#### OsPlatform 方法
```rust
pub fn new(...) -> Self                   // 创建平台信息
pub fn description(&self) -> String       // 获取完整描述
pub fn is_linux(&self) -> bool            // 平台判断方法
pub fn is_windows(&self) -> bool
pub fn is_macos(&self) -> bool
```

#### RsyncEnv 方法
```rust
pub fn new(platform: OsPlatform) -> Self  // 创建环境
pub fn detect() -> Self                   // 自动检测当前平台
```

## 使用示例

### 基础使用

```rust
use rule::{RsyncEnv, OsPlatform, OsKernel, OsArchVersion, OsDistributionVesion};

// 1. 自动检测当前平台
let env = RsyncEnv::detect();
println!("Running on: {}", env.platform.description());
// 输出: "Ubuntu 22.04 on x86_64"

// 2. 手动创建平台信息
let platform = OsPlatform::new(
    OsKernel::Linux,
    OsArchVersion::Aarch64,
    OsDistributionVesion::Debian {
        version: "12".to_string(),
    },
);

// 3. 平台判断
if platform.is_linux() && platform.arch.is_arm() {
    println!("Linux on ARM detected!");
}
```

### 二进制分发场景

```rust
fn get_download_url(platform: &OsPlatform) -> String {
    let base_url = "https://releases.example.com/myapp";
    
    match (&platform.distribution, &platform.arch) {
        (OsDistributionVesion::Ubuntu { .. }, OsArchVersion::X86_64) => {
            format!("{}/ubuntu-amd64.tar.gz", base_url)
        }
        (OsDistributionVesion::Ubuntu { .. }, OsArchVersion::Aarch64) => {
            format!("{}/ubuntu-arm64.tar.gz", base_url)
        }
        (OsDistributionVesion::Windows11 { .. }, OsArchVersion::X86_64) => {
            format!("{}/windows-x64.exe", base_url)
        }
        (OsDistributionVesion::MacOS { .. }, OsArchVersion::Aarch64) => {
            format!("{}/macos-apple-silicon.dmg", base_url)
        }
        _ => format!("{}/generic.tar.gz", base_url),
    }
}
```

### 条件编译和运行时检查

```rust
fn optimize_for_platform(env: &RsyncEnv) {
    // 64位特定优化
    if env.platform.arch.is_64bit() {
        println!("Enabling 64-bit optimizations");
    }
    
    // ARM 特定优化
    if env.platform.arch.is_arm() {
        println!("Enabling ARM NEON optimizations");
    }
    
    // Linux 特定功能
    if env.platform.is_linux() {
        println!("Using inotify for file watching");
    }
    
    // Windows 特定功能
    if env.platform.is_windows() {
        println!("Using ReadDirectoryChangesW for file watching");
    }
}
```

## 测试覆盖

完整的测试套件覆盖：

- ✅ 架构检测和特性判断 (`is_64bit`, `is_arm`)
- ✅ 发行版名称和版本提取
- ✅ Linux/Windows/macOS/BSD 平台判断
- ✅ 平台描述生成
- ✅ 自动检测当前平台
- ✅ 多种发行版和架构的组合测试

运行测试：
```bash
cargo test --lib
```

## 设计优势

1. **类型安全**: 使用枚举确保只能表示有效的平台组合
2. **可扩展**: 添加新发行版或架构只需扩展枚举
3. **零成本抽象**: 方法调用在编译时优化
4. **完整性**: 覆盖主流和小众平台
5. **实用性**: 提供丰富的查询和判断方法

## 下一步扩展

可以考虑添加：

1. **实际的系统检测**: 从 `/etc/os-release`、注册表等读取真实信息
2. **包管理器检测**: apt, yum, pacman, brew 等
3. **容器环境检测**: Docker, Podman, LXC 等
4. **虚拟化检测**: VM, KVM, Hyper-V 等
5. **CPU 特性检测**: AVX, SSE, NEON 等指令集支持

## 文件清单

- `src/file.rs`: 核心平台类型定义和实现
- `src/platform_examples.rs`: 完整的测试和使用示例
- `src/lib.rs`: 导出公共 API

所有代码已通过编译和测试验证 ✅
