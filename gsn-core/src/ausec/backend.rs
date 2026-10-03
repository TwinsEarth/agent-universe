//! AUSec 执行后端模型（v3.7.0 基座）。
//!
//! # 四种执行后端
//!
//! 见 `docs/ausec/AUSEC-DESIGN.md`：[`ExecutionBackend`] 统一抽象
//! FnCall / Container / MicroVM / Full VM，插件分级（[`Tier`]）与风险评级
//! 决定默认后端，后端再映射到 v3.0.0 的隔离强度。
//!
//! # 诚实边界（务必读完）
//!
//! 本文件区分两件**绝不能混为一谈**的事：
//!
//! 1. [`primitive_availability`]：**当前平台是否具备该后端所需的 OS 原语/运行时**
//!    （docker/podman、`/dev/kvm`、Firecracker、Hyper-V、Apple Virtualization）。
//!    这只是"平台理论上可行"。
//! 2. [`EXECUTOR_WIRED`]：**本仓是否真正实现了该后端的执行器**。
//!
//! v3.7.0 只交付"选择 + 平台可用性声明"。除进程内 [`ExecutionBackend::FnCall`]
//! 外，Container/MicroVM/FullVM 的真实执行器**尚未接线**；即使平台原语可用，
//! [`readiness`] 也返回 [`Readiness::ExecutorNotWired`]，绝不允许把"探测到 docker"
//! 谎报成"容器沙箱已能安全执行"。真正执行在后续补丁按"探测成功才 Available、
//! 否则具名拒绝"接入（沿用 `crate::sandbox` 的能力声明模式）。
//!
//! 所有"参考启动量级"均为设计目标/外部资料数量级，**不是本仓实测基准**
//! （CI 在三平台无 Docker/KVM/GUI 环境），见 [`ExecutionBackend::startup_hint`]。

use crate::plugin::tier::Tier;
use serde::{Deserialize, Serialize};

/// 运行平台（用于后端可行性判定）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Linux,
    MacOs,
    Windows,
    Other,
}

impl Platform {
    /// 编译期当前平台。
    pub fn current() -> Platform {
        #[cfg(target_os = "linux")]
        {
            Platform::Linux
        }
        #[cfg(target_os = "macos")]
        {
            Platform::MacOs
        }
        #[cfg(target_os = "windows")]
        {
            Platform::Windows
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Platform::Other
        }
    }
}

/// 平台原语/运行时探测结果（由调用方在运行时探测后注入，便于确定性测试）。
///
/// 全部默认 `false`：**未探测即视为不具备**（fail-closed），不允许凭平台名
/// 乐观假设运行时存在。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformFeatures {
    /// 存在 docker / podman 等容器运行时（PATH 可执行）。
    pub container_runtime: bool,
    /// `/dev/kvm` 可访问（Linux）。
    pub dev_kvm: bool,
    /// firecracker 二进制可执行（Linux）。
    pub firecracker: bool,
    /// Hyper-V 可用（Windows）。
    pub hyper_v: bool,
    /// Apple Virtualization.framework 可用（macOS）。
    pub apple_virtualization: bool,
}

/// 四种执行后端。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionBackend {
    /// 进程内函数派发：极低开销，**无 OS 级隔离**，仅可信（System）短任务。
    FnCall,
    /// 容器（共享宿主内核）：软件工程 / 工具调用。
    Container,
    /// MicroVM（Firecracker/KVM）：强隔离。
    MicroVm,
    /// 完整虚拟机：完整 OS / GUI / 图形渲染 / Android，隔离最强、开销最大。
    FullVm,
}

impl ExecutionBackend {
    /// 规范标识（用于 PMB / 状态序列化）。
    pub fn as_str(self) -> &'static str {
        match self {
            ExecutionBackend::FnCall => "fncall",
            ExecutionBackend::Container => "container",
            ExecutionBackend::MicroVm => "microvm",
            ExecutionBackend::FullVm => "fullvm",
        }
    }

    /// 从字符串解析。
    pub fn parse(s: &str) -> Option<ExecutionBackend> {
        Some(match s {
            "fncall" => ExecutionBackend::FnCall,
            "container" => ExecutionBackend::Container,
            "microvm" => ExecutionBackend::MicroVm,
            "fullvm" => ExecutionBackend::FullVm,
            _ => return None,
        })
    }

    pub fn all() -> [ExecutionBackend; 4] {
        [
            ExecutionBackend::FnCall,
            ExecutionBackend::Container,
            ExecutionBackend::MicroVm,
            ExecutionBackend::FullVm,
        ]
    }

    /// 参考启动量级——**设计目标/外部资料数量级，非本仓实测**。
    pub fn startup_hint(self) -> &'static str {
        match self {
            ExecutionBackend::FnCall => "<1ms 级（设计目标，非本仓实测）",
            ExecutionBackend::Container => "毫秒级（设计目标，非本仓实测）",
            ExecutionBackend::MicroVm => "十毫秒级（设计目标，非本仓实测）",
            ExecutionBackend::FullVm => "百毫秒级（设计目标，非本仓实测）",
        }
    }

    /// 该后端是否提供 OS/虚拟化级隔离（FnCall 明确为"无"）。
    pub fn os_isolation(self) -> OsIsolation {
        match self {
            // 进程内函数派发不提供任何 OS 边界——必须如实标注，禁止用于不可信代码。
            ExecutionBackend::FnCall => OsIsolation::NoneInProcess,
            ExecutionBackend::Container => OsIsolation::SharedKernelContainer,
            ExecutionBackend::MicroVm => OsIsolation::GuestKernelMicroVm,
            ExecutionBackend::FullVm => OsIsolation::GuestKernelFullVm,
        }
    }

    /// 本仓是否已接线该后端的**真实执行器**。
    ///
    /// v3.7.0：仅 FnCall 作为选择/声明就绪；容器与虚拟机执行器在后续补丁按
    /// "探测成功才可用、否则具名拒绝"接入。此表是防止"文档承诺 > 代码事实"的
    /// 编译期事实，测试会锁定它。
    pub fn executor_wired(self) -> bool {
        match self {
            ExecutionBackend::FnCall => true,
            ExecutionBackend::Container | ExecutionBackend::MicroVm | ExecutionBackend::FullVm => {
                false
            }
        }
    }
}

/// OS 隔离强度（诚实标注，供能力声明与审计使用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OsIsolation {
    /// 无隔离：宿主进程内函数调用。
    NoneInProcess,
    /// 共享宿主内核（容器）。
    SharedKernelContainer,
    /// 独立客户内核（MicroVM）。
    GuestKernelMicroVm,
    /// 独立客户内核 + 完整设备模型（Full VM）。
    GuestKernelFullVm,
}

/// 第三方插件风险评级（决定 MicroVM 还是 Full VM）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskGrade {
    /// 标准风险 → MicroVM。
    #[default]
    Standard,
    /// 高风险（需图形界面 / 来源敏感）→ Full VM。
    Elevated,
}

/// 平台**原语**可行性（不含"执行器是否接线"）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrimitiveAvailability {
    /// 平台具备所需原语/运行时。
    Available,
    /// 平台可能支持，但当前未探测到前置条件（fail-closed，未探测即此态）。
    NeedsProbe { precondition: &'static str },
    /// 该平台根本没有对应原语。
    Unsupported { reason: &'static str },
}

/// 后端就绪状态（综合平台原语 + 执行器接线）。这是**唯一**应被上层用来判断
/// "现在能不能真的在这个后端上跑"的类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Readiness {
    /// 可立即使用（平台原语具备且执行器已接线）。
    Ready,
    /// 平台具备/可能具备原语，但本仓执行器尚未接线（v3.7.0 的 Container/VM 均为此态）。
    ExecutorNotWired { primitive: PrimitiveAvailability },
    /// 需要先满足前置条件（探测运行时 / 权限）。
    NeedsProbe { precondition: String },
    /// 平台无此原语，具名拒绝。
    Unsupported { reason: String },
}

impl Readiness {
    /// 是否现在就能真实执行。v3.7.0 只有 FnCall 在所有平台 Ready。
    pub fn can_run_now(&self) -> bool {
        matches!(self, Readiness::Ready)
    }
}

/// 依据插件分级与风险评级选择默认执行后端。
///
/// 映射（设计文档 §1）：
/// System → FnCall；Official → Container；Certified → MicroVM；
/// ThirdParty 标准风险 → MicroVM，高风险 → FullVm。
///
/// 黑名单插件**不可调度**：返回 `None`（黑名单在宿主安装闸门已被拒，这里再兜底）。
pub fn backend_for_tier(tier: Tier, risk: RiskGrade) -> Option<ExecutionBackend> {
    Some(match tier {
        Tier::System => ExecutionBackend::FnCall,
        Tier::Official => ExecutionBackend::Container,
        Tier::Certified => ExecutionBackend::MicroVm,
        Tier::ThirdParty => match risk {
            RiskGrade::Standard => ExecutionBackend::MicroVm,
            RiskGrade::Elevated => ExecutionBackend::FullVm,
        },
        Tier::Blacklist => return None,
    })
}

/// 判定某后端在指定平台 + 探测特征下的**原语可行性**（纯函数，确定性可测）。
pub fn primitive_availability(
    platform: Platform,
    f: PlatformFeatures,
    backend: ExecutionBackend,
) -> PrimitiveAvailability {
    use PrimitiveAvailability as P;
    match backend {
        // 进程内派发不依赖任何外部原语。
        ExecutionBackend::FnCall => P::Available,
        ExecutionBackend::Container => {
            if f.container_runtime {
                P::Available
            } else {
                // 三平台桌面 Docker 都可能存在；未探测到统一报前置条件（不按平台硬拒）。
                P::NeedsProbe {
                    precondition: "需要可执行的 docker 或 podman 容器运行时",
                }
            }
        }
        ExecutionBackend::MicroVm => match platform {
            Platform::Linux => {
                if f.dev_kvm && f.firecracker {
                    P::Available
                } else if f.dev_kvm {
                    P::NeedsProbe {
                        precondition: "/dev/kvm 可用，但未探测到 firecracker 二进制",
                    }
                } else {
                    P::NeedsProbe {
                        precondition: "需要可访问的 /dev/kvm（权限）与 firecracker",
                    }
                }
            }
            // Firecracker/KVM 仅 Linux：macOS/Windows/其它无此原语，具名拒绝，不降级冒充。
            _ => P::Unsupported {
                reason: "MicroVM(Firecracker/KVM) 仅支持 Linux；本平台无对应原语",
            },
        },
        ExecutionBackend::FullVm => match platform {
            Platform::Linux => {
                if f.dev_kvm {
                    P::Available
                } else {
                    P::NeedsProbe {
                        precondition: "Linux FullVM 需要可访问的 /dev/kvm",
                    }
                }
            }
            Platform::Windows => {
                if f.hyper_v {
                    P::Available
                } else {
                    P::NeedsProbe {
                        precondition: "需要启用 Hyper-V / 虚拟化平台",
                    }
                }
            }
            Platform::MacOs => {
                if f.apple_virtualization {
                    P::Available
                } else {
                    P::NeedsProbe {
                        precondition: "需要 Apple Virtualization.framework（Apple Silicon）",
                    }
                }
            }
            Platform::Other => P::Unsupported {
                reason: "未知平台，无已知 FullVM 虚拟化原语",
            },
        },
    }
}

/// 综合判定：平台原语 + 执行器是否接线 → 真实就绪状态。
pub fn readiness(
    platform: Platform,
    features: PlatformFeatures,
    backend: ExecutionBackend,
) -> Readiness {
    let primitive = primitive_availability(platform, features, backend);
    if !backend.executor_wired() {
        // 即使平台原语齐备，执行器没接线就不能跑——防止"探测到 docker 即声称容器沙箱可用"。
        return Readiness::ExecutorNotWired { primitive };
    }
    match primitive {
        PrimitiveAvailability::Available => Readiness::Ready,
        PrimitiveAvailability::NeedsProbe { precondition } => Readiness::NeedsProbe {
            precondition: precondition.to_string(),
        },
        PrimitiveAvailability::Unsupported { reason } => Readiness::Unsupported {
            reason: reason.to_string(),
        },
    }
}

/// 运行时探测平台特征（真实 PATH / 设备检查）。
///
/// 只做"运行时/设备是否存在"的**事实探测**，不代表沙箱能安全执行（后者由
/// [`readiness`] 综合 `executor_wired` 决定）。探测失败一律按 `false`（fail-closed）。
pub fn detect_features() -> PlatformFeatures {
    fn on_path(cmd: &str) -> bool {
        std::env::var_os("PATH")
            .is_some_and(|paths| std::env::split_paths(&paths).any(|d| d.join(cmd).is_file()))
    }
    let container_runtime = on_path(if cfg!(windows) {
        "docker.exe"
    } else {
        "docker"
    }) || on_path(if cfg!(windows) {
        "podman.exe"
    } else {
        "podman"
    });

    #[cfg(target_os = "linux")]
    {
        let dev_kvm = std::path::Path::new("/dev/kvm").exists();
        let firecracker = on_path("firecracker");
        PlatformFeatures {
            container_runtime,
            dev_kvm,
            firecracker,
            hyper_v: false,
            apple_virtualization: false,
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        // 非 Linux：KVM/Firecracker 恒为 false；Hyper-V / Apple VZ 的可靠检测需要
        // 平台 API，v3.7.0 不做乐观假设，留作 NeedsProbe（由操作员/后续补丁确证）。
        PlatformFeatures {
            container_runtime,
            ..PlatformFeatures::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_to_backend_mapping() {
        assert_eq!(
            backend_for_tier(Tier::System, RiskGrade::Standard),
            Some(ExecutionBackend::FnCall)
        );
        assert_eq!(
            backend_for_tier(Tier::Official, RiskGrade::Standard),
            Some(ExecutionBackend::Container)
        );
        assert_eq!(
            backend_for_tier(Tier::Certified, RiskGrade::Standard),
            Some(ExecutionBackend::MicroVm)
        );
        assert_eq!(
            backend_for_tier(Tier::ThirdParty, RiskGrade::Standard),
            Some(ExecutionBackend::MicroVm)
        );
        assert_eq!(
            backend_for_tier(Tier::ThirdParty, RiskGrade::Elevated),
            Some(ExecutionBackend::FullVm)
        );
        // 黑名单不可调度——这是必须在旧实现上失败的安全断言（旧版无 Blacklist 分支）。
        assert_eq!(backend_for_tier(Tier::Blacklist, RiskGrade::Standard), None);
        assert_eq!(backend_for_tier(Tier::Blacklist, RiskGrade::Elevated), None);
    }

    #[test]
    fn fncall_ready_everywhere_but_has_no_os_isolation() {
        for p in [
            Platform::Linux,
            Platform::MacOs,
            Platform::Windows,
            Platform::Other,
        ] {
            let r = readiness(p, PlatformFeatures::default(), ExecutionBackend::FnCall);
            assert_eq!(r, Readiness::Ready, "{p:?}");
            assert!(r.can_run_now());
        }
        // 关键诚实点：FnCall 无 OS 隔离，不得用于不可信代码。
        assert_eq!(
            ExecutionBackend::FnCall.os_isolation(),
            OsIsolation::NoneInProcess
        );
        assert!(ExecutionBackend::FnCall.executor_wired());
    }

    #[test]
    fn container_and_vms_not_wired_even_when_primitives_present() {
        // 防回归：即使平台原语全绿，只要执行器未接线（v3.7.0），就不得报 Ready。
        let full = PlatformFeatures {
            container_runtime: true,
            dev_kvm: true,
            firecracker: true,
            hyper_v: true,
            apple_virtualization: true,
        };
        for b in [
            ExecutionBackend::Container,
            ExecutionBackend::MicroVm,
            ExecutionBackend::FullVm,
        ] {
            let r = readiness(Platform::Linux, full, b);
            assert!(
                matches!(r, Readiness::ExecutorNotWired { .. }),
                "{b:?} 执行器未接线时必须是 ExecutorNotWired，got {r:?}"
            );
            assert!(!r.can_run_now(), "{b:?} 未接线不得 can_run_now");
            assert!(!b.executor_wired());
        }
    }

    #[test]
    fn microvm_unsupported_off_linux_by_platform() {
        let f = PlatformFeatures::default();
        for p in [Platform::MacOs, Platform::Windows, Platform::Other] {
            assert!(
                matches!(
                    primitive_availability(p, f, ExecutionBackend::MicroVm),
                    PrimitiveAvailability::Unsupported { .. }
                ),
                "{p:?} 上 MicroVM 必须 Unsupported"
            );
        }
        // Linux 未探测到 kvm → NeedsProbe（fail-closed），不是 Available。
        assert!(matches!(
            primitive_availability(Platform::Linux, f, ExecutionBackend::MicroVm),
            PrimitiveAvailability::NeedsProbe { .. }
        ));
        // Linux 双原语齐备才 Available。
        let ok = PlatformFeatures {
            dev_kvm: true,
            firecracker: true,
            ..Default::default()
        };
        assert_eq!(
            primitive_availability(Platform::Linux, ok, ExecutionBackend::MicroVm),
            PrimitiveAvailability::Available
        );
    }

    #[test]
    fn fullvm_platform_specific_primitives() {
        let f = PlatformFeatures::default();
        assert!(matches!(
            primitive_availability(Platform::Linux, f, ExecutionBackend::FullVm),
            PrimitiveAvailability::NeedsProbe { .. }
        ));
        assert!(matches!(
            primitive_availability(Platform::Windows, f, ExecutionBackend::FullVm),
            PrimitiveAvailability::NeedsProbe { .. }
        ));
        assert!(matches!(
            primitive_availability(Platform::MacOs, f, ExecutionBackend::FullVm),
            PrimitiveAvailability::NeedsProbe { .. }
        ));
        assert!(matches!(
            primitive_availability(Platform::Other, f, ExecutionBackend::FullVm),
            PrimitiveAvailability::Unsupported { .. }
        ));
        assert_eq!(
            primitive_availability(
                Platform::Windows,
                PlatformFeatures {
                    hyper_v: true,
                    ..Default::default()
                },
                ExecutionBackend::FullVm
            ),
            PrimitiveAvailability::Available
        );
    }

    #[test]
    fn backend_str_roundtrip() {
        for b in ExecutionBackend::all() {
            assert_eq!(ExecutionBackend::parse(b.as_str()), Some(b));
        }
        assert_eq!(ExecutionBackend::parse("jail"), None);
    }

    #[test]
    fn current_platform_is_one_of_known() {
        // 编译期 cfg 必须落在已知平台集合内（CI 三平台覆盖）。
        let p = Platform::current();
        assert!(matches!(
            p,
            Platform::Linux | Platform::MacOs | Platform::Windows
        ));
    }

    #[test]
    fn detect_features_is_fail_closed_shape() {
        // 只验证返回结构合法、不 panic；真实取值取决于运行环境，不写死。
        let _ = detect_features();
    }
}
