//! DeepSeek 模型适配器（v2.4.8）
//!
//! 把 DeepSeek 后端包装成 agent-universe 异构 LLM 协商可用的模型节点。
//! - 配置（model / base_url / api_key / sampling）
//! - 准备请求（recipe + protocol + tokenizer 三步）
//! - 接入 Deliberation：family = "deepseek"
//!
//! 本模块不做真实 HTTP 调用（保持 CPU 原型可验证）；
//! 真实网络注入通过 DeepSeekClient trait 抽象，默认提供 MockClient。

use super::recipe::{
    DeepSeekModel, DsChatRequest, DsChatResponse, DsMessage, DsSampling, Recipe,
};
use super::protocol::Protocol;
use super::tokenizer::{ContextBudget, estimate_tokens};
use crate::collaboration::hetero_llm::{LlmModel, Proposal};

/// DeepSeek 后端配置。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeepSeekConfig {
    pub model: DeepSeekModel,
    pub base_url: String,
    pub api_key_env: String,
    pub sampling: DsSampling,
}

impl Default for DeepSeekConfig {
    fn default() -> Self {
        Self {
            model: DeepSeekModel::Chat,
            base_url: "https://api.deepseek.com/v1".into(),
            api_key_env: "DEEPSEEK_API_KEY".into(),
            sampling: DsSampling::default(),
        }
    }
}

/// 模型调用抽象：便于替换为真实 HTTP 客户端。
pub trait DeepSeekClient {
    fn chat(&self, req: &DsChatRequest) -> Result<DsChatResponse, String>;
}

/// 固定应答的 Mock 客户端（CPU 原型，不发网络）。
pub struct MockDeepSeekClient {
    pub answer: String,
    pub reasoning: Option<String>,
}

impl DeepSeekClient for MockDeepSeekClient {
    fn chat(&self, req: &DsChatRequest) -> Result<DsChatResponse, String> {
        let usage_prompt: u32 = req.messages.iter().map(super::tokenizer::message_tokens).sum();
        let content = self.answer.clone();
        let completion = estimate_tokens(&content);
        Ok(DsChatResponse {
            id: format!("mock-{}", uuid::Uuid::new_v4()),
            choices: vec![crate::deepseek::recipe::DsChoice {
                index: 0,
                message: DsMessage {
                    role: crate::deepseek::recipe::DsRole::Assistant,
                    content,
                    reasoning_content: self.reasoning.clone(),
                },
                finish_reason: "stop".into(),
            }],
            usage: crate::deepseek::recipe::DsUsage {
                prompt_tokens: usage_prompt,
                completion_tokens: completion,
                total_tokens: usage_prompt + completion,
            },
        })
    }
}

/// DeepSeek 适配器：把配置 + 客户端 + 预算打包成可调用节点。
pub struct DeepSeekAdapter<C: DeepSeekClient> {
    pub config: DeepSeekConfig,
    client: C,
    budget: ContextBudget,
}

impl<C: DeepSeekClient> DeepSeekAdapter<C> {
    pub fn new(config: DeepSeekConfig, client: C) -> Self {
        let reserved = config.sampling.max_tokens;
        let model = config.model;
        Self { config, client, budget: ContextBudget::new(model, reserved) }
    }

    /// 暴露给协商层的 LlmModel（family=deepseek）。
    pub fn as_llm_model(&self, id: String, weight: f64) -> LlmModel {
        LlmModel {
            id,
            family: "deepseek".into(),
            weight,
        }
    }

    /// 准备并执行一次对话，返回纯文本答案 + token 用量。
    pub fn chat(
        &self,
        agent_identity: &str,
        task_instruction: &str,
        user_query: &str,
        history: Vec<DsMessage>,
    ) -> Result<(String, u32), String> {
        self.config.sampling.validate()?;
        let system = Recipe::system_prompt(agent_identity, task_instruction);
        // 把用户查询作为最新一条 user 消息追加到历史。
        let mut full_history = history;
        full_history.push(Recipe::user(user_query));
        let mut req = Recipe::build_request(
            self.config.model,
            system,
            full_history,
            self.config.sampling.clone(),
        );
        // 上下文预算：超预算则截断历史。
        if !self.budget.fits(&req) {
            req.messages = self.budget.truncate_history(req.messages);
        }
        let resp = self.client.chat(&req)?;
        let answer = Protocol::extract_answer(&resp)?;
        Ok((answer, resp.usage.total_tokens))
    }

    /// 便捷方法：产出一个 Proposal（用于 Deliberation.propose）。
    pub fn propose(
        &self,
        model_id: &str,
        confidence: f64,
        agent_identity: &str,
        task_instruction: &str,
        user_query: &str,
    ) -> Result<Proposal, String> {
        let (answer, _usage) =
            self.chat(agent_identity, task_instruction, user_query, vec![])?;
        Ok(Proposal {
            model_id: model_id.into(),
            answer,
            confidence,
        })
    }

    pub fn input_budget(&self) -> u32 {
        self.budget.input_budget()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deepseek::recipe::DsSampling;

    #[test]
    fn adapter_chat_with_mock_returns_answer() {
        let cfg = DeepSeekConfig::default();
        let client = MockDeepSeekClient {
            answer: "42".into(),
            reasoning: Some("thought".into()),
        };
        let adapter = DeepSeekAdapter::new(cfg, client);
        let (ans, usage) = adapter.chat("agent-1", "math", "what is 6*7", vec![]).unwrap();
        assert_eq!(ans, "42");
        assert!(usage > 0);
    }

    #[test]
    fn adapter_produces_deepseek_family_llm_model() {
        let cfg = DeepSeekConfig::default();
        let adapter = DeepSeekAdapter::new(cfg, MockDeepSeekClient { answer: "x".into(), reasoning: None });
        let m = adapter.as_llm_model("ds-1".into(), 0.8);
        assert_eq!(m.family, "deepseek");
        assert_eq!(m.id, "ds-1");
        assert!((m.weight - 0.8).abs() < 1e-9);
    }

    #[test]
    fn adapter_rejects_invalid_sampling() {
        let cfg = DeepSeekConfig { sampling: DsSampling { temperature: 9.0, ..Default::default() }, ..Default::default() };
        let adapter = DeepSeekAdapter::new(cfg, MockDeepSeekClient { answer: "x".into(), reasoning: None });
        assert!(adapter.chat("a", "b", "c", vec![]).is_err());
    }

    #[test]
    fn adapter_propose_into_deliberation() {
        use crate::collaboration::hetero_llm::{Deliberation, Proposal};
        let cfg = DeepSeekConfig::default();
        let adapter = DeepSeekAdapter::new(cfg, MockDeepSeekClient { answer: "A".into(), reasoning: None });
        let mut d = Deliberation::new();
        d.add_model(adapter.as_llm_model("ds-1".into(), 0.9));
        let p = adapter.propose("ds-1", 0.95, "a", "b", "c").unwrap();
        d.propose(p).unwrap();
        assert_eq!(d.proposal_count(), 1);
    }
}
