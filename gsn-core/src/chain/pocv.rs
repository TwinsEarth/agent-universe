//! PoCV 可验证计算（P0-3 修复版：签名化证明）
//!
//! # 诚实边界（重要，勿误读）
//!
//! 本证明只保证三件事：
//!
//! 1. **归属**：`prover_did` 的标识部分等于签名公钥的指纹（`Did::fingerprint`），
//!    且签名由对应私钥签发；
//! 2. **未篡改**：对固定规范化字节做 Ed25519 签名，任何字段（含哈希、steps、
//!    nonce、时间窗）被改动即验签失败；
//! 3. **贡献者见到该 input/output**：签名覆盖 `input_hash` / `output_hash`，
//!    且 `verify` 时调用方传入的**实际** `actual_input` / `actual_output` 的
//!    SHA-256 必须与签名中的哈希逐字节一致。
//!
//! 本证明 **不** 证明计算正确性：没有确定性重放、没有 zk/STARK/SNARK。任何拿到
//! 输入的人都能算出同样的哈希并签名。要证明"真的按 steps 步算力且结果正确"，
//! 需要真实 VCS（RISC0 / Halo2 等）并需外部密码学审计。BFT-lite QA 仍是结果
//! 验收的策略门；本证明只是密码学证据门（有/无有效签名证明），不替代 QA。
//!
//! # 需外部审计
//!
//! 本模块全部密码学改动（域标签 [`POCV_DOMAIN_TAG`]、规范化签名字节、时间窗、
//! nonce 防重放、DID↔公钥绑定）需外部审计后才可上线。
//!
//! # 历史背景（P0-3 漏洞）
//!
//! 旧 [`ProofOfComputation`] 只有自报的 `input_hash` / `output_hash`，
//! `PoCVVerifier::verify_proof` 仅把自报 input/output 各算一次 SHA-256 与自报
//! 哈希比对——任意 `(input, output)` 都能自洽通过，证明可伪造。现已新增
//! [`SignedProofOfComputation`]（Ed25519 签名 + DID 绑定 + 重放保护）。
//! 旧结构保留仅为向后兼容，**不得再作为信任根使用**。

use crate::identity::signer::Ed25519Signer;
use crate::identity::{is_weak_pubkey, Did, Keypair};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

/// 签名域标签（domain separation，规范化首行）。
///
/// 与 QA 投票（`AU-QA-VOTE`）、治理命令（`AU-GOV-CMD/v1`）互不串域：
/// 同一私钥在别的信封体系里签的字节串，不能被当作 PoCV 证明接受。
pub const POCV_DOMAIN_TAG: &str = "AU-POCV/v1";

/// 默认签名有效期（秒）：签发后 10 分钟内可被接受。
///
/// 证明本身带 `issued_at` / `expires_at`；调用方可在签发时覆盖。
pub const POCV_DEFAULT_TTL_SECS: u64 = 600;

/// 对字节求 SHA-256，返回 32 字节摘要。
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// 带贡献者签名的计算证明（P0-3 修复后的信任根）。
///
/// 序列化友好：`prover_pubkey` / `signature` 为 hex 字符串；
/// `input_hash` / `output_hash` 为原始 32 字节（JSON 里是 32 元素数组，
/// 仅用于信封落盘，不跨语言手编）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedProofOfComputation {
    /// 贡献者 DID（did:nau:<fp> 或 did:aip:<fp>）
    pub prover_did: String,
    /// 签名公钥 hex（32 字节 Ed25519 验证密钥）
    pub prover_pubkey: String,
    /// 声称的输入哈希（SHA-256）
    pub input_hash: [u8; 32],
    /// 声称的输出哈希（SHA-256）
    pub output_hash: [u8; 32],
    /// 声称的计算步数（仅留痕，不被验证为真实）
    pub steps: u64,
    /// 一次性随机数，防重放
    pub nonce: String,
    /// 签发时间（unix 秒）
    pub issued_at: u64,
    /// 过期时间（unix 秒，必须 > issued_at）
    pub expires_at: u64,
    /// Ed25519 签名（hex，64 字节）
    pub signature: String,
}

impl SignedProofOfComputation {
    /// 规范化待签名字节：域标签首行，随后固定字段顺序，不依赖 JSON 键序。
    ///
    /// ```text
    /// AU-POCV/v1
    /// <prover_did>
    /// <hex(input_hash)>
    /// <hex(output_hash)>
    /// <steps>
    /// <nonce>
    /// <issued_at>
    /// <expires_at>
    /// ```
    pub fn signing_bytes(&self) -> Vec<u8> {
        format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
            POCV_DOMAIN_TAG,
            self.prover_did,
            hex::encode(self.input_hash),
            hex::encode(self.output_hash),
            self.steps,
            self.nonce,
            self.issued_at,
            self.expires_at,
        )
        .into_bytes()
    }

    /// 贡献者用自己的密钥对签发一份证明。
    ///
    /// `input` / `output` 只用于计算声称哈希；调用方负责保证二者确实是被执行计算的
    /// 输入与产出。`issued_at` 由调用方（通常取服务端接受时的时钟口径）传入，
    /// `expires_at = issued_at + ttl_secs`。
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        prover_did: &str,
        keypair: &Keypair,
        input: &[u8],
        output: &[u8],
        steps: u64,
        nonce: &str,
        issued_at: u64,
        ttl_secs: u64,
    ) -> Self {
        let mut proof = Self {
            prover_did: prover_did.to_string(),
            prover_pubkey: hex::encode(keypair.public_key()),
            input_hash: sha256(input),
            output_hash: sha256(output),
            steps,
            nonce: nonce.to_string(),
            issued_at,
            expires_at: issued_at.saturating_add(ttl_secs),
            signature: String::new(),
        };
        let sig = Ed25519Signer::new(keypair).sign(&proof.signing_bytes());
        proof.signature = hex::encode(sig);
        proof
    }

    /// 解析签名公钥为 32 字节。
    fn pubkey_bytes(&self) -> Result<[u8; 32], String> {
        let raw = hex::decode(&self.prover_pubkey)
            .map_err(|e| format!("prover_pubkey hex 解码失败: {e}"))?;
        raw.as_slice()
            .try_into()
            .map_err(|_| format!("prover_pubkey 必须为 32 字节，实际 {} 字节", raw.len()))
    }

    /// 验证签名证明。
    ///
    /// 校验顺序（与 QA 投票一致：先验身份与签名，最后才消费 nonce，避免无效证明污染
    /// 重放集合）：
    ///
    /// 1. DID 可解析且其标识部分 == `Did::fingerprint(prover_pubkey)`；公钥非弱；
    /// 2. 时间窗：`expires_at > issued_at`、`now >= issued_at`（非未来）、`now <= expires_at`；
    /// 3. hex 解码签名并用 `prover_pubkey` 验 `signing_bytes`；
    /// 4. `nonce` 非空且 `seen_nonces.insert(nonce)` 成功（防重放）；
    /// 5. 实际产出一致性：`SHA256(actual_input) == input_hash` 且
    ///    `SHA256(actual_output) == output_hash`。
    ///
    /// `seen_nonces` 由调用方维护（进程内跨请求存活；重启后的重放窗口由时间窗约束）。
    ///
    /// # 需外部审计
    /// 该校验链为密码学信任根，上线前需外部审计。
    pub fn verify(
        &self,
        actual_input: &[u8],
        actual_output: &[u8],
        seen_nonces: &mut HashSet<String>,
        now: u64,
    ) -> Result<(), String> {
        // 1. DID ↔ 公钥绑定 + 弱公钥拒绝
        let did = Did::parse(&self.prover_did)?;
        let pk = self.pubkey_bytes()?;
        if is_weak_pubkey(&pk) {
            return Err("POCV_WEAK_PUBKEY: 签名公钥为退化（全零/全相同）公钥，拒绝".to_string());
        }
        if did.identifier() != Did::fingerprint(&pk) {
            return Err(format!(
                "POCV_DID_MISMATCH: prover_did 标识 {} 与公钥指纹 {} 不一致",
                did.identifier(),
                Did::fingerprint(&pk)
            ));
        }

        // 2. 时间窗
        if self.expires_at <= self.issued_at {
            return Err("POCV_BAD_WINDOW: expires_at 必须晚于 issued_at".to_string());
        }
        if now < self.issued_at {
            return Err("POCV_NOT_YET_VALID: 证明尚未生效（issued_at 在未来）".to_string());
        }
        if now > self.expires_at {
            return Err("POCV_EXPIRED: 证明已过期".to_string());
        }

        // 3. 验签（先验签，避免无效证明污染 nonce 集合）
        let sig = hex::decode(&self.signature).map_err(|e| format!("签名 hex 解码失败: {e}"))?;
        if !Ed25519Signer::verify_with_pubkey(&pk, &self.signing_bytes(), &sig) {
            return Err("POCV_BAD_SIGNATURE: 签名验证失败".to_string());
        }

        // 4. nonce 防重放
        if self.nonce.is_empty() {
            return Err("POCV_EMPTY_NONCE: 证明缺少 nonce".to_string());
        }
        if !seen_nonces.insert(self.nonce.clone()) {
            return Err(format!(
                "POCV_REPLAY: nonce {} 已被消费过（重放）",
                self.nonce
            ));
        }

        // 5. 实际产出一致性（claimed 哈希必须匹配调用方传入的真实字节）
        if sha256(actual_input) != self.input_hash {
            return Err(
                "POCV_INPUT_MISMATCH: 实际输入 SHA-256 与证明 input_hash 不一致".to_string()
            );
        }
        if sha256(actual_output) != self.output_hash {
            return Err(
                "POCV_OUTPUT_MISMATCH: 实际输出 SHA-256 与证明 output_hash 不一致".to_string()
            );
        }

        Ok(())
    }
}

// ── 旧结构（保留向后兼容；P0-3 后不得再作为信任根）─────────────────────────

/// 【已弃用，可伪造】无签名计算证明。
///
/// 仅保留 API 兼容。任何安全判定都必须改用 [`SignedProofOfComputation`]。
#[derive(Debug, Clone)]
pub struct ProofOfComputation {
    pub input_hash: [u8; 32],
    pub output_hash: [u8; 32],
    pub steps: u64,
    pub prover_did: String,
}

/// 【已弃用】无签名哈希校验器。
pub struct PoCVVerifier;

impl Default for PoCVVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl PoCVVerifier {
    pub fn new() -> Self {
        Self
    }

    pub fn compute_hash(&self, data: &[u8]) -> [u8; 32] {
        sha256(data)
    }

    pub fn verify_hash(&self, data: &[u8], expected: &[u8; 32]) -> bool {
        self.compute_hash(data) == *expected
    }

    /// 【已弃用，可伪造】只比对自报哈希，无签名、无身份。
    pub fn verify_proof(&self, proof: &ProofOfComputation, input: &[u8], output: &[u8]) -> bool {
        let input_match = self.compute_hash(input) == proof.input_hash;
        let output_match = self.compute_hash(output) == proof.output_hash;
        input_match && output_match
    }

    /// 【已弃用】生成无签名证明。
    pub fn generate_proof(
        &self,
        input: &[u8],
        output: &[u8],
        steps: u64,
        prover_did: String,
    ) -> ProofOfComputation {
        ProofOfComputation {
            input_hash: self.compute_hash(input),
            output_hash: self.compute_hash(output),
            steps,
            prover_did,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个 (did, keypair)，did 的指纹与公钥一致（did:nau:<fp>）。
    fn prover(seed: u8) -> (String, Keypair) {
        let mut s = [0u8; 32];
        s[0] = seed;
        let kp = Keypair::from_seed(&s);
        let did = Did::from_public_key(kp.public_key()).to_string();
        (did, kp)
    }

    const NOW: u64 = 1_000_000;

    #[test]
    fn signed_roundtrip_verifies() {
        let (did, kp) = prover(1);
        let input = b"task-42 input";
        let output = b"the result payload";
        let proof = SignedProofOfComputation::sign(&did, &kp, input, output, 1000, "n1", NOW - 10, POCV_DEFAULT_TTL_SECS);
        let mut seen = HashSet::new();
        proof.verify(input, output, &mut seen, NOW).unwrap();
    }

    #[test]
    fn did_must_match_pubkey_fingerprint() {
        let (did, kp) = prover(1);
        let proof = SignedProofOfComputation::sign(&did, &kp, b"i", b"o", 1, "n", NOW - 10, 600);
        // 换一个不匹配的 DID 字符串（指纹不符）
        let mut bad = proof.clone();
        bad.prover_did = "did:nau:deadbeefdeadbeef".to_string();
        let mut seen = HashSet::new();
        assert!(bad.verify(b"i", b"o", &mut seen, NOW).unwrap_err().contains("DID_MISMATCH"));
    }

    #[test]
    fn weak_pubkey_rejected() {
        let (did, kp) = prover(1);
        let mut proof = SignedProofOfComputation::sign(&did, &kp, b"i", b"o", 1, "n", NOW - 10, 600);
        proof.prover_pubkey = hex::encode([0u8; 32]);
        let mut seen = HashSet::new();
        assert!(proof.verify(b"i", b"o", &mut seen, NOW).unwrap_err().contains("WEAK_PUBKEY"));
    }

    #[test]
    fn future_issued_at_rejected() {
        let (did, kp) = prover(1);
        let proof = SignedProofOfComputation::sign(&did, &kp, b"i", b"o", 1, "n", NOW + 100, 600);
        let mut seen = HashSet::new();
        assert!(proof.verify(b"i", b"o", &mut seen, NOW).unwrap_err().contains("NOT_YET_VALID"));
    }

    #[test]
    fn expired_proof_rejected() {
        let (did, kp) = prover(1);
        let proof = SignedProofOfComputation::sign(&did, &kp, b"i", b"o", 1, "n", NOW - 1000, 60);
        let mut seen = HashSet::new();
        assert!(proof.verify(b"i", b"o", &mut seen, NOW).unwrap_err().contains("EXPIRED"));
    }

    #[test]
    fn forged_signature_rejected() {
        let (did, kp) = prover(1);
        let mut proof = SignedProofOfComputation::sign(&did, &kp, b"i", b"o", 1, "n", NOW - 10, 600);
        proof.signature = "00".repeat(64);
        let mut seen = HashSet::new();
        assert!(proof.verify(b"i", b"o", &mut seen, NOW).unwrap_err().contains("BAD_SIGNATURE"));
    }

    #[test]
    fn replay_nonce_rejected() {
        let (did, kp) = prover(1);
        let proof = SignedProofOfComputation::sign(&did, &kp, b"i", b"o", 1, "once", NOW - 10, 600);
        let mut seen = HashSet::new();
        proof.verify(b"i", b"o", &mut seen, NOW).unwrap();
        assert!(proof.verify(b"i", b"o", &mut seen, NOW).unwrap_err().contains("REPLAY"));
    }

    #[test]
    fn tampered_output_rejected_by_consistency_check() {
        // 关键 P0-3 回归：自报哈希与真实输出不一致时，即使签名是真的也拒绝。
        let (did, kp) = prover(1);
        let proof = SignedProofOfComputation::sign(&did, &kp, b"i", b"honest output", 1, "n", NOW - 10, 600);
        let mut seen = HashSet::new();
        assert!(proof.verify(b"i", b"attacker-changed output", &mut seen, NOW).unwrap_err().contains("OUTPUT_MISMATCH"));
    }

    #[test]
    fn tampered_field_after_signature_breaks_verify() {
        let (did, kp) = prover(1);
        let mut proof = SignedProofOfComputation::sign(&did, &kp, b"i", b"o", 1, "n", NOW - 10, 600);
        proof.steps = 9999; // 篡改 steps → signing_bytes 变化 → 验签失败
        let mut seen = HashSet::new();
        assert!(proof.verify(b"i", b"o", &mut seen, NOW).unwrap_err().contains("BAD_SIGNATURE"));
    }

    #[test]
    fn legacy_api_still_compiles_and_self_consistent() {
        let v = PoCVVerifier::new();
        let p = v.generate_proof(b"i", b"o", 5, "did:nau:x".to_string());
        assert!(v.verify_proof(&p, b"i", b"o"));
        assert!(!v.verify_proof(&p, b"i", b"changed"));
    }
}
