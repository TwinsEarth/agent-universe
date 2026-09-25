//! 多协议格式互转（v2.4.8）
//!
//! 在三种格式之间无损转换：
//! - DeepSeek 原生 Chat Completions 格式（recipe.rs 定义）
//! - OpenAI Chat Completions 兼容格式（DeepSeek API 本身兼容 OpenAI，此处做结构对齐）
//! - gsn 内部 Message 信封（collaboration / aca 用的通用消息）
//!
//! 互转保证：角色枚举不丢、文本不丢、reasoning_content 仅 reasoner 保留。

use super::recipe::{DsChatRequest, DsChatResponse, DsMessage, DsRole, DsSampling};

/// gsn 内部通用消息信封（简化版，避免跨 crate 依赖）。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GsnMessage {
    pub role: String,
    pub content: String,
}

/// OpenAI 兼容请求结构（仅做序列化对齐，不发网络）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OaiChatRequest {
    pub model: String,
    pub messages: Vec<GsnMessage>,
    pub temperature: f64,
    pub top_p: f64,
    pub max_tokens: u32,
    pub stream: bool,
}

/// 协议转换器：无状态。
pub struct Protocol;

impl Protocol {
    /// gsn 内部消息 → DeepSeek 消息。
    pub fn gsn_to_ds(msg: &GsnMessage) -> Result<DsMessage, String> {
        let role = match msg.role.as_str() {
            "system" => DsRole::System,
            "user" => DsRole::User,
            "assistant" => DsRole::Assistant,
            "tool" => DsRole::Tool,
            other => return Err(format!("unknown role '{other}'")),
        };
        Ok(DsMessage {
            role,
            content: msg.content.clone(),
            reasoning_content: None,
        })
    }

    /// DeepSeek 消息 → gsn 内部消息。
    pub fn ds_to_gsn(msg: &DsMessage) -> GsnMessage {
        GsnMessage {
            role: match msg.role {
                DsRole::System => "system",
                DsRole::User => "user",
                DsRole::Assistant => "assistant",
                DsRole::Tool => "tool",
            }
            .to_string(),
            content: msg.content.clone(),
        }
    }

    /// DeepSeek 请求 → OpenAI 兼容请求（展平 sampling）。
    pub fn ds_to_oai(req: &DsChatRequest) -> OaiChatRequest {
        OaiChatRequest {
            model: req.model.clone(),
            messages: req.messages.iter().map(Self::ds_to_gsn).collect(),
            temperature: req.sampling.temperature,
            top_p: req.sampling.top_p,
            max_tokens: req.sampling.max_tokens,
            stream: req.stream,
        }
    }

    /// OpenAI 兼容请求 → DeepSeek 请求。
    pub fn oai_to_ds(req: &OaiChatRequest) -> Result<DsChatRequest, String> {
        let mut messages = Vec::with_capacity(req.messages.len());
        for m in &req.messages {
            messages.push(Self::gsn_to_ds(m)?);
        }
        Ok(DsChatRequest {
            model: req.model.clone(),
            messages,
            sampling: DsSampling {
                temperature: req.temperature,
                top_p: req.top_p,
                max_tokens: req.max_tokens,
                stop: Vec::new(),
            },
            response_format: None,
            stream: req.stream,
        })
    }

    /// 从 DeepSeek 响应中提取纯文本答案（reasoner 模型同时返回 reasoning）。
    pub fn extract_answer(resp: &DsChatResponse) -> Result<String, String> {
        let choice = resp.choices.first().ok_or("empty choices")?;
        Ok(choice.message.content.clone())
    }

    /// 从 DeepSeek reasoner 响应中提取思考链。
    pub fn extract_reasoning(resp: &DsChatResponse) -> Option<String> {
        resp.choices
            .first()
            .and_then(|c| c.message.reasoning_content.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deepseek::recipe::{Recipe, DeepSeekModel};

    #[test]
    fn gsn_ds_roundtrip_preserves_role_and_content() {
        let g = GsnMessage { role: "user".into(), content: "hello".into() };
        let ds = Protocol::gsn_to_ds(&g).unwrap();
        assert_eq!(ds.role, DsRole::User);
        assert_eq!(ds.content, "hello");
        let back = Protocol::ds_to_gsn(&ds);
        assert_eq!(g, back);
    }

    #[test]
    fn unknown_role_rejected() {
        let g = GsnMessage { role: "wizard".into(), content: "x".into() };
        assert!(Protocol::gsn_to_ds(&g).is_err());
    }

    #[test]
    fn ds_oai_roundtrip_preserves_sampling() {
        let sys = Recipe::system_prompt("a", "b");
        let req = Recipe::build_request(
            DeepSeekModel::Chat,
            sys,
            vec![Recipe::user("hi")],
            DsSampling { temperature: 0.7, top_p: 0.9, max_tokens: 512, stop: vec![] },
        );
        let oai = Protocol::ds_to_oai(&req);
        assert_eq!(oai.temperature, 0.7);
        assert_eq!(oai.max_tokens, 512);
        let back = Protocol::oai_to_ds(&oai).unwrap();
        assert_eq!(back.model, req.model);
        assert_eq!(back.messages.len(), req.messages.len());
    }

    #[test]
    fn extract_answer_and_reasoning() {
        let json = r#"{
            "id":"x","choices":[{"index":0,"message":{"role":"assistant","content":"42","reasoning_content":"think"},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}
        }"#;
        let resp: DsChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(Protocol::extract_answer(&resp).unwrap(), "42");
        assert_eq!(Protocol::extract_reasoning(&resp).unwrap(), "think");
    }
}
