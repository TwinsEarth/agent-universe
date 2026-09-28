//! 分层拓扑（Layered Topology）v2.4.1 / v2.6.5
//!
//! 从 P2P 点对点 / 星型结构，升级为 UDOS 多层分层结构。
//! 每一层把下层的一个扇区收敛为一个代表节点，向上再收敛，直到 Lv7 宇宙级。
//!
//! 设计目标（对应 P0b 实测证据一）：
//! - 任意节点在每一层的扇入（fan-in）≤ fanout（常数）；
//! - 总边数随 N 增长为 O(N · fanout)，而不是星型的 O(N) 中心负载或网状的 O(N²)；
//! - 跨节点消息路径长度随两房间在聚合树中的真实汇聚深度变化（GAP §5.2：不再恒为 7）。
//!
//! v2.6.5 修复（GAP §5.2/§5.3/§5.4）：
//! - 房间归属由节点在**有序集合中的排名**除以 fanout 决定——纯确定性，与加入顺序无关；
//!   用同一组 did、两种插入顺序构建，拓扑逐字节一致。
//! - `route_hops` 按两房间在聚合树中的最近公共祖先（lca）真实计算跳数，
//!   不再对所有跨房间对返回常量 7；未注册节点返回 None。
//! - `fanin_of` 计入 1 条上行到代表的边（此前漏算）。

use std::collections::BTreeSet;

/// 七层规模，从房间级到宇宙级。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// Lv1 房间级：几十节点（局域网 / 一台物理机内）
    Lv1Room,
    /// Lv2 楼栋 / 园区级
    Lv2Building,
    /// Lv3 城市级
    Lv3City,
    /// Lv4 省级
    Lv4Province,
    /// Lv5 国家级
    Lv5Country,
    /// Lv6 大洲级
    Lv6Continent,
    /// Lv7 宇宙 / 全球级（根种子层）
    Lv7Cosmos,
}

impl Level {
    pub const ALL: [Level; 7] = [
        Level::Lv1Room,
        Level::Lv2Building,
        Level::Lv3City,
        Level::Lv4Province,
        Level::Lv5Country,
        Level::Lv6Continent,
        Level::Lv7Cosmos,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Level::Lv1Room => "Lv1 房间级",
            Level::Lv2Building => "Lv2 楼栋级",
            Level::Lv3City => "Lv3 城市级",
            Level::Lv4Province => "Lv4 省级",
            Level::Lv5Country => "Lv5 国家级",
            Level::Lv6Continent => "Lv6 大洲级",
            Level::Lv7Cosmos => "Lv7 宇宙级",
        }
    }

    pub fn depth() -> usize {
        7
    }
}

/// 分层拓扑。节点按字典序排入一个有序集合，每 fanout 个切为一个 Lv1 房间；
/// 房间再按 fanout 个聚合为 Lv2 组，逐层向上直到 Lv7 根。
#[derive(Debug, Clone, Default)]
pub struct LayeredTopology {
    /// 每层的扇入上限（同一父节点下挂的子节点数）。
    fanout: u32,
    /// 全部注册 did 的有序集合（BTreeSet 迭代即字典序，确定性）。
    members: BTreeSet<String>,
}

impl LayeredTopology {
    /// fanout 为每层扇入上限（建议 8~16；UDOS P0b 实测取 9）。
    pub fn new(fanout: u32) -> Self {
        Self {
            fanout: fanout.max(2),
            members: BTreeSet::new(),
        }
    }

    pub fn fanout(&self) -> u32 {
        self.fanout
    }

    /// 把一个节点加入拓扑。幂等；归属由有序排名决定，与加入顺序无关。
    pub fn join(&mut self, did: String) {
        self.members.insert(did);
    }

    pub fn node_count(&self) -> usize {
        self.members.len()
    }

    /// did 在有序成员序列中的 0-based 排名；未注册返回 None。
    fn rank_of(&self, did: &str) -> Option<usize> {
        self.members.iter().position(|m| m == did)
    }

    /// 由排名推 Lv1 房间号。
    fn room_of_rank(&self, rank: usize) -> u64 {
        (rank / self.fanout as usize) as u64
    }

    /// Lv1 房间数。
    pub fn room_count(&self) -> usize {
        match self.members.len() {
            0 => 0,
            n => self.room_of_rank(n - 1) as usize + 1,
        }
    }

    /// 该节点的扇入：同房间邻居数 + 1 条到本房间代表的上行边（GAP §5.4 此前漏算上行）。
    pub fn fanin_of(&self, did: &str) -> usize {
        let rank = match self.rank_of(did) {
            Some(r) => r,
            None => return 0,
        };
        let room = self.room_of_rank(rank) as usize;
        let start = room * self.fanout as usize;
        let end = (start + self.fanout as usize).min(self.members.len());
        let room_size = end - start;
        // 邻居 + 1 条上行边；代表本身不额外自环，饱和到 fanout。
        room_size.saturating_sub(1) + 1
    }

    /// 总边数：每个房间内全连接 n(n-1)/2，外加每节点 1 条上行边。O(1) 按公式计。
    pub fn logical_edges(&self) -> u64 {
        let n = self.members.len() as u64;
        let f = self.fanout as u64;
        if n == 0 {
            return 0;
        }
        let full_rooms = n / f;
        let rem = n % f;
        let mut edges = full_rooms * f * (f - 1) / 2;
        if rem > 1 {
            edges += rem * (rem - 1) / 2;
        }
        edges += n; // 每节点一条上行边
        edges
    }

    /// 跨节点路由跳数（GAP §5.2）：
    /// - 未注册节点 → None；
    /// - 自己 → 0；同房间邻居 → 1；
    /// - 跨房间 → 按两房间在聚合树中的最近公共祖先层 lca 计算，
    ///   上行 (lca-1) 跳 + 下行 (lca-1) 跳 = 2·(lca-1)。
    ///   房间聚合：Lv2 = room/fanout，Lv3 = room/fanout² ……逐层取整直到两桶相等。
    pub fn route_hops(&self, from: &str, to: &str) -> Option<usize> {
        let rf = self.rank_of(from)?;
        let rt = self.rank_of(to)?;
        if rf == rt {
            return Some(0);
        }
        let r1 = self.room_of_rank(rf);
        let r2 = self.room_of_rank(rt);
        if r1 == r2 {
            return Some(1);
        }
        let f = self.fanout as u64;
        let mut b1 = r1;
        let mut b2 = r2;
        let mut lca: u32 = 1;
        loop {
            b1 /= f;
            b2 /= f;
            lca += 1;
            if b1 == b2 || lca >= Level::depth() as u32 {
                break;
            }
        }
        Some(2 * (lca - 1) as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(fanout: u32, n: usize) -> LayeredTopology {
        let mut topo = LayeredTopology::new(fanout);
        for i in 0..n {
            topo.join(format!("did:aip:{i:06x}"));
        }
        topo
    }

    #[test]
    fn levels_have_labels() {
        assert_eq!(Level::ALL.len(), 7);
        assert_eq!(Level::Lv7Cosmos.label(), "Lv7 宇宙级");
    }

    #[test]
    fn join_groups_by_fanout() {
        let topo = build(9, 20);
        assert_eq!(topo.node_count(), 20);
        // 9 个/房：20 节点应占 ceil(20/9)=3 个房
        assert_eq!(topo.room_count(), 3);
    }

    #[test]
    fn fanin_is_bounded_by_fanout() {
        let topo = build(9, 100);
        // 扇入 = 同房间邻居(fanout-1) + 1 上行 = fanout
        for i in 0..100usize {
            assert!(topo.fanin_of(&format!("did:aip:{i:06x}")) <= 9);
        }
    }

    #[test]
    fn edges_scale_linearly_not_quadratically() {
        let e10k = build(9, 10_000).logical_edges();
        let e20k = build(9, 20_000).logical_edges();
        let n1 = 10_000u64;
        // 亚二次：远小于 N^2
        assert!(e10k < n1 * n1 / 100, "edges={} should be sub-quadratic", e10k);
        // 规模对照（GAP §5.3）：N 翻倍，edges 应约翻倍（线性），而非 4 倍（二次）。
        let ratio = e20k as f64 / e10k as f64;
        assert!(
            ratio < 2.5,
            "edges should grow ~linearly with N, got e20k/e10k={:.2}",
            ratio
        );
    }

    #[test]
    fn topology_is_insertion_order_independent() {
        // GAP §5.4：同一组 did、两种插入顺序，拓扑必须一致。
        let mut a = LayeredTopology::new(9);
        for i in 0..500usize {
            a.join(format!("did:aip:{i:06x}"));
        }
        let mut b = LayeredTopology::new(9);
        for i in (0..500usize).rev() {
            b.join(format!("did:aip:{i:06x}"));
        }
        assert_eq!(a.room_count(), b.room_count());
        assert_eq!(a.logical_edges(), b.logical_edges());
        for i in 0..500usize {
            assert_eq!(
                a.route_hops("did:aip:000000", &format!("did:aip:{i:06x}")),
                b.route_hops("did:aip:000000", &format!("did:aip:{i:06x}"))
            );
        }
    }

    #[test]
    fn route_hows_reflect_real_lca_not_constant_seven() {
        // fanout=9，500 节点 → ceil(500/9)=56 房间。
        let topo = build(9, 500);
        // 同房间邻居 1 跳
        assert_eq!(topo.route_hops("did:aip:000000", "did:aip:000001"), Some(1));
        // 同一 Lv2 组的两个相邻房间（room0 与 room1，bucket=0）→ lca=2 → 2 跳
        let room1_first = "did:aip:000009"; // rank 9 → room 1
        assert_eq!(topo.route_hops("did:aip:000000", room1_first), Some(2));
        // 跨 Lv2 组：room0(bucket0) 与 room10(bucket=10/9=1) → lca=3 → 4 跳
        let room10_first = "did:aip:00005a"; // rank 90 → room 10
        let far = topo.route_hops("did:aip:000000", room10_first).unwrap();
        assert_eq!(far, 4);
        // 关键：远房间跳数 > 近房间跳数，证明不是常量 7
        assert!(far > 2);
        // 未注册节点 → None
        assert_eq!(topo.route_hops("did:aip:deadbeef", "did:aip:000001"), None);
    }
}
