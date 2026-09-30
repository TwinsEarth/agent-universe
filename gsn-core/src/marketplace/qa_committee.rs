//! BFT-lite QA 委员会
//!
//! n ≥ 3f+1，法定人数 q = 2f+1
//! 同轮 equivocation 整轮作废
//! 沉默 > f → NO_QUORUM + view_change

use crate::identity::signer::Ed25519Signer;
use crate::identity::Keypair;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// QA 投票
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QaVote {
    /// 结果合格，停止
    Stop,
    /// 需要继续 / 返工
    Continue,
    /// 未投票（沉默）
    Silent,
}

/// 签名 QA 投票（v2.5.9：认证 + 重放保护）
///
/// 委员用自己的 Ed25519 私钥对「任务 + 轮次 + 投票 + nonce + 时间窗」签名；
/// 服务端只接受固定委员集成员的有效签名票，从而杜绝「调用方合成委员与票」。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedQaVote {
    pub task_id: String,
    pub round: u32,
    pub voter: String,
    pub vote: QaVote,
    /// 一次性随机数，防重放
    pub nonce: String,
    /// 签发时间（unix 秒）
    pub issued_at: u64,
    /// 过期时间（unix 秒）
    pub expires_at: u64,
    /// Ed25519 签名（hex，64 字节）
    pub signature: String,
}

impl SignedQaVote {
    /// 待签名的规范字节：固定字段顺序与标签，不依赖 JSON 键序
    pub fn signing_bytes(&self) -> Vec<u8> {
        let tag = match self.vote {
            QaVote::Stop => "STOP",
            QaVote::Continue => "CONTINUE",
            QaVote::Silent => "SILENT",
        };
        format!(
            "AU-QA-VOTE\ntask_id={}\nround={}\nvoter={}\nvote={}\nnonce={}\nissued_at={}\nexpires_at={}",
            self.task_id,
            self.round,
            self.voter,
            tag,
            self.nonce,
            self.issued_at,
            self.expires_at
        )
        .into_bytes()
    }

    /// 由委员密钥对签发一张投票
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        task_id: &str,
        round: u32,
        voter: &str,
        vote: QaVote,
        nonce: &str,
        issued_at: u64,
        expires_at: u64,
        keypair: &Keypair,
    ) -> Self {
        let mut sv = Self {
            task_id: task_id.to_string(),
            round,
            voter: voter.to_string(),
            vote,
            nonce: nonce.to_string(),
            issued_at,
            expires_at,
            signature: String::new(),
        };
        let sig = Ed25519Signer::new(keypair).sign(&sv.signing_bytes());
        sv.signature = hex::encode(sig);
        sv
    }
}

/// QA 委员
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QaMember {
    pub did: String,
    pub vote: QaVote,
    /// 是否在本轮投了多个不同票（equivocation）
    pub equivocated: bool,
}

/// QA 投票结果
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QaDecision {
    /// 通过（STOP 票 ≥ q）
    Stop,
    /// 不通过（CONTINUE 票 ≥ q）
    Continue,
    /// 无共识（沉默 > f 或 equivocation）
    NoQuorum,
}

/// BFT-lite QA 委员会
#[derive(Debug, Clone)]
pub struct QaCommittee {
    members: Vec<QaMember>,
    n: u32,
    f: u32,
    /// 固定委员公钥（did → 32 字节公钥）；非空表示认证委员会
    pubkeys: HashMap<String, [u8; 32]>,
    /// 已消费 nonce（防重放）
    seen_nonces: HashSet<String>,
    /// 委员会绑定的任务
    bound_task: Option<String>,
    /// 当前视图 / 轮次
    round: u32,
}

impl QaCommittee {
    /// 创建委员会
    ///
    /// 要求 n ≥ 3f+1
    pub fn new(n: u32, f: u32) -> Result<Self, String> {
        // checked 运算：f 极大时 debug panic / release 回绕会构造出护栏失效的荒谬委员会（GAP §3.3）
        let min_n = f
            .checked_mul(3)
            .and_then(|x| x.checked_add(1))
            .ok_or_else(|| format!("f={f} 过大：3f+1 溢出 u32，拒绝构造委员会"))?;
        if n < min_n {
            return Err(format!(
                "BFT-lite 要求 n ≥ 3f+1，当前 n={}, f={}, 需要 n≥{min_n}",
                n, f
            ));
        }
        Ok(Self {
            members: Vec::new(),
            n,
            f,
            pubkeys: HashMap::new(),
            seen_nonces: HashSet::new(),
            bound_task: None,
            round: 0,
        })
    }

    /// 用固定成员集（did + 公钥）创建**认证委员会**（v2.5.9）
    ///
    /// 委员集在创建时固定，f = (n−1)/3。调用方可以指定「谁来当评委」，
    /// 但无法伪造评委的票——票必须由持有对应私钥的委员签名。
    pub fn with_fixed_members(
        task_id: &str,
        round: u32,
        members: Vec<(String, [u8; 32])>,
    ) -> Result<Self, String> {
        if members.is_empty() {
            return Err("固定委员集不能为空".to_string());
        }
        let n = members.len() as u32;
        let f = (n - 1) / 3;
        let mut committee = Self::new(n, f)?;
        committee.bound_task = Some(task_id.to_string());
        committee.round = round;
        for (did, pk) in members {
            committee.pubkeys.insert(did.clone(), pk);
            committee.add_member(did);
        }
        Ok(committee)
    }

    /// 添加委员
    pub fn add_member(&mut self, did: String) {
        if self.members.len() < self.n as usize {
            self.members.push(QaMember {
                did,
                vote: QaVote::Silent,
                equivocated: false,
            });
        }
    }

    /// 记录投票
    pub fn cast_vote(&mut self, did: &str, vote: QaVote) -> Result<(), String> {
        let member = self
            .members
            .iter_mut()
            .find(|m| m.did == did)
            .ok_or_else(|| format!("NOT_FOUND: 委员 {} 不存在", did))?;

        // 检测 equivocation：已投过票且新票不同
        if member.vote != QaVote::Silent && member.vote != vote {
            member.equivocated = true;
            return Ok(());
        }
        member.vote = vote;
        Ok(())
    }

    /// 接收一张签名投票（v2.5.9：认证 + 重放保护）
    ///
    /// 依次校验：委员身份 → 任务绑定 → 轮次绑定 → 时间窗 → 签名 →
    /// nonce 重放 → 记录投票（含 equivocation 检测）。
    pub fn cast_signed_vote(&mut self, sv: SignedQaVote, now: u64) -> Result<(), String> {
        // 1. 投票者必须在固定委员集
        let pk = *self
            .pubkeys
            .get(&sv.voter)
            .ok_or_else(|| format!("投票者 {} 不在固定委员集", sv.voter))?;

        // 2. 任务必须与委员会绑定任务一致
        match &self.bound_task {
            Some(t) if t == &sv.task_id => {}
            _ => return Err(format!("投票任务 {} 与委员会绑定任务不符", sv.task_id)),
        }

        // 3. 轮次必须匹配
        if sv.round != self.round {
            return Err(format!(
                "投票轮次 {} 与当前轮次 {} 不符",
                sv.round, self.round
            ));
        }

        // 4. 时间窗
        if sv.expires_at <= sv.issued_at {
            return Err("投票有效期非法：expires_at 必须晚于 issued_at".to_string());
        }
        if now < sv.issued_at {
            return Err("投票尚未生效（issued_at 在未来）".to_string());
        }
        if now > sv.expires_at {
            return Err("投票已过期".to_string());
        }

        // 5. 不允许签 Silent
        if sv.vote == QaVote::Silent {
            return Err("签名投票不能为 Silent".to_string());
        }

        // 6. 验签（先验签，避免无效票污染 nonce 集合）
        let sig = hex::decode(&sv.signature).map_err(|e| format!("签名 hex 解码失败: {e}"))?;
        if !Ed25519Signer::verify_with_pubkey(&pk, &sv.signing_bytes(), &sig) {
            return Err("投票签名验证失败".to_string());
        }

        // 7. nonce 防重放
        if sv.nonce.is_empty() {
            return Err("投票缺少 nonce".to_string());
        }
        if !self.seen_nonces.insert(sv.nonce.clone()) {
            return Err(format!("检测到重放：nonce {} 已被使用", sv.nonce));
        }

        // 8. 记录投票（cast_vote 内含 equivocation 检测）
        self.cast_vote(&sv.voter, sv.vote)
    }

    /// 统计决策
    pub fn tally(&self) -> QaDecision {
        // 如果有 equivocation，整轮作废
        if self.members.iter().any(|m| m.equivocated) {
            return QaDecision::NoQuorum;
        }

        let stop_count = self
            .members
            .iter()
            .filter(|m| m.vote == QaVote::Stop)
            .count() as u32;
        let continue_count = self
            .members
            .iter()
            .filter(|m| m.vote == QaVote::Continue)
            .count() as u32;
        let silent_count = self
            .members
            .iter()
            .filter(|m| m.vote == QaVote::Silent)
            .count() as u32;

        let quorum = self.quorum();

        // 沉默 > f → 无共识
        if silent_count > self.f {
            return QaDecision::NoQuorum;
        }

        if stop_count >= quorum {
            QaDecision::Stop
        } else if continue_count >= quorum {
            QaDecision::Continue
        } else {
            QaDecision::NoQuorum
        }
    }

    /// 重置投票（view change 后）
    pub fn reset_votes(&mut self) {
        for member in &mut self.members {
            member.vote = QaVote::Silent;
            member.equivocated = false;
        }
    }

    /// 进入下一视图（view change）：清票并递增轮次
    pub fn advance_view(&mut self) {
        self.reset_votes();
        self.round += 1;
    }

    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    pub fn n(&self) -> u32 {
        self.n
    }

    pub fn f(&self) -> u32 {
        self.f
    }

    pub fn quorum(&self) -> u32 {
        // new 已校验 3f+1 不溢出，故 2f+1 必不溢出；checked 仅作防御，溢出时安全降级为 MAX（不可达法定人数）
        self.f
            .checked_mul(2)
            .and_then(|x| x.checked_add(1))
            .unwrap_or(u32::MAX)
    }

    /// 当前轮次 / 视图
    pub fn round(&self) -> u32 {
        self.round
    }

    /// 是否为认证委员会（含固定委员公钥）
    pub fn is_authenticated(&self) -> bool {
        !self.pubkeys.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造 4 委员（f=1，quorum=3），返回委员会与各委员 (did, keypair)
    fn committee4() -> (QaCommittee, Vec<(String, Keypair)>) {
        let mut mset: Vec<(String, [u8; 32])> = Vec::new();
        let mut kps: Vec<(String, Keypair)> = Vec::new();
        for i in 1u8..=4 {
            let mut s = [0u8; 32];
            s[0] = i;
            let kp = Keypair::from_seed(&s);
            let pk: [u8; 32] = kp.public_key().try_into().unwrap();
            let did = format!("did:nau:m{}", i);
            mset.push((did.clone(), pk));
            kps.push((did, kp));
        }
        let c = QaCommittee::with_fixed_members("task-1", 0, mset).unwrap();
        (c, kps)
    }

    #[test]
    fn three_signed_stop_votes_accept() {
        let (mut c, kps) = committee4();
        let now = 1_000_000u64;
        for (i, (did, kp)) in kps.iter().take(3).enumerate() {
            let sv = SignedQaVote::sign(
                "task-1",
                0,
                did,
                QaVote::Stop,
                &format!("nonce-{}", i),
                now - 10,
                now + 60,
                kp,
            );
            c.cast_signed_vote(sv, now).unwrap();
        }
        assert_eq!(c.tally(), QaDecision::Stop);
    }

    #[test]
    fn forged_signature_rejected() {
        let (mut c, kps) = committee4();
        let now = 1_000_000u64;
        let (did, kp) = &kps[0];
        let mut sv =
            SignedQaVote::sign("task-1", 0, did, QaVote::Stop, "n1", now - 10, now + 60, kp);
        sv.signature = "00".repeat(64); // 伪造签名
        assert!(c.cast_signed_vote(sv, now).is_err());
    }

    #[test]
    fn replay_rejected_by_nonce() {
        let (mut c, kps) = committee4();
        let now = 1_000_000u64;
        let sv = SignedQaVote::sign(
            "task-1",
            0,
            &kps[0].0,
            QaVote::Stop,
            "same-nonce",
            now - 10,
            now + 60,
            &kps[0].1,
        );
        c.cast_signed_vote(sv.clone(), now).unwrap();
        let err = c.cast_signed_vote(sv, now).unwrap_err();
        assert!(err.contains("重放"), "实际错误: {err}");
    }

    #[test]
    fn expired_vote_rejected() {
        let (mut c, kps) = committee4();
        let now = 1_000_000u64;
        let sv = SignedQaVote::sign(
            "task-1",
            0,
            &kps[0].0,
            QaVote::Stop,
            "n",
            now - 100,
            now - 10,
            &kps[0].1,
        );
        assert!(c.cast_signed_vote(sv, now).unwrap_err().contains("过期"));
    }

    #[test]
    fn wrong_task_rejected() {
        let (mut c, kps) = committee4();
        let now = 1_000_000u64;
        let sv = SignedQaVote::sign(
            "other-task",
            0,
            &kps[0].0,
            QaVote::Stop,
            "n",
            now - 10,
            now + 60,
            &kps[0].1,
        );
        assert!(c.cast_signed_vote(sv, now).is_err());
    }

    #[test]
    fn non_member_vote_rejected() {
        let (mut c, _kps) = committee4();
        let now = 1_000_000u64;
        let mut s = [0u8; 32];
        s[0] = 99;
        let kp = Keypair::from_seed(&s);
        let sv = SignedQaVote::sign(
            "task-1",
            0,
            "did:nau:stranger",
            QaVote::Stop,
            "n",
            now - 10,
            now + 60,
            &kp,
        );
        assert!(c.cast_signed_vote(sv, now).is_err());
    }

    #[test]
    fn equivocation_invalidates_then_view_change_recovers() {
        let (mut c, kps) = committee4();
        let now = 1_000_000u64;
        let s1 = SignedQaVote::sign(
            "task-1",
            0,
            &kps[0].0,
            QaVote::Stop,
            "n1",
            now - 10,
            now + 60,
            &kps[0].1,
        );
        let s2 = SignedQaVote::sign(
            "task-1",
            0,
            &kps[0].0,
            QaVote::Continue,
            "n2",
            now - 10,
            now + 60,
            &kps[0].1,
        );
        c.cast_signed_vote(s1, now).unwrap();
        c.cast_signed_vote(s2, now).unwrap();
        assert_eq!(c.tally(), QaDecision::NoQuorum);

        // view change 后进入新一轮，可重新投票达成 Stop
        c.advance_view();
        assert_eq!(c.round(), 1);
        for (i, (did, kp)) in kps.iter().take(3).enumerate() {
            let sv = SignedQaVote::sign(
                "task-1",
                1,
                did,
                QaVote::Stop,
                &format!("nn-{}", i),
                now - 10,
                now + 60,
                kp,
            );
            c.cast_signed_vote(sv, now).unwrap();
        }
        assert_eq!(c.tally(), QaDecision::Stop);
    }
}
