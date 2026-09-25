//! Token 级编解码与上下文窗口管理（v2.4.8）
//!
//! 纯 CPU 原型，不依赖外部 tokenizer：
//! - 按 DeepSeek 官方口径估算 token 数（英文 ~4 char/token，中文 ~1.5 char/token）
//! - 上下文窗口预算分配（system 优先保留，历史从旧往新截断）
//! - 超预算时报错或按策略截断

use super::recipe::{DsChatRequest, DsMessage, DeepSeekModel};

/// 估算一段文本的 token 数（启发式，非真 BPE）。
///
/// DeepSeek 官方未公开精确 BPE；此函数用于预算控制，误差可接受。
pub fn estimate_tokens(text: &str) -> u32 {
    // 粗略：ASCII 按 4 char/token，非 ASCII（中文等）按 1.5 char/token。
    let mut ascii_chars = 0usize;
    let mut cjk_chars = 0usize;
    for c in text.chars() {
        if c.is_ascii() {
            ascii_chars += 1;
        } else {
            cjk_chars += 1;
        }
    }
    let est = (ascii_chars as f64 / 4.0) + (cjk_chars as f64 / 1.5);
    est.ceil() as u32
}

/// 一条消息的 token 估算（含角色开销 ~4 tokens）。
pub fn message_tokens(msg: &DsMessage) -> u32 {
    4 + estimate_tokens(&msg.content)
        + msg
            .reasoning_content
            .as_deref()
            .map(estimate_tokens)
            .unwrap_or(0)
}

/// 上下文预算管理器。
pub struct ContextBudget {
    model: DeepSeekModel,
    reserved_output: u32,
}

impl ContextBudget {
    pub fn new(model: DeepSeekModel, reserved_output: u32) -> Self {
        Self { model, reserved_output }
    }

    /// 输入可用窗口 = 模型上下文窗口 - 预留输出 tokens。
    pub fn input_budget(&self) -> u32 {
        self.model.context_window().saturating_sub(self.reserved_output)
    }

    /// 计算整条请求的 token 占用。
    pub fn estimate_request(&self, req: &DsChatRequest) -> u32 {
        req.messages.iter().map(message_tokens).sum::<u32>() + 3 // 对话格式开销
    }

    /// 判断请求是否超出输入预算。
    pub fn fits(&self, req: &DsChatRequest) -> bool {
        self.estimate_request(req) <= self.input_budget()
    }

    /// 从旧历史往新截断，保留 system 和最近消息，使请求不超预算。
    /// 截断后返回新消息列表（system 永远保留）。
    pub fn truncate_history(&self, mut messages: Vec<DsMessage>) -> Vec<DsMessage> {
        let budget = self.input_budget();
        // 永远保留第一条 system。
        let system = messages.first().cloned();
        let mut rest: Vec<DsMessage> = messages.drain(1..).collect();

        let mut kept: Vec<DsMessage> = Vec::new();
        let mut used: u32 = system.as_ref().map(message_tokens).unwrap_or(0);

        // 从最新一条往回保留，直到预算用尽。
        while let Some(msg) = rest.pop() {
            let cost = message_tokens(&msg);
            if used + cost > budget {
                break;
            }
            used += cost;
            kept.push(msg);
        }
        kept.reverse();

        let mut out = Vec::new();
        if let Some(s) = system {
            out.push(s);
        }
        out.extend(kept);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deepseek::recipe::{Recipe, DsSampling};

    #[test]
    fn estimate_tokens_ascii_and_cjk() {
        // 4 ASCII chars ~= 1 token
        assert!(estimate_tokens("hello world this is ascii") >= 5);
        // CJK denser
        let cjk = "这是一段中文用于测试token估算";
        let ascii = "hello world this is ascii text";
        assert!(estimate_tokens(cjk) > estimate_tokens(ascii) / 2);
    }

    #[test]
    fn budget_reserves_output() {
        let b = ContextBudget::new(DeepSeekModel::Chat, 4096);
        assert_eq!(b.input_budget(), 64_000 - 4096);
    }

    #[test]
    fn truncate_keeps_system_and_recent() {
        let b = ContextBudget::new(DeepSeekModel::Chat, 63_000); // 仅留 1000 input tokens
        let mut msgs = vec![Recipe::system_prompt("sys", "rule")];
        // 塞很多长消息，确保超预算
        for i in 0..100 {
            msgs.push(Recipe::user(&format!(
                "This is a long message number {i} with substantial content to exceed the token budget threshold quickly enough for truncation to kick in."
            )));
        }
        let truncated = b.truncate_history(msgs);
        // system 保留
        assert_eq!(truncated[0].role, crate::deepseek::recipe::DsRole::System);
        // 最近消息保留，旧消息被截断
        assert!(truncated.len() < 101, "truncated len={}", truncated.len());
        // 不超预算
        let req = Recipe::build_request(
            DeepSeekModel::Chat,
            truncated[0].clone(),
            truncated[1..].to_vec(),
            DsSampling::default(),
        );
        assert!(b.fits(&req));
    }
}
