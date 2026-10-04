//! 资源目录（Catalog）—— v3.8.0 基座骨架。
//!
//! 本版本只提供**只读、确定性**的资源形态/计量维度目录，供状态查询与后续版本复用。
//! 容量注册（节点声明可供给的算力/存储/带宽/Agent 能力 + 质押准入）在 v3.8.1 落地，
//! 届时这里会扩展为带配额与信誉初始分的可注册目录；本版本不持久化、不接受挂单写入。

use super::{MeterUnit, ResourceKind};

/// 每种资源形态默认支持的计量维度（纯静态目录，无副作用）。
///
/// 注意这只是**目录提示**，不强制成交单位；成交时仍以
/// [`crate::economy::resource::ResourceOffer`]/[`ResourceAsk`] 上的 `unit` 为准并做
/// 单位一致性校验。
pub fn default_units(kind: ResourceKind) -> &'static [MeterUnit] {
    match kind {
        ResourceKind::Compute => &[MeterUnit::CpuMillis, MeterUnit::GpuMillis],
        ResourceKind::Storage => &[MeterUnit::StorageGbSec],
        ResourceKind::Network => &[MeterUnit::NetworkBytes],
        ResourceKind::AgentCapability => &[
            MeterUnit::Invocation,
            MeterUnit::SnapshotRestore,
            MeterUnit::MemoryMbSec,
        ],
    }
}

/// 目录条目：资源形态 + 其默认计量维度。
pub fn catalog_entries() -> Vec<(ResourceKind, &'static [MeterUnit])> {
    ResourceKind::ALL
        .iter()
        .copied()
        .map(|k| (k, default_units(k)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_covers_every_kind_with_nonempty_units() {
        let entries = catalog_entries();
        assert_eq!(entries.len(), ResourceKind::ALL.len());
        for (kind, units) in entries {
            assert!(!units.is_empty(), "{kind:?} 目录至少需要一个计量维度");
        }
    }

    #[test]
    fn compute_defaults_include_cpu_and_gpu() {
        let units = default_units(ResourceKind::Compute);
        assert!(units.contains(&MeterUnit::CpuMillis));
        assert!(units.contains(&MeterUnit::GpuMillis));
    }
}
