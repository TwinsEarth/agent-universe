//! v5.1.1：CAT7 七字段认知记忆块（v5-memory-roadmap §二）。
//!
//! 跨 Agent 记忆交换的标准单元（对齐 Mesh Memory Protocol 的 CAT7 原语）：
//! `claim_hash / kind / author / content / source / confidence / created_at` 七字段。
//! 本文件只定义交换格式与转换/校验（纯），不触碰存储；FleetMemory 的
//! `export_cat7` / `import_cat7`（fleet.rs）负责与 `fleet-memory.jsonl` 的往返。

use serde::{Deserialize, Serialize};

use crate::memory::fleet::{fleet_claim_hash, FleetEntry, FleetKind};

/// 七字段认知记忆块（跨 Agent 交换单元）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cat7Block {
    /// SHA-256(kind|author|content) 内容锚点（跨 Agent 唯一标识，同内容必同 hash）。
    pub claim_hash: String,
    /// 策展分类：fact / decision / open_problem。
    pub kind: FleetKind,
    /// 来源 Agent id。
    pub author: String,
    /// 记忆内容本体。
    pub content: String,
    /// 血缘前驱：上一块的 claim_hash（v5.1.3 lineage 的前置字段；无则 None）。
    pub source: Option<String>,
    /// 接收方角色评估置信度（0.0..=1.0）；`None` = 未评估（诚实，不编造默认值）。
    pub confidence: Option<f32>,
    /// 创建时间（ISO8601，UTC 秒精度）。
    pub created_at: String,
}

impl Cat7Block {
    /// 从集体记忆条目构造交换块。
    /// `source` 传入前驱 claim_hash（无则 None）；`confidence` 恒为 `None`
    /// （本版本不做接收方评估，v5.1.2 SVAF 才引入字段级评估）。
    pub fn from_fleet_entry(entry: &FleetEntry, source: Option<&str>) -> Self {
        Self {
            claim_hash: entry.claim_hash.clone(),
            kind: entry.kind,
            author: entry.author.clone(),
            content: entry.content.clone(),
            source: source.map(|s| s.to_string()),
            confidence: None,
            created_at: entry.ts.clone(),
        }
    }

    /// 完整性校验：`claim_hash` 必须等于重算值（SHA-256(kind|author|content)），
    /// 否则视为被篡改/损坏，调用方应 fail-closed 拒绝。
    pub fn verify(&self) -> bool {
        self.claim_hash == fleet_claim_hash(self.kind, &self.author, &self.content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::fleet::{FleetAccess, FleetEntry, FleetKind};

    fn entry(kind: FleetKind, author: &str, content: &str) -> FleetEntry {
        FleetEntry::new(kind, author, FleetAccess::Contributor, content)
    }

    /// 七字段完整 + verify 通过 + 往返一致。
    #[test]
    fn cat7_roundtrip_and_verify() {
        let e = entry(FleetKind::Fact, "alice", "地球是圆的");
        let b = Cat7Block::from_fleet_entry(&e, Some("prev-hash"));
        assert_eq!(
            b.claim_hash,
            fleet_claim_hash(FleetKind::Fact, "alice", "地球是圆的")
        );
        assert_eq!(b.kind, FleetKind::Fact);
        assert_eq!(b.author, "alice");
        assert_eq!(b.content, "地球是圆的");
        assert_eq!(b.source.as_deref(), Some("prev-hash"));
        assert_eq!(b.created_at, e.ts);
        assert!(b.verify(), "未篡改块必须通过完整性校验");

        let json = serde_json::to_string(&b).unwrap();
        let back: Cat7Block = serde_json::from_str(&json).unwrap();
        assert_eq!(back.claim_hash, b.claim_hash);
        assert!(back.verify());
    }

    /// 无前驱时 source=None；confidence 恒为 None（未评估，诚实不编造）。
    #[test]
    fn cat7_none_source_and_honest_confidence() {
        let e = entry(FleetKind::Decision, "bob", "采用方案 B");
        let b = Cat7Block::from_fleet_entry(&e, None);
        assert_eq!(b.source, None);
        assert_eq!(
            b.confidence, None,
            "v5.1.1 不做接收方评估，confidence 必须为 None"
        );
    }

    /// 篡改检测：改内容后 verify 必须失败（fail-closed 依据）。
    #[test]
    fn cat7_tamper_detected() {
        let e = entry(FleetKind::OpenProblem, "carol", "如何扩容");
        let mut b = Cat7Block::from_fleet_entry(&e, None);
        b.content = "如何扩容（已被篡改）".to_string();
        assert!(!b.verify(), "篡改内容后 claim_hash 校验必须失败");
    }

    /// 同内容同 hash（跨 Agent 去重与血缘锚定的确定性基础）。
    #[test]
    fn cat7_hash_deterministic() {
        let a = Cat7Block::from_fleet_entry(&entry(FleetKind::Fact, "dave", "42"), None);
        let b2 = Cat7Block::from_fleet_entry(&entry(FleetKind::Fact, "dave", "42"), None);
        assert_eq!(a.claim_hash, b2.claim_hash);
    }
}
