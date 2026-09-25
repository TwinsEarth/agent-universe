//! 群体智能飞轮（v2.4.6）
//!
//! ① 跨模型协同 → 误差独立性决定集成上限
//! ② 简单个体 + 复杂网络 → 结构决定增长曲线形状
//! ③ 三层记忆共享 → 每次执行都产生经验，经验反馈优化结构
//! ④ 回到①：协同产生的新数据 → 更新记忆 → 优化结构 → 再协同

#[derive(Debug, Clone, Default)]
pub struct Flywheel {
    /// 参与协同的独立模型/节点数
    independent_models: u32,
    /// 累计协同轮次
    collaboration_rounds: u64,
    /// 累计写入三层记忆的经验数
    experiences_ingested: u64,
    /// 结构被优化（拓扑/路由被反馈调整）的次数
    structure_optimizations: u64,
}

impl Flywheel {
    pub fn new() -> Self {
        Self::default()
    }

    /// ① 登记一次跨模型协同；independent_models 必须 ≥3 才有集成红利。
    pub fn collaborate(&mut self, independent_models: u32) {
        self.independent_models = independent_models;
        self.collaboration_rounds += 1;
    }

    /// ③ 吞入经验（来自个体/群体/跨代记忆）。
    pub fn ingest(&mut self, n: u64) {
        self.experiences_ingested += n;
    }

    /// ④ 用经验反馈优化结构，闭合飞轮。
    pub fn optimize_structure(&mut self) {
        self.structure_optimizations += 1;
    }

    /// 飞轮是否自加速：独立模型≥3、协同有轮次、经验已反哺结构。
    pub fn is_spinning(&self) -> bool {
        self.independent_models >= 3
            && self.collaboration_rounds > 0
            && self.experiences_ingested > 0
            && self.structure_optimizations > 0
    }

    pub fn status(&self) -> (u32, u64, u64, u64) {
        (
            self.independent_models,
            self.collaboration_rounds,
            self.experiences_ingested,
            self.structure_optimizations,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flywheel_closes_loop() {
        let mut f = Flywheel::new();
        assert!(!f.is_spinning());
        f.collaborate(4);       // ① ≥3 独立模型
        f.ingest(100);          // ③ 经验入库
        f.optimize_structure(); // ④ 反哺结构
        assert!(f.is_spinning());
        let (k, rounds, exp, opt) = f.status();
        assert_eq!((k, rounds, exp, opt), (4, 1, 100, 1));
    }

    #[test]
    fn needs_independent_models() {
        let mut f = Flywheel::new();
        f.collaborate(1); // 单一模型无集成红利
        f.ingest(10);
        f.optimize_structure();
        assert!(!f.is_spinning());
    }
}
