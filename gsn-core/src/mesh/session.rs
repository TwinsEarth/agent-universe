//! 临时会话识别码（SN）分配 + 永久 DID 绑定

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 永久 DID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PermanentDid(pub String);

impl PermanentDid {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 临时会话 ID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId {
    /// 会话序号（自增）
    pub number: u64,
    /// 会话唯一码（随机生成）
    pub code: String,
}

impl SessionId {
    pub fn display(&self) -> String {
        format!("SN-{:06}-{}", self.number, self.code)
    }
}

/// 会话注册表：管理临时 SN ↔ 永久 DID 映射
pub struct SessionRegistry {
    /// 下一个会话序号
    next_number: u64,
    /// 当前活跃会话：session_id → permanent_did
    active: HashMap<String, PermanentDid>,
    /// 历史绑定记录：permanent_did → 累计会话数
    history: HashMap<String, u32>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self {
            next_number: 1,
            active: HashMap::new(),
            history: HashMap::new(),
        }
    }

    /// 分配一个新的临时 SN（peer 首次上线）
    pub fn allocate_session(&mut self, code: &str) -> SessionId {
        let number = self.next_number;
        self.next_number += 1;
        SessionId { number, code: code.to_string() }
    }

    /// 将临时 SN 绑定到永久 DID
    pub fn bind(&mut self, session: &SessionId, did: PermanentDid) -> Result<(), String> {
        let key = session.display();
        if self.active.contains_key(&key) {
            return Err(format!("会话 {} 已绑定", key));
        }
        self.active.insert(key, did.clone());
        let count = self.history.entry(did.0).or_insert(0);
        *count += 1;
        Ok(())
    }

    /// 解除绑定（会话结束）
    pub fn unbind(&mut self, session: &SessionId) -> bool {
        self.active.remove(&session.display()).is_some()
    }

    /// 查询某会话绑定的 DID
    pub fn did_for(&self, session: &SessionId) -> Option<&PermanentDid> {
        self.active.get(&session.display())
    }

    /// 查询某永久 DID 当前是否在线（有活跃会话）
    pub fn is_online(&self, did: &PermanentDid) -> bool {
        self.active.values().any(|d| d == did)
    }

    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    /// 某永久 DID 的历史会话次数
    pub fn session_count_for(&self, did: &str) -> u32 {
        self.history.get(did).copied().unwrap_or(0)
    }

    /// 所有活跃会话
    pub fn active_sessions(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.active.keys().cloned().collect();
        keys.sort();
        keys
    }
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocate_increments_number() {
        let mut r = SessionRegistry::new();
        let s1 = r.allocate_session("abc");
        let s2 = r.allocate_session("def");
        assert_eq!(s1.number, 1);
        assert_eq!(s2.number, 2);
        assert_eq!(s1.display(), "SN-000001-abc");
    }

    #[test]
    fn bind_sn_to_did() {
        let mut r = SessionRegistry::new();
        let s = r.allocate_session("abc");
        let did = PermanentDid("did:gsn:mac-mini".into());
        r.bind(&s, did.clone()).unwrap();

        assert_eq!(r.did_for(&s), Some(&did));
        assert!(r.is_online(&did));
        assert_eq!(r.active_count(), 1);
    }

    #[test]
    fn cannot_double_bind_same_session() {
        let mut r = SessionRegistry::new();
        let s = r.allocate_session("abc");
        r.bind(&s, PermanentDid("did:1".into())).unwrap();
        let err = r.bind(&s, PermanentDid("did:2".into()));
        assert!(err.is_err());
    }

    #[test]
    fn unbind_ends_session() {
        let mut r = SessionRegistry::new();
        let s = r.allocate_session("abc");
        let did = PermanentDid("did:1".into());
        r.bind(&s, did.clone()).unwrap();
        assert!(r.is_online(&did));
        assert!(r.unbind(&s));
        assert!(!r.is_online(&did));
        assert_eq!(r.active_count(), 0);
    }

    #[test]
    fn same_did_multiple_sessions_counted() {
        let mut r = SessionRegistry::new();
        let did = PermanentDid("did:mac".into());

        let s1 = r.allocate_session("aaa");
        r.bind(&s1, did.clone()).unwrap();
        r.unbind(&s1);

        let s2 = r.allocate_session("bbb");
        r.bind(&s2, did.clone()).unwrap();

        assert_eq!(r.session_count_for("did:mac"), 2);
    }

    #[test]
    fn three_machines_get_unique_sns() {
        let mut r = SessionRegistry::new();
        let mac = r.allocate_session("mac01");
        let linux = r.allocate_session("lin02");
        let win = r.allocate_session("win03");

        r.bind(&mac, PermanentDid("did:mac".into())).unwrap();
        r.bind(&linux, PermanentDid("did:linux".into())).unwrap();
        r.bind(&win, PermanentDid("did:win".into())).unwrap();

        assert_eq!(r.active_count(), 3);
        assert_ne!(mac.display(), linux.display());
        assert_ne!(linux.display(), win.display());
    }
}
