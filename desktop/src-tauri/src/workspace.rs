// Agent Universe v2.9.0 — 默认工作区 & 应用配置（模型提供商 / 过程展示 / 后台）
// 配置存放于 ~/.agent-universe/config.json，工作区位于 ~/.agent-universe/workspaces/default。

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// App 数据根目录
pub fn app_data_dir() -> PathBuf {
    home_dir().join(".agent-universe")
}

/// 用户可见的默认工作区
pub fn default_workspace_dir() -> PathBuf {
    app_data_dir().join("workspaces").join("default")
}

/// daemon 数据目录（账本、SQLite、沙箱）
pub fn daemon_data_dir() -> PathBuf {
    app_data_dir().join("daemon-data")
}

pub fn config_file() -> PathBuf {
    app_data_dir().join("config.json")
}

/// 确保默认工作区存在并返回其路径（含子目录）。
pub fn ensure_default_workspace() -> std::io::Result<PathBuf> {
    let ws = default_workspace_dir();
    fs::create_dir_all(ws.join("agents"))?;
    fs::create_dir_all(ws.join("tasks"))?;
    fs::create_dir_all(ws.join("files"))?;
    fs::create_dir_all(ws.join("output"))?;
    Ok(ws)
}

/// 模型提供商
#[derive(Serialize, Deserialize, Clone)]
pub struct ProviderConfig {
    pub id: String,
    pub name: String,
    /// openai | anthropic | deepseek | ollama | custom
    pub kind: String,
    pub base_url: String,
    /// 本地保存的 API Key（桌面单机；可留空表示通过环境变量/稍后配置）
    pub api_key: String,
    pub model: String,
    pub enabled: bool,
}

/// 应用配置
#[derive(Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub version: String,
    pub api_port: u16,
    /// 过程展示分级：results | steps | full
    pub process_visibility: String,
    pub run_in_background: bool,
    pub active_provider: String,
    pub providers: Vec<ProviderConfig>,
    /// 实验功能 / 插件开关（插件管理页持久化）
    #[serde(default)]
    pub feature_flags: std::collections::BTreeMap<String, bool>,
    pub onboarding_done: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            version: "2.9.0".to_string(),
            api_port: 4002,
            process_visibility: "steps".to_string(),
            run_in_background: true,
            active_provider: "deepseek".to_string(),
            providers: vec![
                ProviderConfig {
                    id: "deepseek".to_string(),
                    name: "DeepSeek".to_string(),
                    kind: "deepseek".to_string(),
                    base_url: "https://api.deepseek.com".to_string(),
                    api_key: String::new(),
                    model: "deepseek-chat".to_string(),
                    enabled: true,
                },
                ProviderConfig {
                    id: "openai".to_string(),
                    name: "OpenAI".to_string(),
                    kind: "openai".to_string(),
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: String::new(),
                    model: "gpt-4o".to_string(),
                    enabled: false,
                },
                ProviderConfig {
                    id: "anthropic".to_string(),
                    name: "Anthropic".to_string(),
                    kind: "anthropic".to_string(),
                    base_url: "https://api.anthropic.com".to_string(),
                    api_key: String::new(),
                    model: "claude-sonnet-4-5".to_string(),
                    enabled: false,
                },
                ProviderConfig {
                    id: "ollama".to_string(),
                    name: "Ollama (本地)".to_string(),
                    kind: "ollama".to_string(),
                    base_url: "http://127.0.0.1:11434".to_string(),
                    api_key: String::new(),
                    model: "qwen2.5".to_string(),
                    enabled: false,
                },
            ],
            feature_flags: std::collections::BTreeMap::new(),
            onboarding_done: false,
        }
    }
}

pub fn load_config() -> AppConfig {
    if let Ok(text) = fs::read_to_string(config_file()) {
        if let Ok(cfg) = serde_json::from_str::<AppConfig>(&text) {
            return cfg;
        }
    }
    AppConfig::default()
}

pub fn save_config(cfg: &AppConfig) -> std::io::Result<()> {
    fs::create_dir_all(app_data_dir())?;
    let text = serde_json::to_string_pretty(cfg)
        .map_err(std::io::Error::other)?;
    fs::write(config_file(), text)
}
