//! 统一多模型适配器（v2.4.9）
//!
//! 把 OpenAI / Gemini / Anthropic 三家包装成异构 LLM 协商可用的模型节点。
//! 每家都产出 family="openai"/"gemini"/"anthropic" 的 LlmModel。

use crate::collaboration::hetero_llm::{LlmModel, Proposal};
use super::openai::{MockOpenAiClient, OaChatRequest, OpenAiClient, OpenAiModel, OaMessage, OaRole};
use super::gemini::{GeContent, GeGenConfig, GePart, GeRequest, GeminiClient, GeminiModel, MockGeminiClient};
use super::anthropic::{AnMessage, AnRequest, AnRole, AnthropicClient, AnthropicModel, MockAnthropicClient};
use super::doubao::{DbChatRequest, DbMessage, DbRole, DbThinking, DbThinkingType, DoubaoClient, DoubaoModel, MockDoubaoClient};
use super::domestic::{DomesticClient, DomesticModel, DomesticProvider, MockDomesticClient};

/// 统一 LLM 后端枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmBackend {
    OpenAi(OpenAiModel),
    Gemini(GeminiModel),
    Anthropic(AnthropicModel),
    DeepSeek,
    Doubao(DoubaoModel),
}

impl LlmBackend {
    pub fn family(&self) -> &'static str {
        match self {
            LlmBackend::OpenAi(_) => "openai",
            LlmBackend::Gemini(_) => "gemini",
            LlmBackend::Anthropic(_) => "anthropic",
            LlmBackend::DeepSeek => "deepseek",
            LlmBackend::Doubao(_) => "doubao",
        }
    }
}

/// 统一推理结果。
#[derive(Debug, Clone)]
pub struct LlmResult {
    pub answer: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

/// 多模型适配器：OpenAI 后端。
pub struct OpenAiAdapter {
    model: OpenAiModel,
    client: MockOpenAiClient,
}

impl OpenAiAdapter {
    pub fn new(model: OpenAiModel, answer: &str) -> Self {
        Self { model, client: MockOpenAiClient { answer: answer.into() } }
    }

    pub fn as_llm_model(&self, id: String, weight: f64) -> LlmModel {
        LlmModel { id, family: "openai".into(), weight }
    }

    pub fn chat(&self, user_query: &str) -> LlmResult {
        let req = OaChatRequest {
            model: self.model.as_str().into(),
            messages: vec![super::openai::OaMessage {
                role: super::openai::OaRole::User,
                content: user_query.into(),
            }],
            temperature: 0.7,
            top_p: 1.0,
            max_tokens: 4096,
            stream: false,
        };
        let resp = self.client.chat(&req).unwrap();
        LlmResult {
            answer: resp.choices[0].message.content.clone(),
            prompt_tokens: resp.usage.prompt_tokens,
            completion_tokens: resp.usage.completion_tokens,
        }
    }

    pub fn propose(&self, model_id: &str, confidence: f64) -> Proposal {
        let r = self.chat("");
        Proposal { model_id: model_id.into(), answer: r.answer, confidence }
    }
}

/// 多模型适配器：Gemini 后端。
pub struct GeminiAdapter {
    #[allow(dead_code)] // model is in URL path, not request body
    model: GeminiModel,
    client: MockGeminiClient,
}

impl GeminiAdapter {
    pub fn new(model: GeminiModel, answer: &str) -> Self {
        Self { model, client: MockGeminiClient { answer: answer.into() } }
    }

    pub fn as_llm_model(&self, id: String, weight: f64) -> LlmModel {
        LlmModel { id, family: "gemini".into(), weight }
    }

    pub fn chat(&self, user_query: &str) -> LlmResult {
        let req = GeRequest {
            contents: vec![GeContent {
                role: "user".into(),
                parts: vec![GePart { text: user_query.into() }],
            }],
            system_instruction: None,
            generation_config: GeGenConfig { temperature: 0.7, top_p: 0.9, max_output_tokens: 4096 },
        };
        let resp = self.client.generate(&req).unwrap();
        LlmResult {
            answer: resp.candidates[0].content.parts[0].text.clone(),
            prompt_tokens: resp.usage_metadata.prompt_token_count,
            completion_tokens: resp.usage_metadata.candidates_token_count,
        }
    }

    pub fn propose(&self, model_id: &str, confidence: f64) -> Proposal {
        let r = self.chat("");
        Proposal { model_id: model_id.into(), answer: r.answer, confidence }
    }
}

/// 多模型适配器：Anthropic 后端。
pub struct AnthropicAdapter {
    model: AnthropicModel,
    client: MockAnthropicClient,
}

impl AnthropicAdapter {
    pub fn new(model: AnthropicModel, answer: &str) -> Self {
        Self { model, client: MockAnthropicClient { answer: answer.into() } }
    }

    pub fn as_llm_model(&self, id: String, weight: f64) -> LlmModel {
        LlmModel { id, family: "anthropic".into(), weight }
    }

    pub fn chat(&self, user_query: &str) -> LlmResult {
        let req = AnRequest {
            model: self.model.as_str().into(),
            messages: vec![AnMessage { role: AnRole::User, content: user_query.into() }],
            system: None,
            max_tokens: 4096,
            temperature: 0.7,
            top_p: 1.0,
        };
        let resp = self.client.messages(&req).unwrap();
        LlmResult {
            answer: resp.content[0].text.clone(),
            prompt_tokens: resp.usage.input_tokens,
            completion_tokens: resp.usage.output_tokens,
        }
    }

    pub fn propose(&self, model_id: &str, confidence: f64) -> Proposal {
        let r = self.chat("");
        Proposal { model_id: model_id.into(), answer: r.answer, confidence }
    }
}

/// 多模型适配器：豆包/火山引擎方舟后端（v2.5.0）。
pub struct DoubaoAdapter {
    model: DoubaoModel,
    client: MockDoubaoClient,
    thinking: bool,
}

impl DoubaoAdapter {
    pub fn new(model: DoubaoModel, answer: &str) -> Self {
        Self {
            model,
            client: MockDoubaoClient { answer: answer.into(), reasoning: None },
            thinking: false,
        }
    }

    pub fn with_thinking(mut self, enabled: bool) -> Self {
        self.thinking = enabled && self.model.supports_reasoning();
        self
    }

    pub fn as_llm_model(&self, id: String, weight: f64) -> LlmModel {
        LlmModel { id, family: "doubao".into(), weight }
    }

    pub fn chat(&self, user_query: &str) -> LlmResult {
        let thinking = if self.thinking {
            Some(DbThinking { ty: DbThinkingType::Enabled })
        } else {
            None
        };
        let req = DbChatRequest {
            model: self.model.as_str().into(),
            messages: vec![DbMessage { role: DbRole::User, content: user_query.into() }],
            temperature: 0.7,
            top_p: 1.0,
            max_tokens: 4096,
            stream: false,
            thinking,
        };
        let resp = self.client.chat(&req).unwrap();
        LlmResult {
            answer: resp.choices[0].message.content.clone(),
            prompt_tokens: resp.usage.prompt_tokens,
            completion_tokens: resp.usage.completion_tokens,
        }
    }

    pub fn propose(&self, model_id: &str, confidence: f64) -> Proposal {
        let r = self.chat("");
        Proposal { model_id: model_id.into(), answer: r.answer, confidence }
    }
}

/// 多模型适配器：国内模型统一后端（v2.5.1）。
pub struct DomesticAdapter {
    model: DomesticModel,
    client: MockDomesticClient,
}

impl DomesticAdapter {
    pub fn new(model: DomesticModel, answer: &str) -> Self {
        Self { model, client: MockDomesticClient { answer: answer.into() } }
    }

    pub fn provider(&self) -> DomesticProvider {
        self.model.provider()
    }

    pub fn as_llm_model(&self, id: String, weight: f64) -> LlmModel {
        LlmModel { id, family: self.model.provider().family().into(), weight }
    }

    pub fn chat(&self, user_query: &str) -> LlmResult {
        let req = OaChatRequest {
            model: self.model.as_str().into(),
            messages: vec![OaMessage { role: OaRole::User, content: user_query.into() }],
            temperature: 0.7,
            top_p: 1.0,
            max_tokens: 4096,
            stream: false,
        };
        let resp = self.client.chat(&req).unwrap();
        LlmResult {
            answer: resp.choices[0].message.content.clone(),
            prompt_tokens: resp.usage.prompt_tokens,
            completion_tokens: resp.usage.completion_tokens,
        }
    }

    pub fn propose(&self, model_id: &str, confidence: f64) -> Proposal {
        let r = self.chat("");
        Proposal { model_id: model_id.into(), answer: r.answer, confidence }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collaboration::hetero_llm::Deliberation;

    #[test]
    fn openai_adapter_produces_family_openai() {
        let a = OpenAiAdapter::new(OpenAiModel::Gpt4o, "A");
        let m = a.as_llm_model("gpt-1".into(), 0.9);
        assert_eq!(m.family, "openai");
        let r = a.chat("hi");
        assert_eq!(r.answer, "A");
    }

    #[test]
    fn gemini_adapter_produces_family_gemini() {
        let a = GeminiAdapter::new(GeminiModel::Gemini15Pro, "B");
        let m = a.as_llm_model("gem-1".into(), 0.8);
        assert_eq!(m.family, "gemini");
        let r = a.chat("hi");
        assert_eq!(r.answer, "B");
    }

    #[test]
    fn anthropic_adapter_produces_family_anthropic() {
        let a = AnthropicAdapter::new(AnthropicModel::ClaudeSonnet4, "C");
        let m = a.as_llm_model("cl-1".into(), 0.85);
        assert_eq!(m.family, "anthropic");
        let r = a.chat("hi");
        assert_eq!(r.answer, "C");
    }

    #[test]
    fn doubao_adapter_produces_family_doubao() {
        let a = DoubaoAdapter::new(DoubaoModel::Seed21Pro, "D");
        let m = a.as_llm_model("db-1".into(), 0.88);
        assert_eq!(m.family, "doubao");
        let r = a.chat("你好");
        assert_eq!(r.answer, "D");
    }

    #[test]
    fn doubao_thinking_only_on_supported_models() {
        let pro = DoubaoAdapter::new(DoubaoModel::Seed21Pro, "x").with_thinking(true);
        assert!(pro.thinking);
        let old = DoubaoAdapter::new(DoubaoModel::Doubao15Pro32k, "x").with_thinking(true);
        assert!(!old.thinking); // 不支持深度思考的模型自动关闭
    }

    #[test]
    fn domestic_adapter_kimi_family() {
        let a = DomesticAdapter::new(DomesticModel::KimiK2, "K");
        assert_eq!(a.provider(), DomesticProvider::Kimi);
        let m = a.as_llm_model("kimi-1".into(), 0.82);
        assert_eq!(m.family, "kimi");
        assert_eq!(a.chat("你好").answer, "K");
    }

    #[test]
    fn domestic_adapter_qwen_family() {
        let a = DomesticAdapter::new(DomesticModel::QwenMax, "Q");
        assert_eq!(a.provider(), DomesticProvider::Qwen);
        let m = a.as_llm_model("qwen-1".into(), 0.8);
        assert_eq!(m.family, "qwen");
    }

    #[test]
    fn domestic_adapter_zhipu_family() {
        let a = DomesticAdapter::new(DomesticModel::Glm4Plus, "Z");
        let m = a.as_llm_model("glm-1".into(), 0.78);
        assert_eq!(m.family, "zhipu");
    }

    #[test]
    fn all_six_domestic_providers_deliberate() {
        let kimi = DomesticAdapter::new(DomesticModel::KimiK2, "A");
        let qwen = DomesticAdapter::new(DomesticModel::QwenMax, "A");
        let zhipu = DomesticAdapter::new(DomesticModel::Glm4Plus, "B");
        let minimax = DomesticAdapter::new(DomesticModel::MiniMaxText01, "A");
        let hunyuan = DomesticAdapter::new(DomesticModel::HunyuanPro, "C");
        let xiaomi = DomesticAdapter::new(DomesticModel::MiLMLarge, "B");

        let mut d = Deliberation::new();
        d.add_model(kimi.as_llm_model("kimi".into(), 0.82));
        d.add_model(qwen.as_llm_model("qwen".into(), 0.80));
        d.add_model(zhipu.as_llm_model("glm".into(), 0.78));
        d.add_model(minimax.as_llm_model("mm".into(), 0.75));
        d.add_model(hunyuan.as_llm_model("hy".into(), 0.70));
        d.add_model(xiaomi.as_llm_model("xm".into(), 0.65));

        d.propose(kimi.propose("kimi", 0.9)).unwrap();
        d.propose(qwen.propose("qwen", 0.85)).unwrap();
        d.propose(zhipu.propose("glm", 0.7)).unwrap();
        d.propose(minimax.propose("mm", 0.88)).unwrap();
        d.propose(hunyuan.propose("hy", 0.6)).unwrap();
        d.propose(xiaomi.propose("xm", 0.75)).unwrap();

        let (winner, _) = d.vote().unwrap();
        // A wins: kimi 0.82*0.9 + qwen 0.80*0.85 + mm 0.75*0.88 = 0.738+0.68+0.66 = 2.078
        // B: glm 0.78*0.7 + xm 0.65*0.75 = 0.546+0.4875 = 1.03
        // C: hy 0.70*0.6 = 0.42
        assert_eq!(winner, "A");
        assert_eq!(d.proposal_count(), 6);
    }

    #[test]
    fn three_backends_deliberate_together() {
        let openai = OpenAiAdapter::new(OpenAiModel::Gpt4o, "A");
        let gemini = GeminiAdapter::new(GeminiModel::Gemini15Pro, "A");
        let anthropic = AnthropicAdapter::new(AnthropicModel::ClaudeSonnet4, "B");

        let mut d = Deliberation::new();
        d.add_model(openai.as_llm_model("gpt".into(), 0.9));
        d.add_model(gemini.as_llm_model("gem".into(), 0.8));
        d.add_model(anthropic.as_llm_model("claude".into(), 0.85));

        d.propose(openai.propose("gpt", 0.95)).unwrap();
        d.propose(gemini.propose("gem", 0.9)).unwrap();
        d.propose(anthropic.propose("claude", 0.7)).unwrap();

        let (winner, votes) = d.vote().unwrap();
        // A wins (2 votes: gpt 0.9*0.95 + gem 0.8*0.9) vs B (claude 0.85*0.7)
        assert_eq!(winner, "A");
        assert!(votes > 0.0);
        assert_eq!(d.proposal_count(), 3);
    }

    #[test]
    fn backend_families_are_distinct() {
        assert_eq!(LlmBackend::OpenAi(OpenAiModel::Gpt4o).family(), "openai");
        assert_eq!(LlmBackend::Gemini(GeminiModel::Gemini15Pro).family(), "gemini");
        assert_eq!(LlmBackend::Anthropic(AnthropicModel::ClaudeSonnet4).family(), "anthropic");
        assert_eq!(LlmBackend::DeepSeek.family(), "deepseek");
    }
}
