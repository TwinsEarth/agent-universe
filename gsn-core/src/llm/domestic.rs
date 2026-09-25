//! 国内主流大模型统一适配（v2.5.1）
//!
//! 适配 Kimi / 通义千问 / 智谱GLM / MiniMax / 腾讯混元 / 小米MiLM。
//! 这些模型的 API 均兼容 OpenAI Chat Completions 格式，仅 base_url 与 model 名不同。

use serde::{Deserialize, Serialize};
use super::openai::{OaChatRequest, OaChatResponse, OaMessage, OaRole};

/// 国内模型提供商。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DomesticProvider {
    /// Kimi / 月之暗面 Moonshot
    Kimi,
    /// 通义千问 / 阿里 DashScope
    Qwen,
    /// 智谱 GLM
    Zhipu,
    /// MiniMax
    MiniMax,
    /// 腾讯混元 / 元宝
    Hunyuan,
    /// 小米 MiLM
    Xiaomi,
}

impl DomesticProvider {
    pub fn family(&self) -> &'static str {
        match self {
            DomesticProvider::Kimi => "kimi",
            DomesticProvider::Qwen => "qwen",
            DomesticProvider::Zhipu => "zhipu",
            DomesticProvider::MiniMax => "minimax",
            DomesticProvider::Hunyuan => "hunyuan",
            DomesticProvider::Xiaomi => "xiaomi",
        }
    }

    pub fn base_url(&self) -> &'static str {
        match self {
            DomesticProvider::Kimi => "https://api.moonshot.cn/v1",
            DomesticProvider::Qwen => "https://dashscope.aliyuncs.com/compatible-mode/v1",
            DomesticProvider::Zhipu => "https://open.bigmodel.cn/api/paas/v4",
            DomesticProvider::MiniMax => "https://api.minimax.chat/v1",
            DomesticProvider::Hunyuan => "https://api.hunyuan.cloud.tencent.com/v1",
            DomesticProvider::Xiaomi => "https://api.xiaomi.com/v1",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            DomesticProvider::Kimi => "Kimi (Moonshot)",
            DomesticProvider::Qwen => "通义千问 (Qwen)",
            DomesticProvider::Zhipu => "智谱 GLM",
            DomesticProvider::MiniMax => "MiniMax",
            DomesticProvider::Hunyuan => "腾讯混元 (Hunyuan)",
            DomesticProvider::Xiaomi => "小米 MiLM",
        }
    }
}

/// 具体模型（按提供商分类）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DomesticModel {
    // Kimi
    KimiV1_8k,
    KimiV1_32k,
    KimiV1_128k,
    KimiK2,
    // Qwen
    QwenTurbo,
    QwenPlus,
    QwenMax,
    Qwen25_72b,
    // Zhipu
    Glm4Plus,
    Glm4,
    Glm4Flash,
    // MiniMax
    MiniMaxAbab65,
    MiniMaxText01,
    // Hunyuan
    HunyuanPro,
    HunyuanStandard,
    HunyuanTurbo,
    // Xiaomi
    MiLMLarge,
}

impl DomesticModel {
    pub fn provider(&self) -> DomesticProvider {
        match self {
            DomesticModel::KimiV1_8k | DomesticModel::KimiV1_32k | DomesticModel::KimiV1_128k | DomesticModel::KimiK2 => DomesticProvider::Kimi,
            DomesticModel::QwenTurbo | DomesticModel::QwenPlus | DomesticModel::QwenMax | DomesticModel::Qwen25_72b => DomesticProvider::Qwen,
            DomesticModel::Glm4Plus | DomesticModel::Glm4 | DomesticModel::Glm4Flash => DomesticProvider::Zhipu,
            DomesticModel::MiniMaxAbab65 | DomesticModel::MiniMaxText01 => DomesticProvider::MiniMax,
            DomesticModel::HunyuanPro | DomesticModel::HunyuanStandard | DomesticModel::HunyuanTurbo => DomesticProvider::Hunyuan,
            DomesticModel::MiLMLarge => DomesticProvider::Xiaomi,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            DomesticModel::KimiV1_8k => "moonshot-v1-8k",
            DomesticModel::KimiV1_32k => "moonshot-v1-32k",
            DomesticModel::KimiV1_128k => "moonshot-v1-128k",
            DomesticModel::KimiK2 => "kimi-k2-0711-preview",
            DomesticModel::QwenTurbo => "qwen-turbo",
            DomesticModel::QwenPlus => "qwen-plus",
            DomesticModel::QwenMax => "qwen-max",
            DomesticModel::Qwen25_72b => "qwen2.5-72b-instruct",
            DomesticModel::Glm4Plus => "glm-4-plus",
            DomesticModel::Glm4 => "glm-4",
            DomesticModel::Glm4Flash => "glm-4-flash",
            DomesticModel::MiniMaxAbab65 => "abab6.5s-chat",
            DomesticModel::MiniMaxText01 => "MiniMax-Text-01",
            DomesticModel::HunyuanPro => "hunyuan-pro",
            DomesticModel::HunyuanStandard => "hunyuan-standard",
            DomesticModel::HunyuanTurbo => "hunyuan-turbo",
            DomesticModel::MiLMLarge => "milm-large",
        }
    }

    pub fn context_window(&self) -> u32 {
        match self {
            DomesticModel::KimiV1_8k => 8_000,
            DomesticModel::KimiV1_32k => 32_000,
            DomesticModel::KimiV1_128k | DomesticModel::KimiK2 => 128_000,
            DomesticModel::QwenTurbo | DomesticModel::QwenPlus => 131_000,
            DomesticModel::QwenMax | DomesticModel::Qwen25_72b => 131_000,
            DomesticModel::Glm4Plus | DomesticModel::Glm4 => 128_000,
            DomesticModel::Glm4Flash => 128_000,
            DomesticModel::MiniMaxAbab65 | DomesticModel::MiniMaxText01 => 245_000,
            DomesticModel::HunyuanPro | DomesticModel::HunyuanStandard => 32_000,
            DomesticModel::HunyuanTurbo => 32_000,
            DomesticModel::MiLMLarge => 32_000,
        }
    }
}

/// Mock 客户端（所有国内模型共用 OpenAI 兼容格式）。
pub struct MockDomesticClient {
    pub answer: String,
}

pub trait DomesticClient {
    fn chat(&self, req: &OaChatRequest) -> Result<OaChatResponse, String>;
}

impl DomesticClient for MockDomesticClient {
    fn chat(&self, req: &OaChatRequest) -> Result<OaChatResponse, String> {
        let _ = req;
        Ok(OaChatResponse {
            id: "domestic-mock".into(),
            choices: vec![super::openai::OaChoice {
                message: OaMessage { role: OaRole::Assistant, content: self.answer.clone() },
                finish_reason: "stop".into(),
            }],
            usage: super::openai::OaUsage { prompt_tokens: 15, completion_tokens: 8, total_tokens: 23 },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn providers_have_distinct_families() {
        assert_eq!(DomesticProvider::Kimi.family(), "kimi");
        assert_eq!(DomesticProvider::Qwen.family(), "qwen");
        assert_eq!(DomesticProvider::Zhipu.family(), "zhipu");
        assert_eq!(DomesticProvider::MiniMax.family(), "minimax");
        assert_eq!(DomesticProvider::Hunyuan.family(), "hunyuan");
        assert_eq!(DomesticProvider::Xiaomi.family(), "xiaomi");
    }

    #[test]
    fn providers_have_different_base_urls() {
        assert_ne!(
            DomesticProvider::Kimi.base_url(),
            DomesticProvider::Qwen.base_url()
        );
        assert!(DomesticProvider::Kimi.base_url().contains("moonshot"));
        assert!(DomesticProvider::Qwen.base_url().contains("aliyuncs"));
        assert!(DomesticProvider::Zhipu.base_url().contains("bigmodel"));
    }

    #[test]
    fn models_map_to_correct_provider() {
        assert_eq!(DomesticModel::KimiK2.provider(), DomesticProvider::Kimi);
        assert_eq!(DomesticModel::QwenMax.provider(), DomesticProvider::Qwen);
        assert_eq!(DomesticModel::Glm4Plus.provider(), DomesticProvider::Zhipu);
        assert_eq!(DomesticModel::MiniMaxText01.provider(), DomesticProvider::MiniMax);
        assert_eq!(DomesticModel::HunyuanPro.provider(), DomesticProvider::Hunyuan);
        assert_eq!(DomesticModel::MiLMLarge.provider(), DomesticProvider::Xiaomi);
    }

    #[test]
    fn model_strings_and_windows() {
        assert_eq!(DomesticModel::KimiK2.as_str(), "kimi-k2-0711-preview");
        assert_eq!(DomesticModel::KimiK2.context_window(), 128_000);
        assert_eq!(DomesticModel::QwenMax.as_str(), "qwen-max");
        assert_eq!(DomesticModel::Glm4Plus.as_str(), "glm-4-plus");
    }

    #[test]
    fn mock_client_returns_answer() {
        let client = MockDomesticClient { answer: "你好".into() };
        let req = OaChatRequest {
            model: "qwen-max".into(),
            messages: vec![OaMessage { role: OaRole::User, content: "hi".into() }],
            temperature: 0.7, top_p: 1.0, max_tokens: 256, stream: false,
        };
        let resp = client.chat(&req).unwrap();
        assert_eq!(resp.choices[0].message.content, "你好");
    }
}
