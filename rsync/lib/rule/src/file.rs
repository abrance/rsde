use crate::event::*;
/// 实现简单的单文件 FileSource, JsonTransform, HttpSink, FileSink
///
/// 这个文件展示了 rsync 规则系统的使用方式
use crate::rule::*;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OsKernel {
    Linux,
    Windows,
    MacOS,
    FreeBSD,
    OpenBSD,
    NetBSD,
    Other(String),
}

impl OsKernel {
    pub fn as_str(&self) -> &str {
        match self {
            OsKernel::Linux => "linux",
            OsKernel::Windows => "windows",
            OsKernel::MacOS => "macos",
            OsKernel::FreeBSD => "freebsd",
            OsKernel::OpenBSD => "openbsd",
            OsKernel::NetBSD => "netbsd",
            OsKernel::Other(s) => s.as_str(),
        }
    }
}

/// 操作系统发行版及版本
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OsDistributionVesion {
    // Linux 发行版
    Ubuntu {
        version: String, // 版本号, 如 "20.04", "22.04", "24.04"
    },
    Debian {
        version: String, // 版本号, 如 "10", "11", "12" (Buster, Bullseye, Bookworm)
    },
    Fedora {
        version: String, // 版本号, 如 "38", "39", "40"
    },
    CentOS {
        version: String, // 版本号, 如 "7", "8", "9"
    },
    RedHat {
        version: String, // RHEL 版本号, 如 "8.5", "9.0"
    },
    Rocky {
        version: String, // Rocky Linux 版本号, 如 "8.5", "9.0"
    },
    AlmaLinux {
        version: String, // AlmaLinux 版本号, 如 "8.5", "9.0"
    },
    Arch {
        rolling: bool, // Arch 是滚动更新，标记是否为最新
    },
    Manjaro {
        version: String, // Manjaro 版本
    },
    OpenSUSE {
        version: String, // openSUSE 版本, 如 "Leap 15.4", "Tumbleweed"
    },
    Gentoo,
    Alpine {
        version: String, // Alpine 版本号, 如 "3.17", "3.18"
    },

    // Windows 版本
    Windows10 {
        build: String, // 如 "19041", "19042", "19043", "19044", "19045"
    },
    Windows11 {
        build: String, // 如 "22000", "22621", "22631"
    },
    WindowsServer {
        version: String, // 如 "2019", "2022"
        build: String,
    },
    WindowsLegacy {
        version: String, // 如 "7", "8", "8.1"
    },

    // macOS 版本
    MacOS {
        version: String,  // 版本号, 如 "13.0" (Ventura), "14.0" (Sonoma)
        codename: String, // 代号, 如 "Ventura", "Sonoma", "Sequoia"
    },

    // BSD 家族
    FreeBSD {
        version: String, // 如 "13.2", "14.0"
    },
    OpenBSD {
        version: String, // 如 "7.3", "7.4"
    },
    NetBSD {
        version: String,
    },

    // 其他
    Unknown,
    Other {
        name: String,
        version: Option<String>,
    },
}

/// CPU 架构版本
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OsArchVersion {
    // x86 系列
    X86,    // 32位 x86
    X86_64, // 64位 x86 (AMD64/Intel 64)

    // ARM 系列
    Armv5,   // ARMv5
    Armv6,   // ARMv6 (如 Raspberry Pi 1)
    Armv7,   // ARMv7 (32位 ARM, 如 Raspberry Pi 2/3)
    Armv7hf, // ARMv7 hard-float
    Aarch64, // 64位 ARM (ARMv8, 如 Apple Silicon, Raspberry Pi 4)

    // ARM 特殊变体
    ArmBigEndian, // ARM Big Endian

    // RISC-V
    Riscv32, // 32位 RISC-V
    Riscv64, // 64位 RISC-V

    // PowerPC
    PowerPC,     // 32位 PowerPC
    PowerPC64,   // 64位 PowerPC
    PowerPC64le, // 64位 PowerPC Little Endian

    // MIPS
    Mips,     // 32位 MIPS
    Mips64,   // 64位 MIPS
    Mipsel,   // MIPS Little Endian
    Mips64el, // MIPS64 Little Endian

    // SPARC
    Sparc,   // 32位 SPARC
    Sparc64, // 64位 SPARC

    // IBM
    S390x, // IBM System z (s390x)

    // WebAssembly
    Wasm32, // 32位 WebAssembly
    Wasm64, // 64位 WebAssembly

    // 其他
    Unknown,
    Other(String),
}

impl OsArchVersion {
    /// 获取架构的字符串表示
    pub fn as_str(&self) -> &str {
        match self {
            OsArchVersion::X86 => "x86",
            OsArchVersion::X86_64 => "x86_64",
            OsArchVersion::Armv5 => "armv5",
            OsArchVersion::Armv6 => "armv6",
            OsArchVersion::Armv7 => "armv7",
            OsArchVersion::Armv7hf => "armv7hf",
            OsArchVersion::Aarch64 => "aarch64",
            OsArchVersion::ArmBigEndian => "arm-be",
            OsArchVersion::Riscv32 => "riscv32",
            OsArchVersion::Riscv64 => "riscv64",
            OsArchVersion::PowerPC => "powerpc",
            OsArchVersion::PowerPC64 => "powerpc64",
            OsArchVersion::PowerPC64le => "powerpc64le",
            OsArchVersion::Mips => "mips",
            OsArchVersion::Mips64 => "mips64",
            OsArchVersion::Mipsel => "mipsel",
            OsArchVersion::Mips64el => "mips64el",
            OsArchVersion::Sparc => "sparc",
            OsArchVersion::Sparc64 => "sparc64",
            OsArchVersion::S390x => "s390x",
            OsArchVersion::Wasm32 => "wasm32",
            OsArchVersion::Wasm64 => "wasm64",
            OsArchVersion::Unknown => "unknown",
            OsArchVersion::Other(s) => s.as_str(),
        }
    }

    /// 判断是否为 64 位架构
    pub fn is_64bit(&self) -> bool {
        matches!(
            self,
            OsArchVersion::X86_64
                | OsArchVersion::Aarch64
                | OsArchVersion::PowerPC64
                | OsArchVersion::PowerPC64le
                | OsArchVersion::Mips64
                | OsArchVersion::Mips64el
                | OsArchVersion::Sparc64
                | OsArchVersion::S390x
                | OsArchVersion::Riscv64
                | OsArchVersion::Wasm64
        )
    }

    /// 判断是否为 ARM 架构
    pub fn is_arm(&self) -> bool {
        matches!(
            self,
            OsArchVersion::Armv5
                | OsArchVersion::Armv6
                | OsArchVersion::Armv7
                | OsArchVersion::Armv7hf
                | OsArchVersion::Aarch64
                | OsArchVersion::ArmBigEndian
        )
    }
}

impl OsDistributionVesion {
    /// 获取发行版的名称
    pub fn name(&self) -> &str {
        match self {
            OsDistributionVesion::Ubuntu { .. } => "Ubuntu",
            OsDistributionVesion::Debian { .. } => "Debian",
            OsDistributionVesion::Fedora { .. } => "Fedora",
            OsDistributionVesion::CentOS { .. } => "CentOS",
            OsDistributionVesion::RedHat { .. } => "Red Hat Enterprise Linux",
            OsDistributionVesion::Rocky { .. } => "Rocky Linux",
            OsDistributionVesion::AlmaLinux { .. } => "AlmaLinux",
            OsDistributionVesion::Arch { .. } => "Arch Linux",
            OsDistributionVesion::Manjaro { .. } => "Manjaro",
            OsDistributionVesion::OpenSUSE { .. } => "openSUSE",
            OsDistributionVesion::Gentoo => "Gentoo",
            OsDistributionVesion::Alpine { .. } => "Alpine Linux",
            OsDistributionVesion::Windows10 { .. } => "Windows 10",
            OsDistributionVesion::Windows11 { .. } => "Windows 11",
            OsDistributionVesion::WindowsServer { .. } => "Windows Server",
            OsDistributionVesion::WindowsLegacy { .. } => "Windows",
            OsDistributionVesion::MacOS { .. } => "macOS",
            OsDistributionVesion::FreeBSD { .. } => "FreeBSD",
            OsDistributionVesion::OpenBSD { .. } => "OpenBSD",
            OsDistributionVesion::NetBSD { .. } => "NetBSD",
            OsDistributionVesion::Unknown => "Unknown",
            OsDistributionVesion::Other { name, .. } => name.as_str(),
        }
    }

    /// 获取发行版的版本号（如果有）
    pub fn version(&self) -> Option<&str> {
        match self {
            OsDistributionVesion::Ubuntu { version }
            | OsDistributionVesion::Debian { version }
            | OsDistributionVesion::Fedora { version }
            | OsDistributionVesion::CentOS { version }
            | OsDistributionVesion::RedHat { version }
            | OsDistributionVesion::Rocky { version }
            | OsDistributionVesion::AlmaLinux { version }
            | OsDistributionVesion::Manjaro { version }
            | OsDistributionVesion::OpenSUSE { version }
            | OsDistributionVesion::Alpine { version }
            | OsDistributionVesion::FreeBSD { version }
            | OsDistributionVesion::OpenBSD { version }
            | OsDistributionVesion::NetBSD { version }
            | OsDistributionVesion::WindowsLegacy { version } => Some(version.as_str()),
            OsDistributionVesion::Windows10 { build }
            | OsDistributionVesion::Windows11 { build } => Some(build.as_str()),
            OsDistributionVesion::WindowsServer { version, .. } => Some(version.as_str()),
            OsDistributionVesion::MacOS { version, .. } => Some(version.as_str()),
            OsDistributionVesion::Other { version, .. } => version.as_deref(),
            _ => None,
        }
    }

    /// 判断是否为 Linux 发行版
    pub fn is_linux(&self) -> bool {
        matches!(
            self,
            OsDistributionVesion::Ubuntu { .. }
                | OsDistributionVesion::Debian { .. }
                | OsDistributionVesion::Fedora { .. }
                | OsDistributionVesion::CentOS { .. }
                | OsDistributionVesion::RedHat { .. }
                | OsDistributionVesion::Rocky { .. }
                | OsDistributionVesion::AlmaLinux { .. }
                | OsDistributionVesion::Arch { .. }
                | OsDistributionVesion::Manjaro { .. }
                | OsDistributionVesion::OpenSUSE { .. }
                | OsDistributionVesion::Gentoo
                | OsDistributionVesion::Alpine { .. }
        )
    }

    /// 判断是否为 Windows
    pub fn is_windows(&self) -> bool {
        matches!(
            self,
            OsDistributionVesion::Windows10 { .. }
                | OsDistributionVesion::Windows11 { .. }
                | OsDistributionVesion::WindowsServer { .. }
                | OsDistributionVesion::WindowsLegacy { .. }
        )
    }

    /// 判断是否为 macOS
    pub fn is_macos(&self) -> bool {
        matches!(self, OsDistributionVesion::MacOS { .. })
    }

    /// 判断是否为 BSD 系统
    pub fn is_bsd(&self) -> bool {
        matches!(
            self,
            OsDistributionVesion::FreeBSD { .. }
                | OsDistributionVesion::OpenBSD { .. }
                | OsDistributionVesion::NetBSD { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OsPlatform {
    pub kernel: OsKernel,
    pub arch: OsArchVersion,
    pub distribution: OsDistributionVesion, // 发行版 如 Ubuntu, Fedora, Windows, MacOS, Debian
}

impl OsPlatform {
    /// 创建新的平台信息
    pub fn new(kernel: OsKernel, arch: OsArchVersion, distribution: OsDistributionVesion) -> Self {
        Self {
            kernel,
            arch,
            distribution,
        }
    }

    /// 获取平台的完整描述
    pub fn description(&self) -> String {
        format!(
            "{} {} on {}",
            self.distribution.name(),
            self.distribution.version().unwrap_or("unknown"),
            self.arch.as_str()
        )
    }

    /// 判断当前平台是否为 Linux
    pub fn is_linux(&self) -> bool {
        matches!(self.kernel, OsKernel::Linux) && self.distribution.is_linux()
    }

    /// 判断当前平台是否为 Windows
    pub fn is_windows(&self) -> bool {
        matches!(self.kernel, OsKernel::Windows) && self.distribution.is_windows()
    }

    /// 判断当前平台是否为 macOS
    pub fn is_macos(&self) -> bool {
        matches!(self.kernel, OsKernel::MacOS) && self.distribution.is_macos()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RsyncEnv {
    pub platform: OsPlatform,
}

impl RsyncEnv {
    pub fn new(platform: OsPlatform) -> Self {
        Self { platform }
    }

    /// 检测当前系统的平台信息
    #[cfg(target_os = "linux")]
    pub fn detect() -> Self {
        // 这里可以实际检测系统信息
        // 简化示例
        Self::new(OsPlatform::new(
            OsKernel::Linux,
            if cfg!(target_arch = "x86_64") {
                OsArchVersion::X86_64
            } else if cfg!(target_arch = "aarch64") {
                OsArchVersion::Aarch64
            } else {
                OsArchVersion::Unknown
            },
            OsDistributionVesion::Unknown,
        ))
    }

    #[cfg(target_os = "windows")]
    pub fn detect() -> Self {
        Self::new(OsPlatform::new(
            OsKernel::Windows,
            if cfg!(target_arch = "x86_64") {
                OsArchVersion::X86_64
            } else {
                OsArchVersion::Unknown
            },
            OsDistributionVesion::Unknown,
        ))
    }

    #[cfg(target_os = "macos")]
    pub fn detect() -> Self {
        Self::new(OsPlatform::new(
            OsKernel::MacOS,
            if cfg!(target_arch = "aarch64") {
                OsArchVersion::Aarch64
            } else if cfg!(target_arch = "x86_64") {
                OsArchVersion::X86_64
            } else {
                OsArchVersion::Unknown
            },
            OsDistributionVesion::MacOS {
                version: "unknown".to_string(),
                codename: "unknown".to_string(),
            },
        ))
    }
}

/// 文件数据源配置（可序列化）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSourceConfig {
    pub path: String,
    pub watch: bool, // 是否监听文件变化
}

impl FileSourceConfig {
    pub fn new(path: String, watch: bool) -> Self {
        Self { path, watch }
    }
}

#[typetag::serde(name = "file")]
#[async_trait]
impl Source for FileSourceConfig {
    fn clone_box(&self) -> Box<dyn Source> {
        Box::new(self.clone())
    }

    fn outputs(&self) -> Vec<SourceOutput> {
        vec![SourceOutput {
            output_id: "file_output".to_string(),
            event_type: EventType::Text(TextType::PlainText),
        }]
    }

    async fn build(&self, _cx: SourceContext) -> Result<Box<dyn SourceRuntime>> {
        Ok(Box::new(FileSourceRuntime {
            path: self.path.clone(),
            current_offset: 0,
            fd: std::fs::File::open(&self.path)?,
            watch: self.watch,
        }))
    }

    fn can_acknowledge(&self) -> bool {
        false // 文件源不需要确认机制
    }

    fn source_type(&self) -> &str {
        "file"
    }
}

/// 文件数据源运行时实例
pub struct FileSourceRuntime {
    path: String,
    fd: std::fs::File,
    current_offset: u64,
    watch: bool,
}

#[async_trait]
impl SourceRuntime for FileSourceRuntime {
    async fn next_event(&mut self) -> Result<Option<Box<dyn Event>>> {
        use std::io::{Read, Seek};

        loop {
            // 获取文件元数据
            let metadata = self.fd.metadata()?;

            // 如果已经读到文件末尾
            if self.current_offset >= metadata.len() {
                if !self.watch {
                    return Ok(None);
                }
                // 等待文件变化
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                continue;
            }

            // 计算本次要读取的字节数（最多 64KB）
            const READ_SIZE: u64 = 64 * 1024;
            let to_read = std::cmp::min(READ_SIZE, metadata.len() - self.current_offset);

            // 读取数据
            let payload = {
                let mut buffer = vec![0u8; to_read as usize];
                self.fd
                    .seek(std::io::SeekFrom::Start(self.current_offset))?;
                self.fd.read_exact(&mut buffer)?;
                buffer
            };

            // 更新偏移量（增加实际读取的字节数）
            self.current_offset += to_read;

            // 返回事件
            return Ok(Some(Box::new(SimpleEvent {
                metadata: EventMetadata {
                    id: format!("file-{}", self.current_offset),
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    name: self.path.clone(),
                    payload_size: payload.len(),
                    event_type: EventType::Binary(BinaryType::Generic),
                },
                payload,
            })));
        }
    }
}

/// JSON 格式转换器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonTransformConfig {
    pub add_timestamp: bool,
}

#[typetag::serde(name = "json")]
#[async_trait]
impl Transform for JsonTransformConfig {
    fn clone_box(&self) -> Box<dyn Transform> {
        Box::new(self.clone())
    }

    async fn build(&self, _cx: TransformContext) -> Result<Box<dyn TransformRuntime>> {
        Ok(Box::new(JsonTransformRuntime {
            _add_timestamp: self.add_timestamp,
        }))
    }

    fn transform_type(&self) -> &str {
        "json"
    }
}

/// JSON 转换器运行时
pub struct JsonTransformRuntime {
    _add_timestamp: bool,
}

#[async_trait]
impl TransformRuntime for JsonTransformRuntime {
    async fn process(&mut self, event: Box<dyn Event>) -> Result<Vec<Box<dyn Event>>> {
        // 实际实现中，这里会进行 JSON 转换
        // 这里仅作示例，直接返回原事件
        Ok(vec![event])
    }
}

/// HTTP 目标配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpSinkConfig {
    pub url: String,
    pub batch_size: usize,
}

#[typetag::serde(name = "http")]
#[async_trait]
impl Sink for HttpSinkConfig {
    fn clone_box(&self) -> Box<dyn Sink> {
        Box::new(self.clone())
    }

    async fn build(&self, _cx: SinkContext) -> Result<Box<dyn SinkRuntime>> {
        Ok(Box::new(HttpSinkRuntime {
            url: self.url.clone(),
            batch_size: self.batch_size,
            buffer: Vec::new(),
        }))
    }

    fn sink_type(&self) -> &str {
        "http"
    }
}

/// HTTP Sink 运行时
pub struct HttpSinkRuntime {
    url: String,
    batch_size: usize,
    buffer: Vec<Box<dyn Event>>,
}

#[async_trait]
impl SinkRuntime for HttpSinkRuntime {
    async fn write(&mut self, event: Box<dyn Event>) -> Result<()> {
        self.buffer.push(event);

        if self.buffer.len() >= self.batch_size {
            self.flush().await?;
        }

        Ok(())
    }

    async fn flush(&mut self) -> Result<()> {
        // 实际实现中，这里会发送 HTTP 请求
        println!("Flushing {} events to {}", self.buffer.len(), self.url);
        self.buffer.clear();
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileSinkConfig {
    pub env: RsyncEnv,
    pub path: String,
    pub force: bool,          // 是否覆盖已存在文件
    pub mask: Option<String>, // 可选的文件名掩码
}

pub struct FileSinkRuntime {
    pub env: RsyncEnv,
    pub fd: std::fs::File,
    pub current_offset: u64,
}

use std::io::Write;
#[async_trait]
impl SinkRuntime for FileSinkRuntime {
    async fn write(&mut self, event: Box<dyn Event>) -> Result<()> {
        let payload = event.get_payload();

        if let Err(e) = self.fd.write_all(&payload) {
            return Err(e.into());
        }
        self.current_offset += payload.len() as u64;
        Ok(())
    }
}

#[typetag::serde(name = "file")]
#[async_trait]
impl Sink for FileSinkConfig {
    fn clone_box(&self) -> Box<dyn Sink> {
        Box::new(self.clone())
    }

    async fn build(&self, _cx: SinkContext) -> Result<Box<dyn SinkRuntime>> {
        Ok(Box::new(FileSinkRuntime {
            env: self.env.clone(),
            fd: std::fs::File::create(&self.path)?,
            current_offset: 0,
        }))
    }

    fn sink_type(&self) -> &str {
        "file"
    }
}

impl FileSinkConfig {
    pub fn new(env: RsyncEnv, path: String, force: bool, mask: Option<String>) -> Self {
        Self {
            env,
            path,
            force,
            mask,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[tokio::test]
    async fn test_pipeline() {
        // 创建临时文件用于测试
        let temp_dir = std::env::temp_dir();
        let input_path = temp_dir.join("test_input.txt");
        let output_path = temp_dir.join("test_output.txt");

        // 写入测试数据
        let mut file = std::fs::File::create(&input_path).unwrap();
        writeln!(file, "test line 1").unwrap();
        writeln!(file, "test line 2").unwrap();
        drop(file);

        // 1. 创建配置
        let source_config = FileSourceConfig {
            path: input_path.to_string_lossy().to_string(),
            watch: false,
        };

        let transform_config = JsonTransformConfig {
            add_timestamp: true,
        };

        let sink_config = FileSinkConfig {
            path: output_path.to_string_lossy().to_string(),
            force: true,
            mask: None,
            env: RsyncEnv::detect(),
        };

        // 2. 构建运行时实例
        let mut source = source_config
            .build(SourceContext {
                key: ComponentKey::from("source-1".to_string()),
                acknowledgements: false,
            })
            .await
            .unwrap();

        let mut transform = transform_config
            .build(TransformContext {
                key: ComponentKey::from("transform-1".to_string()),
            })
            .await
            .unwrap();

        let mut sink = sink_config
            .build(SinkContext {
                key: ComponentKey::from("sink-1".to_string()),
                acknowledgements: false,
            })
            .await
            .unwrap();

        // 3. 执行数据流
        while let Some(event) = source.next_event().await.unwrap() {
            // 转换
            let transformed_events = transform.process(event).await.unwrap();

            // 写入
            for event in transformed_events {
                sink.write(event).await.unwrap();
            }
        }

        // 4. 清理
        sink.shutdown().await.unwrap();

        // 验证输出文件存在
        assert!(output_path.exists());

        // 清理测试文件
        let _ = std::fs::remove_file(&input_path);
        let _ = std::fs::remove_file(&output_path);
    }
}
