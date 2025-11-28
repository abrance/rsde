/// 平台信息使用示例
use crate::file::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_os_arch_version() {
        // 测试架构方法
        let arch = OsArchVersion::X86_64;
        assert_eq!(arch.as_str(), "x86_64");
        assert!(arch.is_64bit());
        assert!(!arch.is_arm());

        let arm_arch = OsArchVersion::Aarch64;
        assert!(arm_arch.is_64bit());
        assert!(arm_arch.is_arm());
    }

    #[test]
    fn test_os_distribution() {
        // Ubuntu 示例
        let ubuntu = OsDistributionVesion::Ubuntu {
            version: "22.04".to_string(),
        };
        assert_eq!(ubuntu.name(), "Ubuntu");
        assert_eq!(ubuntu.version(), Some("22.04"));
        assert!(ubuntu.is_linux());
        assert!(!ubuntu.is_windows());

        // Windows 11 示例
        let win11 = OsDistributionVesion::Windows11 {
            build: "22621".to_string(),
        };
        assert_eq!(win11.name(), "Windows 11");
        assert!(win11.is_windows());
        assert!(!win11.is_linux());

        // macOS 示例
        let macos = OsDistributionVesion::MacOS {
            version: "14.0".to_string(),
            codename: "Sonoma".to_string(),
        };
        assert_eq!(macos.name(), "macOS");
        assert!(macos.is_macos());
        assert!(!macos.is_linux());
    }

    #[test]
    fn test_platform() {
        // 创建一个 Ubuntu 平台
        let platform = OsPlatform::new(
            OsKernel::Linux,
            OsArchVersion::X86_64,
            OsDistributionVesion::Ubuntu {
                version: "22.04".to_string(),
            },
        );

        assert!(platform.is_linux());
        assert!(!platform.is_windows());
        assert_eq!(platform.description(), "Ubuntu 22.04 on x86_64");
    }

    #[test]
    fn test_rsync_env() {
        // 创建环境
        let platform = OsPlatform::new(
            OsKernel::Linux,
            OsArchVersion::Aarch64,
            OsDistributionVesion::Debian {
                version: "12".to_string(),
            },
        );

        let env = RsyncEnv::new(platform);
        assert_eq!(env.platform.description(), "Debian 12 on aarch64");
    }

    #[test]
    fn test_detect_current_platform() {
        // 检测当前平台
        let env = RsyncEnv::detect();
        println!("Current platform: {}", env.platform.description());

        // 验证检测的平台信息 - 只验证内核类型，因为发行版检测可能返回 Unknown
        #[cfg(target_os = "linux")]
        {
            // Linux 系统应该有 Linux 内核
            assert_eq!(env.platform.kernel.as_str(), "linux");
        }

        #[cfg(target_os = "windows")]
        {
            assert_eq!(env.platform.kernel.as_str(), "windows");
        }

        #[cfg(target_os = "macos")]
        {
            assert_eq!(env.platform.kernel.as_str(), "macos");
        }
    }

    #[test]
    fn test_comprehensive_distributions() {
        // 测试各种 Linux 发行版
        let distros = vec![
            OsDistributionVesion::Ubuntu {
                version: "24.04".to_string(),
            },
            OsDistributionVesion::Fedora {
                version: "40".to_string(),
            },
            OsDistributionVesion::RedHat {
                version: "9.0".to_string(),
            },
            OsDistributionVesion::Alpine {
                version: "3.18".to_string(),
            },
            OsDistributionVesion::Arch { rolling: true },
        ];

        for distro in distros {
            assert!(distro.is_linux());
            println!("{}: {:?}", distro.name(), distro.version());
        }
    }

    #[test]
    fn test_comprehensive_architectures() {
        // 测试各种架构
        let architectures = vec![
            (OsArchVersion::X86_64, true, false),
            (OsArchVersion::X86, false, false),
            (OsArchVersion::Aarch64, true, true),
            (OsArchVersion::Armv7, false, true),
            (OsArchVersion::Riscv64, true, false),
            (OsArchVersion::Wasm32, false, false),
        ];

        for (arch, is_64bit, is_arm) in architectures {
            assert_eq!(arch.is_64bit(), is_64bit);
            assert_eq!(arch.is_arm(), is_arm);
            println!("{}: 64bit={}, ARM={}", arch.as_str(), is_64bit, is_arm);
        }
    }

    #[test]
    fn test_windows_versions() {
        let win_versions = vec![
            OsDistributionVesion::Windows10 {
                build: "19045".to_string(),
            },
            OsDistributionVesion::Windows11 {
                build: "22631".to_string(),
            },
            OsDistributionVesion::WindowsServer {
                version: "2022".to_string(),
                build: "20348".to_string(),
            },
        ];

        for win in win_versions {
            assert!(win.is_windows());
            println!("{}: {}", win.name(), win.version().unwrap_or("unknown"));
        }
    }

    #[test]
    fn test_bsd_systems() {
        let bsd_systems = vec![
            OsDistributionVesion::FreeBSD {
                version: "14.0".to_string(),
            },
            OsDistributionVesion::OpenBSD {
                version: "7.4".to_string(),
            },
            OsDistributionVesion::NetBSD {
                version: "10.0".to_string(),
            },
        ];

        for bsd in bsd_systems {
            assert!(bsd.is_bsd());
            assert!(!bsd.is_linux());
            println!("{}: {}", bsd.name(), bsd.version().unwrap_or("unknown"));
        }
    }
}

/// 实际使用示例
pub fn example_usage() {
    // 1. 检测当前平台
    let env = RsyncEnv::detect();
    println!("Running on: {}", env.platform.description());

    // 2. 根据平台执行不同逻辑
    if env.platform.is_linux() {
        println!("Linux specific logic");
        if env.platform.arch.is_arm() {
            println!("ARM architecture detected");
        }
    } else if env.platform.is_windows() {
        println!("Windows specific logic");
    } else if env.platform.is_macos() {
        println!("macOS specific logic");
    }

    // 3. 手动创建平台信息
    let custom_platform = OsPlatform::new(
        OsKernel::Linux,
        OsArchVersion::Aarch64,
        OsDistributionVesion::Ubuntu {
            version: "22.04".to_string(),
        },
    );

    println!("Custom platform: {}", custom_platform.description());

    // 4. 检查架构特性
    if custom_platform.arch.is_64bit() {
        println!("64-bit architecture");
    }
}
