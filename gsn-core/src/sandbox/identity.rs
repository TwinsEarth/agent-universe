//! Agent Sandbox 身份（v2.7.6）
//!
//! 关键安全边界：
//! - 沙箱有自己的临时身份（SandboxIdentity），仅在本沙箱生命周期内有效；
//! - Agent 长期身份（AgentIdentity，持长期密钥）不进入沙箱；
//! - 沙箱内如需代表 Agent 调用，必须经"入站认证 + 短时令牌"，由宿主侧代理签发，
//!   令牌最小权限、有 TTL、可吊销；
//! - 沙箱销毁后，其临时身份与令牌全部失效。

use serde::{Deserialize, Serialize};

/// 沙箱临时身份（仅本沙箱生命周期有效）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxIdentity {
    /// 沙箱 ID
    pub sandbox_id: String,
    /// 沙箱实例序号（同一逻辑沙箱可能 fork 多个实例）
    pub instance: u32,
    /// 宿主节点 ID
    pub host_id: String,
    /// 创建时间戳（毫秒）
    pub created_ms: u64,
}

/// Agent 身份（长期；密钥不进沙箱）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentIdentity {
    /// Agent 的 DID
    pub agent_did: String,
    /// Agent 名称（审计可读）
    pub agent_name: String,
    /// Agent 所属租户/组织
    pub tenant: String,
}

/// 短时执行令牌（沙箱内代表 Agent 调用时用，最小权限 + TTL）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionToken {
    /// 令牌 ID（可吊销）
    pub token_id: String,
    /// 绑定的沙箱 ID
    pub sandbox_id: String,
    /// 代表的 Agent DID
    pub agent_did: String,
    /// 允许的能力范围（scope，最小权限）
    pub scopes: Vec<String>,
    /// 签发时间（毫秒）
    pub issued_ms: u64,
    /// 过期时间（毫秒）
    pub expires_ms: u64,
}

impl ExecutionToken {
    /// 是否过期
    pub fn is_expired(&self, now_ms: u64) -> bool {
        now_ms >= self.expires_ms
    }

    /// 是否拥有某项能力
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }

    /// 校验令牌是否绑定该沙箱、未过期、具备所需能力
    pub fn authorize(&self, sandbox_id: &str, scope: &str, now_ms: u64) -> Result<(), String> {
        if self.sandbox_id != sandbox_id {
            return Err(format!(
                "令牌绑定沙箱 {} 与当前 {} 不符",
                self.sandbox_id, sandbox_id
            ));
        }
        if self.is_expired(now_ms) {
            return Err("执行令牌已过期".to_string());
        }
        if !self.has_scope(scope) {
            return Err(format!("令牌缺少能力 scope: {scope}"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_authorize_ok() {
        let t = ExecutionToken {
            token_id: "tok1".into(),
            sandbox_id: "sb1".into(),
            agent_did: "did:aip:agent".into(),
            scopes: vec!["code.exec".into(), "fs.read".into()],
            issued_ms: 1000,
            expires_ms: 5000,
        };
        assert!(t.authorize("sb1", "code.exec", 2000).is_ok());
    }

    #[test]
    fn token_wrong_sandbox_denied() {
        let t = ExecutionToken {
            token_id: "t".into(),
            sandbox_id: "sb1".into(),
            agent_did: "d".into(),
            scopes: vec!["x".into()],
            issued_ms: 0,
            expires_ms: 100,
        };
        assert!(t.authorize("sb2", "x", 0).is_err());
    }

    #[test]
    fn token_expired_denied() {
        let t = ExecutionToken {
            token_id: "t".into(),
            sandbox_id: "sb1".into(),
            agent_did: "d".into(),
            scopes: vec!["x".into()],
            issued_ms: 0,
            expires_ms: 100,
        };
        assert!(t.authorize("sb1", "x", 100).is_err());
    }

    #[test]
    fn token_missing_scope_denied() {
        let t = ExecutionToken {
            token_id: "t".into(),
            sandbox_id: "sb1".into(),
            agent_did: "d".into(),
            scopes: vec!["a".into()],
            issued_ms: 0,
            expires_ms: 100,
        };
        assert!(t.authorize("sb1", "b", 0).is_err());
    }
}
