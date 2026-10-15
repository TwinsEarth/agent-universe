//! v5.1.3：lineage 血缘（内容哈希键的父/祖先链，v5-memory-roadmap §二）。
//!
//! 对齐 Mesh Memory Protocol 的 **inter-agent lineage** 原语：每条派生记忆都以
//! 内容哈希（`claim_hash`）为键，记录其直接父节点，并可沿链求完整祖先，使任何
//! 声明都能**追溯来源**。
//!
//! 设计取舍：
//! - 只存 `child → parent` 指针，**祖先链在查询时动态沿链计算**，不缓存祖先列表，
//!   从而避免“父节点后补祖父时，子节点缓存的祖先链变陈旧”。
//! - `link` 前置防环（父链已含子则成环）；`ancestors` 再用访问集合双保险。
//! - 篡改块（`Cat7Block::verify()==false`）不得进入血缘（fail-closed）。
//! - 持久化为 `lineage.jsonl`（每行一条边），父节点可被更正（覆盖）后全量重写。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::memory::cat7::Cat7Block;

/// 血缘错误（fail-closed：非法关系即拒绝，不写盘）。
#[derive(Debug)]
pub enum LineageError {
    /// 节点以自身为父。
    SelfLink,
    /// 建立该关系会形成环（携带涉及的节点）。
    Cycle(String),
    /// CAT7 块未通过完整性校验（篡改块不能进入血缘）。
    BlockNotVerified(String),
    /// I/O 失败。
    Io(String),
    /// JSON 行解析失败。
    Json(String),
}

impl fmt::Display for LineageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SelfLink => write!(f, "节点不能以自身为父节点（SelfLink）"),
            Self::Cycle(h) => write!(f, "检测到血缘环（涉及 {h}），拒绝建立该关系"),
            Self::BlockNotVerified(h) => {
                write!(f, "CAT7 块未通过完整性校验（篡改块不能进入血缘）: {h}")
            }
            Self::Io(e) => write!(f, "I/O 错误: {e}"),
            Self::Json(e) => write!(f, "JSON 解析错误: {e}"),
        }
    }
}

impl std::error::Error for LineageError {}

impl From<std::io::Error> for LineageError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

/// 一条血缘边（持久化行）：`child` 派生自 `parent`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageRecord {
    pub child: String,
    pub parent: String,
}

/// 血缘图：`claim_hash → 直接父节点 claim_hash`；祖先链动态沿链计算。
#[derive(Debug, Clone, Default)]
pub struct LineageGraph {
    parent_of: HashMap<String, String>,
    path: Option<PathBuf>,
}

impl LineageGraph {
    /// 空血缘图（不落盘）。
    pub fn new() -> Self {
        Self::default()
    }

    /// 打开（或创建）血缘库：回读 `lineage.jsonl` 并重放每条边（重放时同样校验无环）。
    pub fn open(path: &Path) -> Result<Self, LineageError> {
        let mut g = Self::new();
        g.path = Some(path.to_path_buf());
        if path.exists() {
            let text = fs::read_to_string(path)?;
            for line in text.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                let rec: LineageRecord =
                    serde_json::from_str(line).map_err(|e| LineageError::Json(e.to_string()))?;
                // 重放走内存 link（不再次落盘），损坏/成环数据在此暴露。
                g.link(&rec.child, &rec.parent)?;
            }
        }
        Ok(g)
    }

    /// 登记一个（无父的）独立节点；已存在则无操作。
    pub fn register(&mut self, hash: &str) {
        self.parent_of.entry(hash.to_string()).or_insert_with(|| {
            // 占位：独立节点不写入 parent_of（无父），
            // 独立节点无需存储，查询时 root 即自身；这里仅保留入口语义。
            String::new()
        });
        // 若占位为空串则移除（独立节点不占用表）。
        if self
            .parent_of
            .get(hash)
            .map(|s| s.is_empty())
            .unwrap_or(false)
        {
            self.parent_of.remove(hash);
        }
    }

    /// 建立血缘：`child` 派生自 `parent`。
    ///
    /// - `child == parent` → [`LineageError::SelfLink`]；
    /// - 若 `parent` 的祖先链已含 `child`（建立后会成环）→ [`LineageError::Cycle`]；
    /// - 若 `child` 已有父，允许更正（覆盖），覆盖后重新校验无环。
    pub fn link(&mut self, child: &str, parent: &str) -> Result<(), LineageError> {
        if child == parent {
            return Err(LineageError::SelfLink);
        }
        // 不先插入：求 parent 的上行链（parent 本身 + 其祖先），若已含 child 则成环。
        let mut up = vec![parent.to_string()];
        up.extend(self.ancestors(parent)?);
        if up.iter().any(|h| h == child) {
            return Err(LineageError::Cycle(child.to_string()));
        }
        self.parent_of.insert(child.to_string(), parent.to_string());
        Ok(())
    }

    /// 以两个 CAT7 块建立血缘：前置完整性校验（篡改块拒绝），再按 claim_hash 链接。
    pub fn link_blocks(
        &mut self,
        child: &Cat7Block,
        parent: &Cat7Block,
    ) -> Result<(), LineageError> {
        if !child.verify() {
            return Err(LineageError::BlockNotVerified(child.claim_hash.clone()));
        }
        if !parent.verify() {
            return Err(LineageError::BlockNotVerified(parent.claim_hash.clone()));
        }
        self.link(&child.claim_hash, &parent.claim_hash)
    }

    /// 直接父节点（无父 → None）。
    pub fn parent_of_node(&self, hash: &str) -> Option<&str> {
        self.parent_of.get(hash).map(|s| s.as_str())
    }

    /// 祖先链（**最近在前**）：`[parent, grandparent, …, root]`；不含自身。
    /// 访问集合在链路意外成环时返回 [`LineageError::Cycle`]（link 已防环，此为双保险）。
    pub fn ancestors(&self, hash: &str) -> Result<Vec<String>, LineageError> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        seen.insert(hash.to_string());
        let mut cur = hash.to_string();
        while let Some(p) = self.parent_of.get(&cur) {
            if !seen.insert(p.clone()) {
                return Err(LineageError::Cycle(p.clone()));
            }
            out.push(p.clone());
            cur = p.clone();
        }
        Ok(out)
    }

    /// 根节点：无父时为自身，否则为祖先链末端。
    pub fn root(&self, hash: &str) -> Result<String, LineageError> {
        Ok(self
            .ancestors(hash)?
            .last()
            .cloned()
            .unwrap_or_else(|| hash.to_string()))
    }

    /// `anc` 是否为 `hash` 的祖先（沿父链可走到）。
    pub fn is_ancestor(&self, anc: &str, hash: &str) -> Result<bool, LineageError> {
        Ok(self.ancestors(hash)?.iter().any(|h| h == anc))
    }

    /// 深度 = 祖先链长度（根=0，其直接子代=1，以此类推）。
    pub fn depth(&self, hash: &str) -> Result<usize, LineageError> {
        Ok(self.ancestors(hash)?.len())
    }

    /// 直接子节点（反向边）：以 `parent` 为父的全部节点（无序）。
    pub fn children_of(&self, parent: &str) -> Vec<String> {
        self.parent_of
            .iter()
            .filter(|(_, p)| p.as_str() == parent)
            .map(|(c, _)| c.clone())
            .collect()
    }

    /// 全部血缘边（持久化用；顺序不保证）。
    pub fn records(&self) -> Vec<LineageRecord> {
        self.parent_of
            .iter()
            .map(|(child, parent)| LineageRecord {
                child: child.clone(),
                parent: parent.clone(),
            })
            .collect()
    }

    /// 全量保存到打开时绑定的文件（边为权威镜像，全量重写）。
    pub fn save(&self) -> Result<(), LineageError> {
        match &self.path {
            Some(p) => self.save_to(p),
            None => Err(LineageError::Io(
                "未绑定文件路径（使用 save_to）".to_string(),
            )),
        }
    }

    /// 全量保存到指定路径：每行一条边。
    pub fn save_to(&self, path: &Path) -> Result<(), LineageError> {
        if let Some(dir) = path.parent() {
            if !dir.as_os_str().is_empty() {
                fs::create_dir_all(dir)?;
            }
        }
        let mut text = String::new();
        for rec in self.records() {
            let line =
                serde_json::to_string(&rec).map_err(|e| LineageError::Json(e.to_string()))?;
            text.push_str(&line);
            text.push('\n');
        }
        fs::write(path, text)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::fleet::{FleetAccess, FleetEntry, FleetKind};
    use std::fs;

    fn tmp_path(name: &str) -> PathBuf {
        let p =
            std::env::temp_dir().join(format!("au-lineage-{name}-{}.jsonl", std::process::id()));
        let _ = fs::remove_file(&p);
        p
    }

    fn block(author: &str, content: &str) -> Cat7Block {
        let e = FleetEntry::new(FleetKind::Fact, author, FleetAccess::Contributor, content);
        Cat7Block::from_fleet_entry(&e, None)
    }

    /// 基本链：parent/ancestors/root/depth，祖先最近在前。
    #[test]
    fn chain_ancestors_root_depth() {
        let mut g = LineageGraph::new();
        g.link("c", "b").unwrap();
        g.link("b", "a").unwrap();

        assert_eq!(g.parent_of_node("c"), Some("b"));
        assert_eq!(g.ancestors("c").unwrap(), vec!["b", "a"]);
        assert_eq!(g.root("c").unwrap(), "a");
        assert_eq!(g.depth("c").unwrap(), 2);
        assert_eq!(g.depth("a").unwrap(), 0);
        // 独立节点：root=自身，祖先为空。
        assert_eq!(g.root("a").unwrap(), "a");
        assert!(g.ancestors("a").unwrap().is_empty());
    }

    /// 自链被拒绝。
    #[test]
    fn self_link_rejected() {
        let mut g = LineageGraph::new();
        assert!(matches!(g.link("x", "x"), Err(LineageError::SelfLink)));
    }

    /// 成环被拒绝：已有 a→b→c（c 是 a 的祖先），再把 c 链接到 a 必须失败。
    #[test]
    fn cycle_rejected() {
        let mut g = LineageGraph::new();
        // a 派生自 b，b 派生自 c（a 的祖先含 c）。
        g.link("a", "b").unwrap();
        g.link("b", "c").unwrap();
        // 再让 c 派生自 a → 闭环。
        let err = g.link("c", "a").unwrap_err();
        assert!(matches!(err, LineageError::Cycle(_)));
        // 拒绝后 c 仍无父（未被写入）。
        assert_eq!(g.parent_of_node("c"), None);
    }

    /// is_ancestor 判定。
    #[test]
    fn ancestor_predicate() {
        let mut g = LineageGraph::new();
        g.link("c", "b").unwrap();
        g.link("b", "a").unwrap();
        assert!(g.is_ancestor("a", "c").unwrap());
        assert!(g.is_ancestor("b", "c").unwrap());
        assert!(!g.is_ancestor("c", "a").unwrap());
        assert!(!g.is_ancestor("c", "c").unwrap());
    }

    /// 分叉：两个子节点共享同一父（菱形的上半），各自祖先正确。
    #[test]
    fn fork_shares_parent() {
        let mut g = LineageGraph::new();
        g.link("b1", "a").unwrap();
        g.link("b2", "a").unwrap();
        assert_eq!(g.ancestors("b1").unwrap(), vec!["a"]);
        assert_eq!(g.ancestors("b2").unwrap(), vec!["a"]);
        let mut kids = g.children_of("a");
        kids.sort();
        assert_eq!(kids, vec!["b1", "b2"]);
    }

    /// 更正父节点（覆盖）后祖先链重新计算。
    #[test]
    fn relink_overrides_parent() {
        let mut g = LineageGraph::new();
        g.link("c", "b").unwrap();
        // 更正：c 实际派生自 d（d 派生自 a）。
        g.link("d", "a").unwrap();
        g.link("c", "d").unwrap();
        assert_eq!(g.parent_of_node("c"), Some("d"));
        assert_eq!(g.ancestors("c").unwrap(), vec!["d", "a"]);
    }

    /// CAT7 块链接：正常块可建立，篡改块被拒绝（fail-closed）。
    #[test]
    fn link_blocks_verifies() {
        let mut g = LineageGraph::new();
        let parent = block("alice", "原始结论");
        let child = block("bob", "修订结论");
        g.link_blocks(&child, &parent).unwrap();
        assert_eq!(
            g.parent_of_node(&child.claim_hash),
            Some(parent.claim_hash.as_str())
        );

        // 篡改 child：verify 失败，不能进入血缘。
        let mut tampered = block("carol", "另一个结论");
        tampered.content = "被篡改的内容".to_string();
        let err = g.link_blocks(&tampered, &parent).unwrap_err();
        assert!(matches!(err, LineageError::BlockNotVerified(_)));
        assert_eq!(g.parent_of_node(&tampered.claim_hash), None);
    }

    /// 同内容块（同 claim_hash）链接到自身 → SelfLink。
    #[test]
    fn identical_claim_is_self_link() {
        let mut g = LineageGraph::new();
        let b = block("alice", "相同内容");
        let b2 = block("alice", "相同内容");
        assert_eq!(b.claim_hash, b2.claim_hash);
        assert!(matches!(
            g.link_blocks(&b2, &b),
            Err(LineageError::SelfLink)
        ));
    }

    /// JSONL 保存 → 重开：边完整重放，祖先链一致。
    #[test]
    fn jsonl_save_open_roundtrip() {
        let p = tmp_path("roundtrip");
        let mut g = LineageGraph::open(&p).unwrap();
        g.link("c", "b").unwrap();
        g.link("b", "a").unwrap();
        g.save().unwrap();
        assert!(p.exists());

        let reopened = LineageGraph::open(&p).unwrap();
        assert_eq!(reopened.ancestors("c").unwrap(), vec!["b", "a"]);
        assert_eq!(reopened.root("c").unwrap(), "a");
        assert_eq!(reopened.depth("c").unwrap(), 2);
    }

    /// 重放含环的损坏文件必须报错（不静默接受）。
    #[test]
    fn open_rejects_cyclic_file() {
        let p = tmp_path("cyclic-file");
        // 手工写入会成环的边：a→b 与 b→a。
        fs::write(
            &p,
            "{\"child\":\"a\",\"parent\":\"b\"}\n{\"child\":\"b\",\"parent\":\"a\"}\n",
        )
        .unwrap();
        let err = LineageGraph::open(&p).unwrap_err();
        assert!(matches!(err, LineageError::Cycle(_)));
    }

    /// 登记独立节点不产生父边，root 即自身。
    #[test]
    fn register_standalone() {
        let mut g = LineageGraph::new();
        g.register("solo");
        assert_eq!(g.parent_of_node("solo"), None);
        assert_eq!(g.root("solo").unwrap(), "solo");
    }
}
