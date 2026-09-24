//! 群体记忆（Swarm Memory）最小可复现对照实验
//!
//! # 机制
//! 在一类「解法可复用、单次任务有尝试预算」的合成任务上，对比两组：
//! - **baseline（无群体记忆）**：每个智能体对每个场景独立盲猜，不共享经验；
//! - **shared（共享记忆）**：任一智能体试出某场景的正确解后写入共享记忆，
//!   后续智能体检索命中即一次成功。
//!
//! 两组在「需要盲猜」时使用由 (agent, scene) 派生、彼此一致的确定性随机序列，
//! 因此两组差异**纯粹来自记忆命中**（单变量对照）。
//!
//! # 度量
//! - 命中率 hit_rate：任务在尝试预算内被解决的比例；
//! - 首次成功率 first_success_rate：第一次尝试就正确的比例；
//! - 平均尝试次数 avg_attempts：每个任务消耗的尝试数（含失败任务的满预算）；
//! - 记忆覆盖率 coverage：已被群体开垦（写入正确解）的场景比例。
//!
//! # 证据等级
//! `cpu-proto`（CPU 合成机制原型）。它只证明「在可复用任务上，共享成功经验
//! 能提升命中率/首次成功率、降低尝试次数」这一**机制**成立；具体数值依赖
//! 合成任务口径（候选数 K、尝试预算 B、智能体数 A），**不向任意真实任务外推**。

use std::collections::HashMap;

/* ------------------------------ 确定性 PRNG ------------------------------ */

/// SplitMix64：无外部依赖、跨平台逐位一致的确定性伪随机数发生器。
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// 返回 [0, n) 内的整数。
    pub fn below(&mut self, n: u32) -> u32 {
        (self.next_u64() % n as u64) as u32
    }
}

/* -------------------------------- 共享记忆 -------------------------------- */

/// 群体共享记忆库：场景签名 -> 正确解。
#[derive(Debug, Clone, Default)]
pub struct SharedMemory {
    solutions: HashMap<u64, u32>,
    /// 检索命中（一次拿到正确解）次数。
    pub retrieval_hits: u64,
    /// 检索未命中次数。
    pub retrieval_misses: u64,
}

impl SharedMemory {
    pub fn new() -> Self {
        Self::default()
    }

    /// 检索某场景的正确解。
    pub fn lookup(&mut self, scene: u64) -> Option<u32> {
        match self.solutions.get(&scene) {
            Some(&sol) => {
                self.retrieval_hits += 1;
                Some(sol)
            }
            None => {
                self.retrieval_misses += 1;
                None
            }
        }
    }

    /// 将成功解法写入记忆（同场景重复写入幂等）。
    pub fn record(&mut self, scene: u64, solution: u32) {
        self.solutions.entry(scene).or_insert(solution);
    }

    /// 已开垦场景数。
    pub fn solved_scenes(&self) -> usize {
        self.solutions.len()
    }

    /// 记忆覆盖率（0..=1）。
    pub fn coverage(&self, total_scenes: usize) -> f64 {
        if total_scenes == 0 {
            0.0
        } else {
            self.solved_scenes() as f64 / total_scenes as f64
        }
    }
}

/* -------------------------------- 指标汇总 -------------------------------- */

/// 一组实验的累计指标。
#[derive(Debug, Clone, Default)]
pub struct Metrics {
    pub tasks: u64,
    pub solved: u64,
    pub first_success: u64,
    pub attempts: u64,
}

impl Metrics {
    fn add(&mut self, solved: bool, first: bool, attempts: u32) {
        self.tasks += 1;
        if solved {
            self.solved += 1;
        }
        if first {
            self.first_success += 1;
        }
        self.attempts += attempts as u64;
    }

    /// 命中率（预算内解决的比例）。
    pub fn hit_rate(&self) -> f64 {
        ratio(self.solved, self.tasks)
    }

    /// 首次尝试成功率。
    pub fn first_success_rate(&self) -> f64 {
        ratio(self.first_success, self.tasks)
    }

    /// 每任务平均尝试次数。
    pub fn avg_attempts(&self) -> f64 {
        ratio(self.attempts, self.tasks)
    }
}

fn ratio(num: u64, den: u64) -> f64 {
    if den == 0 {
        0.0
    } else {
        num as f64 / den as f64
    }
}

/* ------------------------------ 单次盲猜逻辑 ------------------------------ */

/// 在 `choices` 个选项中、用最多 `budget` 次无放回盲猜寻找 `correct`。
/// 返回 (是否在预算内解决, 是否第一次就中, 实际尝试次数)。
///
/// `rng` 由调用方按 (agent, scene) 构造，保证两组同 (agent, scene) 的盲猜序列一致。
fn blind_attempt(rng: &mut Rng, correct: u32, choices: u32, budget: u32) -> (bool, bool, u32) {
    let mut tried: Vec<u32> = Vec::with_capacity(budget as usize);
    for step in 0..budget {
        // 无放回抽样：拒绝已猜过的选项。
        let mut pick = rng.below(choices);
        while tried.contains(&pick) {
            pick = rng.below(choices);
        }
        tried.push(pick);
        if pick == correct {
            return (true, step == 0, step + 1);
        }
    }
    (false, false, budget)
}

/// 为 (agent, scene) 的盲猜派生确定性 RNG（两组共用，保证单变量对照）。
fn rng_for(base_seed: u64, agent: usize, scene: usize) -> Rng {
    let mut r = Rng::new(base_seed ^ 0xDEAD_BEEF);
    // 两次混合 agent / scene，降低相关性。
    r.next_u64();
    let mix = r.next_u64()
        ^ ((agent as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
        ^ ((scene as u64).wrapping_mul(0xC2B9_AE3D_27D4_EB4F));
    Rng::new(mix)
}

/* ------------------------------ 实验结果与运行 ----------------------------- */

/// 单个智能体（在 shared 组）的逐 agent 结果。
#[derive(Debug, Clone)]
pub struct AgentStat {
    pub agent: usize,
    /// 进入该智能体时的记忆覆盖率。
    pub coverage_in: f64,
    pub hit_rate: f64,
    pub first_success_rate: f64,
    pub avg_attempts: f64,
}

/// 一次完整对照实验的结果。
#[derive(Debug, Clone)]
pub struct TrialResult {
    pub scenes: usize,
    pub choices: u32,
    pub budget: u32,
    pub agents: usize,
    pub baseline: Metrics,
    pub shared: Metrics,
    pub per_agent: Vec<AgentStat>,
    pub final_coverage: f64,
    pub retrieval_hits: u64,
}

impl TrialResult {
    /// 多行、可直接写入论文/日志的汇总（数值为固定 seed 的实测值）。
    pub fn summary(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "口径：scenes={} choices(K)={} budget(B)={} agents(A)={} （证据等级 cpu-proto）\n",
            self.scenes, self.choices, self.budget, self.agents
        ));
        s.push_str("指标                 无记忆(baseline)   共享记忆(shared)\n");
        s.push_str(&format!(
            "命中率               {:>14.3}   {:>14.3}\n",
            self.baseline.hit_rate(),
            self.shared.hit_rate()
        ));
        s.push_str(&format!(
            "首次成功率           {:>14.3}   {:>14.3}\n",
            self.baseline.first_success_rate(),
            self.shared.first_success_rate()
        ));
        s.push_str(&format!(
            "平均尝试次数         {:>14.3}   {:>14.3}\n",
            self.baseline.avg_attempts(),
            self.shared.avg_attempts()
        ));
        s.push_str(&format!(
            "最终记忆覆盖率       {:>14}   {:>14.3}\n",
            "-", self.final_coverage
        ));
        s.push_str(&format!("共享记忆检索命中次数 {}\n", self.retrieval_hits));
        s.push_str("\n逐智能体（shared 组，体现 开拓者->高命中 的过程）：\n");
        for a in &self.per_agent {
            s.push_str(&format!(
                "  agent {:>2}: 覆盖入={:.3} 命中率={:.3} 首次={:.3} 尝试={:.3}\n",
                a.agent, a.coverage_in, a.hit_rate, a.first_success_rate, a.avg_attempts
            ));
        }
        s
    }
}

/// 运行对照实验（确定性：同一参数 + 同一 seed 必得同一结果）。
///
/// - `scenes`：场景（任务签名）数量；
/// - `choices`(K)：每个场景的候选解数量（盲猜空间）；
/// - `budget`(B)：每个任务的最大尝试次数；
/// - `agents`(A)：依次处理全部场景的智能体数量。
pub fn run_experiment(
    scenes: usize,
    choices: u32,
    budget: u32,
    agents: usize,
    seed: u64,
) -> TrialResult {
    let mut baseline = Metrics::default();
    let mut shared = Metrics::default();
    let mut mem = SharedMemory::new();
    let mut per_agent: Vec<AgentStat> = Vec::with_capacity(agents);

    for a in 0..agents {
        let coverage_in = mem.coverage(scenes);
        let m_before = shared.clone();

        for s in 0..scenes {
            let scene_id = s as u64;
            // 每个场景的正确解确定性地取 s % choices。
            let correct = (s as u32) % choices;

            // baseline：永远盲猜。
            let (bsv, bfi, bat) = {
                let mut r = rng_for(seed, a, s);
                blind_attempt(&mut r, correct, choices, budget)
            };
            baseline.add(bsv, bfi, bat);

            // shared：先查记忆，未命中再盲猜；盲猜成功则写入记忆。
            match mem.lookup(scene_id) {
                Some(c) if c == correct => shared.add(true, true, 1),
                _ => {
                    let (ssv, sfi, sat) = {
                        let mut r = rng_for(seed, a, s);
                        blind_attempt(&mut r, correct, choices, budget)
                    };
                    shared.add(ssv, sfi, sat);
                    if ssv {
                        mem.record(scene_id, correct);
                    }
                }
            }
        }

        // 该 agent 在 shared 组的增量指标。
        let d_tasks = shared.tasks - m_before.tasks;
        let d_solved = shared.solved - m_before.solved;
        let d_first = shared.first_success - m_before.first_success;
        let d_att = shared.attempts - m_before.attempts;
        per_agent.push(AgentStat {
            agent: a,
            coverage_in,
            hit_rate: ratio(d_solved, d_tasks),
            first_success_rate: ratio(d_first, d_tasks),
            avg_attempts: ratio(d_att, d_tasks),
        });
    }

    TrialResult {
        scenes,
        choices,
        budget,
        agents,
        baseline,
        shared,
        per_agent,
        final_coverage: mem.coverage(scenes),
        retrieval_hits: mem.retrieval_hits,
    }
}
