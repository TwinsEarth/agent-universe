//! MCP 资源定义
//!
//! 兼容 MCP Resources 规范：资源是 Agent 可读取的数据

use serde::{Deserialize, Serialize};

/// 资源 URI
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceUri(String);

impl ResourceUri {
    pub fn new(uri: impl Into<String>) -> Self {
        Self(uri.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 解析 GSN 资源 URI 格式：gsn://<did>/<resource_type>/<id>
    pub fn parse_gsn(uri: &str) -> Option<(String, String, String)> {
        let rest = uri.strip_prefix("gsn://")?;
        let parts: Vec<&str> = rest.splitn(3, '/').collect();
        if parts.len() == 3 {
            Some((parts[0].to_string(), parts[1].to_string(), parts[2].to_string()))
        } else {
            None
        }
    }
}

impl std::fmt::Display for ResourceUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 资源定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDefinition {
    pub uri: ResourceUri,
    pub name: String,
    pub description: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    /// GSN 扩展：资源内容哈希（CID）
    #[serde(default)]
    pub gsn_cid: Option<String>,
    /// GSN 扩展：是否需要访问控制
    #[serde(default)]
    pub gsn_restricted: bool,
}

impl ResourceDefinition {
    pub fn new(uri: ResourceUri, name: String, mime_type: String) -> Self {
        Self {
            uri,
            name,
            description: String::new(),
            mime_type,
            gsn_cid: None,
            gsn_restricted: false,
        }
    }

    pub fn with_description(mut self, desc: String) -> Self {
        self.description = desc;
        self
    }

    pub fn with_cid(mut self, cid: String) -> Self {
        self.gsn_cid = Some(cid);
        self
    }

    pub fn restricted(mut self) -> Self {
        self.gsn_restricted = true;
        self
    }
}

/// 资源内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContents {
    pub uri: ResourceUri,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
}

impl ResourceContents {
    pub fn text(uri: ResourceUri, mime_type: String, text: String) -> Self {
        Self {
            uri,
            mime_type,
            text: Some(text),
            blob: None,
        }
    }
}
