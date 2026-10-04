//! v3.7.6：Linux-MicroVM 专有内存原语的**诚实门**（virtio-pmem / DAX / DAMON / balloon）。
//!
//! 背景见 `docs/ausec/AUSEC-DESIGN.md` §3.1/§5：外部报道（**aiwiki.ai /
//! byteiota.com 对 DSEC 的实测，非本仓复测**）称 virtio-pmem+DAX 让同机 MicroVM
//! 共享宿主页缓存、峰值内存降 40.2%，DAMON+balloon 识别并回收空闲页使按时间累计
//! 内存再降 21.2%。这四项都是 **Linux KVM/Firecracker guest** 才有的内核/虚拟化
//! 原语，macOS / Windows 没有等价物。
//!
//! 本文件严格只做两件诚实的事，**不调用任何 syscall、不碰真实页**：
//!
//! 1. 在 **Linux** 平台把这些原语**识别/声明**为"平台理论上存在的 Linux-MicroVM
//!    原语"，但状态是 [`MemoryPrimitiveState::DeclaredLinux`]——v3.7.6 尚未接线
//!    真实执行器，[`MemoryPrimitiveStatus::can_enforce`] 恒为 `false`，绝不声称
//!    已经共享/回收了任何物理页。
//! 2. 在 **macOS / Windows / Other** 平台一律返回
//!    [`MemoryPrimitiveState::Unsupported`] 并附**具名理由**（fail-closed 的具名
//!    拒绝），不静默、不降级、不把"记账策略生效"谎报成"OS 原语已生效"。
//!
//! 这与 [`crate::ausec::backend`] 的 `PrimitiveAvailability × executor_wired`
//! 双层诚实模型保持一致：平台有原语 ≠ 本仓已强制执行。

use super::backend::Platform;
use serde::{Deserialize, Serialize};

/// Linux-MicroVM 专有的内存共享/回收原语。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryPrimitive {
    /// virtio-pmem：把宿主侧持久内存/页缓存以 pmem 设备暴露给 guest。
    VirtioPmem,
    /// DAX：绕过 guest 页缓存的直接访问，配合 virtio-pmem 让多 MicroVM 共享宿主页缓存。
    Dax,
    /// DAMON：数据访问监控内核机制，用于识别冷/热页，驱动空闲页回收。
    Damon,
    /// virtio-balloon：guest 向宿主报告/归还空闲页的膨胀/收缩设备。
    Balloon,
}

impl MemoryPrimitive {
    /// 规范标识（PMB / 序列化用）。
    pub fn as_str(self) -> &'static str {
        match self {
            MemoryPrimitive::VirtioPmem => "virtio_pmem",
            MemoryPrimitive::Dax => "dax",
            MemoryPrimitive::Damon => "damon",
            MemoryPrimitive::Balloon => "balloon",
        }
    }

    /// 从字符串解析；同时接受 `virtio-pmem` 连写别名。无法识别返回 `None`
    /// （上层据此 fail-closed，绝不忽略未知原语名）。
    pub fn parse(s: &str) -> Option<MemoryPrimitive> {
        Some(match s {
            "virtio_pmem" | "virtio-pmem" => MemoryPrimitive::VirtioPmem,
            "dax" => MemoryPrimitive::Dax,
            "damon" => MemoryPrimitive::Damon,
            "balloon" => MemoryPrimitive::Balloon,
            _ => return None,
        })
    }

    pub fn all() -> [MemoryPrimitive; 4] {
        [
            MemoryPrimitive::VirtioPmem,
            MemoryPrimitive::Dax,
            MemoryPrimitive::Damon,
            MemoryPrimitive::Balloon,
        ]
    }

    /// 该原语做什么（诚实描述；外部降幅仅以"外部报道、非本仓复测"口径引用）。
    pub fn detail(self) -> &'static str {
        match self {
            MemoryPrimitive::VirtioPmem => {
                "virtio-pmem 把宿主侧（含只读共享内容的）页缓存以 pmem 设备映射进 Linux MicroVM guest；外部 DSEC 报道称配合 DAX 使峰值内存降约 40.2%，非本仓复测"
            }
            MemoryPrimitive::Dax => {
                "DAX 直接访问绕过 guest 页缓存，配合 virtio-pmem 让同机多 MicroVM 共享宿主页缓存；Linux KVM/Firecracker 专有，非本仓复测"
            }
            MemoryPrimitive::Damon => {
                "DAMON 数据访问监控用于识别冷页；外部 DSEC 报道称 DAMON+balloon 使按时间累计内存再降约 21.2%，非本仓复测"
            }
            MemoryPrimitive::Balloon => {
                "virtio-balloon 让 Linux guest 报告/归还空闲页（空闲页报告回收）；需要 guest 内核 virtio-balloon 驱动支持，非本仓复测"
            }
        }
    }
}

/// 原语门状态（只区分"平台声明存在"与"平台无原语"；二者在 v3.7.6 都不可强制执行）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryPrimitiveState {
    /// Linux：原语在平台层面**存在/被声明**，但本仓执行器未接线，`can_enforce=false`。
    DeclaredLinux,
    /// 非 Linux：平台无对应原语，具名拒绝。
    Unsupported,
}

/// 一次原语门评估结果（只读、确定性，可注入平台用于跨平台测试）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MemoryPrimitiveStatus {
    pub primitive: &'static str,
    pub platform: Platform,
    pub state: MemoryPrimitiveState,
    /// 当前是否由本仓**真实强制执行**该原语。v3.7.6 恒为 `false`（仅 Linux 声明、
    /// 执行器未接线；非 Linux 无原语）。这是防止"承诺>事实"的硬事实，测试锁定。
    pub can_enforce: bool,
    /// 人类可读的具名说明 / 拒绝理由。
    pub detail: String,
}

/// 评估某内存原语在指定平台上的诚实状态（纯函数）。
///
/// - Linux → [`MemoryPrimitiveState::DeclaredLinux`]：识别/声明，但 `can_enforce=false`
///   （v3.7.6 未接线真实 virtio/内核操作，不生效）。
/// - macOS / Windows / Other → [`MemoryPrimitiveState::Unsupported`]：附具名理由。
pub fn memory_primitive_status(platform: Platform, prim: MemoryPrimitive) -> MemoryPrimitiveStatus {
    match platform {
        Platform::Linux => MemoryPrimitiveStatus {
            primitive: prim.as_str(),
            platform,
            state: MemoryPrimitiveState::DeclaredLinux,
            can_enforce: false,
            detail: format!(
                "{}；该原语为 Linux-MicroVM 专有，v3.7.6 仅在平台层面声明/识别，真实执行器未接线，当前不会真实共享或回收任何物理页",
                prim.detail()
            ),
        },
        Platform::MacOs | Platform::Windows | Platform::Other => MemoryPrimitiveStatus {
            primitive: prim.as_str(),
            platform,
            state: MemoryPrimitiveState::Unsupported,
            can_enforce: false,
            detail: format!(
                "{} 依赖 Linux KVM/Firecracker guest 的 virtio/内核能力（virtio-pmem/DAX/DAMON/virtio-balloon）；{platform:?} 平台无对应原语，具名拒绝，不声称已共享或回收内存",
                prim.as_str()
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_roundtrip_and_alias() {
        for p in MemoryPrimitive::all() {
            assert_eq!(MemoryPrimitive::parse(p.as_str()), Some(p));
        }
        assert_eq!(
            MemoryPrimitive::parse("virtio-pmem"),
            Some(MemoryPrimitive::VirtioPmem)
        );
        assert_eq!(MemoryPrimitive::parse("ksm"), None);
        assert_eq!(MemoryPrimitive::parse(""), None);
    }

    #[test]
    fn linux_declares_but_never_enforces_in_376() {
        for p in MemoryPrimitive::all() {
            let s = memory_primitive_status(Platform::Linux, p);
            assert_eq!(s.state, MemoryPrimitiveState::DeclaredLinux);
            // 关键诚实点：即使在 Linux，v3.7.6 也只声明、未接线，can_enforce 必须 false。
            assert!(!s.can_enforce, "{p:?} 在 3.7.6 不得声称可强制执行");
            assert!(s.detail.contains("执行器未接线"));
        }
    }

    #[test]
    fn non_linux_platforms_named_reject() {
        for plat in [Platform::MacOs, Platform::Windows, Platform::Other] {
            for p in MemoryPrimitive::all() {
                let s = memory_primitive_status(plat, p);
                assert_eq!(s.state, MemoryPrimitiveState::Unsupported, "{plat:?} {p:?}");
                assert!(!s.can_enforce);
                assert!(
                    s.detail.contains("具名拒绝"),
                    "必须给出具名理由: {}",
                    s.detail
                );
                assert!(s.detail.contains("无对应原语"));
            }
        }
    }

    #[test]
    fn no_primitive_is_enforceable_on_any_platform_in_376() {
        // 回归锁：四原语在全部平台的 can_enforce 都必须为 false——
        // 一旦未来某原语真的接线，应在对应补丁里显式更新本断言并附带真实执行证据。
        for plat in [
            Platform::Linux,
            Platform::MacOs,
            Platform::Windows,
            Platform::Other,
        ] {
            for p in MemoryPrimitive::all() {
                assert!(!memory_primitive_status(plat, p).can_enforce);
            }
        }
    }
}
