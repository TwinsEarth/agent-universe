//! 异构 LLM 多智能体协商（v2.4.7）
//!
//! 不同来源/架构的 LLM（GPT 系、Claude 系、本地开源等）独立提案，
//! 经评估评分、内部加权投票收敛；跨模型记忆协作让异构模型共享经验。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 一个异构 LLM 节点。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmModel {
    pub id: String,
    pub family: String, // gpt / claude / local / ...
    /// 历史可信度权重（0~1），投票时加权。
    pub weight: f64,
}

/// 模型对问题的独立提案。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub model_id: String,
    pub answer: String,
    /// 模型自报置信度（0~1）。
    pub confidence: f64,
}

/// 协商会话：收集提案 → 评分 → 内部投票 → 收敛。
#[derive(Debug, Clone, Default)]
pub struct Deliberation {
    models: HashMap<String, LlmModel>,
    proposals: Vec<Proposal>,
}

impl Deliberation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_model(&mut self, m: LlmModel) {
        self.models.insert(m.id.clone(), m);
    }

    /// 提交一个独立提案（模型必须已登记）。
    pub fn propose(&mut self, p: Proposal) -> Result<(), String> {
        if !self.models.contains_key(&p.model_id) {
            return Err(format!("unknown model {}", p.model_id));
        }
        self.proposals.push(p);
        Ok(())
    }

    /// 评估评分：某提案得分 = 模型历史权重 × 提案置信度。
    pub fn score(&self, p: &Proposal) -> f64 {
        self.models
            .get(&p.model_id)
            .map(|m| m.weight * p.confidence)
            .unwrap_or(0.0)
    }

    /// 内部投票：相同答案的提案按 score 累加票数，得票最高者胜出。
    pub fn vote(&self) -> Option<(String, f64)> {
        let mut tally: HashMap<String, f64> = HashMap::new();
        for p in &self.proposals {
            *tally.entry(p.answer.clone()).or_insert(0.0) += self.score(p);
        }
        tally
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
    }

    pub fn proposal_count(&self) -> usize {
        self.proposals.len()
    }
}

/// 跨模型记忆协作：异构模型共享经验条目，标注来源模型与可信度。
#[derive(Debug, Clone, Default)]
pub struct CrossModelMemory {
    /// task_key -> (来源模型, 经验, 可信度)
    entries: HashMap<String, Vec<(String, String, f64)>>,
}

impl CrossModelMemory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn share(&mut self, task_key: &str, model_id: &str, note: &str, confidence: f64) {
        self.entries
            .entry(task_key.into())
            .or_default()
            .push((model_id.into(), note.into(), confidence));
    }

    /// 跨模型检索：聚合不同模型对同一任务的经验，按可信度取最高。
    pub fn recall_best(&self, task_key: &str) -> Option<(String, String)> {
        self.entries
            .get(task_key)?
            .iter()
            .max_by(|a, b| a.2.partial_cmp(&b.2).unwrap())
            .map(|(m, n, _)| (m.clone(), n.clone()))
    }

    /// 有多少个不同模型对该任务贡献过经验（异构独立性）。
    pub fn distinct_models(&self, task_key: &str) -> usize {
        self.entries
            .get(task_key)
            .map(|v| {
                let mut s: Vec<&str> = v.iter().map(|(m, _, _)| m.as_str()).collect();
                s.sort_unstable();
                s.dedup();
                s.len()
            })
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(id: &str, w: f64) -> LlmModel {
        LlmModel { id: id.into(), family: "x".into(), weight: w }
    }

    #[test]
    fn vote_picks_high_scoring_answer() {
        let mut d = Deliberation::new();
        d.add_model(model("gpt", 0.9));
        d.add_model(model("claude", 0.8));
        d.add_model(model("local", 0.5));
        d.propose(Proposal { model_id: "gpt".into(), answer: "A".into(), confidence: 0.95 }).unwrap();
        d.propose(Proposal { model_id: "claude".into(), answer: "A".into(), confidence: 0.9 }).unwrap();
        d.propose(Proposal { model_id: "local".into(), answer: "B".into(), confidence: 0.6 }).unwrap();
        let (answer, votes) = d.vote().unwrap();
        assert_eq!(answer, "A");
        assert!(votes > 0.0);
    }

    #[test]
    fn rejects_unknown_model_proposal() {
        let mut d = Deliberation::new();
        let r = d.propose(Proposal { model_id: "ghost".into(), answer: "x".into(), confidence: 0.5 });
        assert!(r.is_err());
    }

    #[test]
    fn cross_model_memory_recalls_best_and_counts_independence() {
        let mut m = CrossModelMemory::new();
        m.share("math", "gpt", "chain-of-thought", 0.7);
        m.share("math", "claude", "decompose", 0.95);
        m.share("math", "gpt", "retry", 0.5);
        let (src, note) = m.recall_best("math").unwrap();
        assert_eq!(src, "claude");
        assert_eq!(note, "decompose");
        assert_eq!(m.distinct_models("math"), 2);
    }
}
