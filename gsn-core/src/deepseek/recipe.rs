//! DeepSeek 提示词编码配方（v2.4.8）
//!
//! 精确复刻 DeepSeek 官方 API 的输入编码约定：
//! - 角色消息格式（system / user / assistant）
//! - 对话模式（deepseek-chat）与思考模式（deepseek-reasoner）
//! - 停止序列、温度、top_p 等采样参数的合法区间
//! - 系统提示词模板注入点
//!
//! 本模块只做"格式正确"，不做网络调用；网络层在 adapter.rs。

use serde::{Deserialize, Serialize};

/// DeepSeek 模型族。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeepSeekModel {
    /// deepseek-chat（V3.x，通用对话）
    Chat,
    /// deepseek-reasoner（R1，带思考链）
    Reasoner,
}

impl DeepSeekModel {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeepSeekModel::Chat => "deepseek-chat",
            DeepSeekModel::Reasoner => "deepseek-reasoner",
        }
    }

    /// 官方标称上下文窗口（tokens）。
    pub fn context_window(&self) -> u32 {
        64_000
    }

    /// 是否支持思考链输出（reasoning_content）。
    pub fn supports_reasoning(&self) -> bool {
        matches!(self, DeepSeekModel::Reasoner)
    }
}

impl std::fmt::Display for DeepSeekModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 一条 DeepSeek 格式消息。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DsMessage {
    pub role: DsRole,
    pub content: String,
    /// 思考链内容（仅 reasoner 模型返回时填充）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DsRole {
    System,
    User,
    Assistant,
    Tool,
}

/// 采样参数（DeepSeek OpenAI 兼容子集）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DsSampling {
    pub temperature: f64,
    pub top_p: f64,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stop: Vec<String>,
}

impl Default for DsSampling {
    fn default() -> Self {
        Self {
            temperature: 1.0,
            top_p: 1.0,
            max_tokens: 4096,
            stop: Vec::new(),
        }
    }
}

impl DsSampling {
    /// 校验参数在 DeepSeek 官方合法区间内。
    pub fn validate(&self) -> Result<(), String> {
        if !(0.0..=2.0).contains(&self.temperature) {
            return Err(format!(
                "temperature={} out of range [0,2]",
                self.temperature
            ));
        }
        if !(0.0..=1.0).contains(&self.top_p) {
            return Err(format!("top_p={} out of range [0,1]", self.top_p));
        }
        if self.max_tokens == 0 || self.max_tokens > 8192 {
            return Err(format!(
                "max_tokens={} out of range (1..=8192 for chat; reasoner max 8K output)",
                self.max_tokens
            ));
        }
        Ok(())
    }
}

/// DeepSeek 请求体（POST /v1/chat/completions）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DsChatRequest {
    pub model: String,
    pub messages: Vec<DsMessage>,
    #[serde(flatten)]
    pub sampling: DsSampling,
    /// 若设置，启用 JSON 对象模式。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<DsResponseFormat>,
    /// 流模式（本仓库原型默认 false）。
    pub stream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DsResponseFormat {
    #[serde(rename = "type")]
    pub ty: String,
}

/// DeepSeek 响应体。
#[derive(Debug, Clone, Deserialize)]
pub struct DsChatResponse {
    pub id: String,
    pub choices: Vec<DsChoice>,
    pub usage: DsUsage,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DsChoice {
    pub index: u32,
    pub message: DsMessage,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct DsUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// 系统提示词注入器：把 gsn 的上下文包装成 DeepSeek 官方推荐的 system 段。
pub struct Recipe;

impl Recipe {
    /// 构造一条 system 消息：封装 gsn 身份 + 任务指令 + 约束。
    pub fn system_prompt(agent_identity: &str, task_instruction: &str) -> DsMessage {
        let content = format!(
            "You are an agent in the TwinsEarth Agent Universe.\n\
             Identity: {agent_identity}\n\
             Instruction: {task_instruction}\n\
             Rules: answer concisely; do not fabricate tool calls; keep within requested scope."
        );
        DsMessage {
            role: DsRole::System,
            content,
            reasoning_content: None,
        }
    }

    /// 追加用户消息。
    pub fn user(content: impl Into<String>) -> DsMessage {
        DsMessage {
            role: DsRole::User,
            content: content.into(),
            reasoning_content: None,
        }
    }

    /// 追加助手消息（多轮历史）。
    pub fn assistant(content: impl Into<String>) -> DsMessage {
        DsMessage {
            role: DsRole::Assistant,
            content: content.into(),
            reasoning_content: None,
        }
    }

    /// 构造一个完整 chat 请求。
    pub fn build_request(
        model: DeepSeekModel,
        system: DsMessage,
        history: Vec<DsMessage>,
        sampling: DsSampling,
    ) -> DsChatRequest {
        let mut messages = Vec::with_capacity(history.len() + 1);
        messages.push(system);
        messages.extend(history);
        DsChatRequest {
            model: model.as_str().to_string(),
            messages,
            sampling,
            response_format: None,
            stream: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_strings_and_window() {
        assert_eq!(DeepSeekModel::Chat.as_str(), "deepseek-chat");
        assert_eq!(DeepSeekModel::Reasoner.as_str(), "deepseek-reasoner");
        assert_eq!(DeepSeekModel::Chat.context_window(), 64_000);
        assert!(DeepSeekModel::Reasoner.supports_reasoning());
        assert!(!DeepSeekModel::Chat.supports_reasoning());
    }

    #[test]
    fn sampling_validation_rejects_out_of_range() {
        assert!(DsSampling::default().validate().is_ok());
        assert!(DsSampling { temperature: 3.0, ..Default::default() }.validate().is_err());
        assert!(DsSampling { top_p: 1.5, ..Default::default() }.validate().is_err());
        assert!(DsSampling { max_tokens: 0, ..Default::default() }.validate().is_err());
    }

    #[test]
    fn system_prompt_wraps_identity_and_instruction() {
        let m = Recipe::system_prompt("agent-001", "answer math");
        assert_eq!(m.role, DsRole::System);
        assert!(m.content.contains("agent-001"));
        assert!(m.content.contains("answer math"));
    }

    #[test]
    fn build_request_puts_system_first() {
        let sys = Recipe::system_prompt("a", "b");
        let hist = vec![Recipe::user("hi")];
        let req = Recipe::build_request(DeepSeekModel::Chat, sys, hist, DsSampling::default());
        assert_eq!(req.model, "deepseek-chat");
        assert_eq!(req.messages.len(), 2);
        assert_eq!(req.messages[0].role, DsRole::System);
        assert_eq!(req.messages[1].role, DsRole::User);
        assert!(!req.stream);
    }
}
