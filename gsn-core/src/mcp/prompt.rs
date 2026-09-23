//! MCP 提示模板
//!
//! 兼容 MCP Prompts 规范：可复用的提示词模板

use serde::{Deserialize, Serialize};

/// 提示消息角色
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptRole {
    User,
    Assistant,
    System,
}

/// 提示消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMessage {
    pub role: PromptRole,
    pub content: String,
}

impl PromptMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: PromptRole::User, content: content.into() }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: PromptRole::Assistant, content: content.into() }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self { role: PromptRole::System, content: content.into() }
    }
}

/// 提示参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArgument {
    pub name: String,
    pub description: String,
    pub required: bool,
}

/// 提示模板定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptDefinition {
    pub name: String,
    pub description: String,
    pub arguments: Vec<PromptArgument>,
    /// GSN 扩展：教育场景适用学段
    #[serde(default)]
    pub gsn_age_level: Option<String>,
    /// GSN 扩展：是否苏格拉底式提示
    #[serde(default)]
    pub gsn_socratic: bool,
}

impl PromptDefinition {
    pub fn new(name: String, description: String) -> Self {
        Self {
            name,
            description,
            arguments: Vec::new(),
            gsn_age_level: None,
            gsn_socratic: false,
        }
    }

    pub fn with_argument(mut self, arg: PromptArgument) -> Self {
        self.arguments.push(arg);
        self
    }

    pub fn socratic(mut self) -> Self {
        self.gsn_socratic = true;
        self
    }

    /// 渲染提示模板：替换 {argument} 占位符
    pub fn render(&self, messages: &[PromptMessage], args: &serde_json::Map<String, serde_json::Value>) -> Vec<PromptMessage> {
        messages
            .iter()
            .map(|m| {
                let mut rendered = m.content.clone();
                for (key, value) in args {
                    let placeholder = format!("{{{}}}", key);
                    let replacement = value.as_str().map(|s| s.to_string()).unwrap_or_else(|| value.to_string());
                    rendered = rendered.replace(&placeholder, &replacement);
                }
                PromptMessage {
                    role: m.role,
                    content: rendered,
                }
            })
            .collect()
    }
}
