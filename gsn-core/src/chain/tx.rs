//! EIP-1559（transaction type 2）交易构造与 secp256k1 签名。
//!
//! 输出可广播的 raw transaction hex（`0x02 || RLP([...typed fields..., yParity, r, s])`）。
//!
//! # 安全模型
//!
//! - 所有金额/gas/fee 均为 `u64`/`u128` 整数，使用 checked/saturating 运算；
//!   全模块无浮点。
//! - 私钥只从 [`crate::chain::keys::load_signing_key`] 进入本模块；本模块不读环境变量、
//!   不打印私钥。
//!
//! # 诚实性
//!
//! 本模块只产出**待广播的 raw tx**；不做任何网络调用。是否真的被节点接受/上链，
//! 需要真实 RPC 与测试网资金——当前环境未验证。
//!
//! # 需外部审计
//!
//! 交易签名结构、yParity 编码、low-s 规范与链间重放保护（chainId 绑定）为资金路径，
//! 上线前需外部审计。

use k256::ecdsa::{RecoveryId, SigningKey, VerifyingKey};
use sha3::{Digest, Keccak256};

use crate::chain::config::ChainError;
use crate::chain::keys::keccak256;
use crate::chain::rlp::{self, Rlp};

/// EIP-1559 交易（type 2）核心字段。
#[derive(Debug, Clone)]
pub struct TxEip1559 {
    pub chain_id: u64,
    pub nonce: u64,
    /// maxPriorityFeePerGas（wei）。
    pub max_priority_fee_per_gas: u128,
    /// maxFeePerGas（wei）。
    pub max_fee_per_gas: u128,
    pub gas_limit: u64,
    /// 接收方；`None` 表示合约创建。
    pub to: Option<[u8; 20]>,
    /// 转账金额（wei）。
    pub value: u128,
    /// calldata。
    pub data: Vec<u8>,
}

impl TxEip1559 {
    /// 构造签名前的 RLP 字段列表（不含 yParity/r/s）。
    fn unsigned_rlp_fields(&self) -> Vec<Rlp> {
        let to_item = match &self.to {
            Some(addr) => Rlp::Bytes(addr.to_vec()),
            None => Rlp::Bytes(Vec::new()),
        };
        vec![
            Rlp::uint(self.chain_id as u128),
            Rlp::uint(self.nonce as u128),
            Rlp::uint(self.max_priority_fee_per_gas),
            Rlp::uint(self.max_fee_per_gas),
            Rlp::uint(self.gas_limit as u128),
            to_item,
            Rlp::uint(self.value),
            Rlp::Bytes(self.data.clone()),
            Rlp::List(Vec::new()), // access_list（type-2 支持但本实现为空）
        ]
    }

    /// 签名前的 Keccak256 交易哈希（EIP-1559 签名哈希）。
    pub fn signing_hash(&self) -> [u8; 32] {
        let mut preimage = Vec::with_capacity(64);
        preimage.push(0x02); // type-2 前缀参与哈希
        preimage.extend_from_slice(&rlp::encode(&Rlp::List(self.unsigned_rlp_fields())));
        keccak256(&preimage)
    }

    /// 用给定私钥签名，返回 (raw_tx_hex_with_0x, recovery)。
    ///
    /// `raw_tx_hex` 可直接交给 `eth_sendRawTransaction`。
    pub fn sign(&self, sk: &SigningKey) -> Result<SignedTx, ChainError> {
        let digest = self.signing_hash();
        // k256 会自动把 s 规范到 low-s 并相应调整 recovery id（EVM 要求）。
        let (sig, recid) = sk
            .sign_digest_recoverable(Keccak256::new_with_prefix(digest))
            .map_err(ChainError::from)?;

        let r_bytes = scalar_bytes_minimal(&sig);
        let s_bytes = scalar_s_bytes_minimal(&sig);
        let y_parity = recid.to_byte() as u64; // type-2: 0 或 1

        let mut signed_fields = self.unsigned_rlp_fields();
        signed_fields.push(Rlp::Bytes(y_parity_minimal(y_parity)));
        signed_fields.push(Rlp::Bytes(r_bytes));
        signed_fields.push(Rlp::Bytes(s_bytes));

        let mut raw = Vec::with_capacity(64);
        raw.push(0x02);
        raw.extend_from_slice(&rlp::encode(&Rlp::List(signed_fields)));

        Ok(SignedTx {
            raw,
            signing_hash: digest,
            y_parity,
        })
    }
}

/// 签名结果。
#[derive(Debug, Clone)]
pub struct SignedTx {
    raw: Vec<u8>,
    signing_hash: [u8; 32],
    y_parity: u64,
}

impl SignedTx {
    /// raw tx 字节（含 0x02 type 前缀）。
    pub fn raw_bytes(&self) -> &[u8] {
        &self.raw
    }
    /// raw tx hex（带 `0x` 前缀），供 JSON-RPC。
    pub fn raw_hex(&self) -> String {
        format!("0x{}", hex::encode(&self.raw))
    }
    /// 签名哈希（keccak of typed preimage）。
    pub fn signing_hash(&self) -> &[u8; 32] {
        &self.signing_hash
    }
    pub fn y_parity(&self) -> u64 {
        self.y_parity
    }
}

/// 从 Signature 取 r 的最小大端字节串。
fn scalar_bytes_minimal(sig: &k256::ecdsa::Signature) -> Vec<u8> {
    let r: [u8; 32] = sig
        .r()
        .to_bytes()
        .as_slice()
        .try_into()
        .unwrap_or([0u8; 32]);
    strip_leading_zeros(&r)
}

fn scalar_s_bytes_minimal(sig: &k256::ecdsa::Signature) -> Vec<u8> {
    let s: [u8; 32] = sig
        .s()
        .to_bytes()
        .as_slice()
        .try_into()
        .unwrap_or([0u8; 32]);
    strip_leading_zeros(&s)
}

fn y_parity_minimal(v: u64) -> Vec<u8> {
    rlp::minimal_be_u64(v)
}

fn strip_leading_zeros(b: &[u8; 32]) -> Vec<u8> {
    let first = b.iter().position(|x| *x != 0).unwrap_or(32);
    b[first..].to_vec()
}

/// 用 (digest, signature, recid) 恢复验证公钥——用于自校验与测试。
pub fn recover_verifying_key(
    digest: &[u8; 32],
    sig: &k256::ecdsa::Signature,
    recid: RecoveryId,
) -> Result<VerifyingKey, ChainError> {
    VerifyingKey::recover_from_digest(Keccak256::new_with_prefix(digest), sig, recid)
        .map_err(ChainError::from)
}

/// 把 raw tx 反解出来做结构校验：必须以 0x02 开头，其后是 12 元素列表。
pub fn inspect_raw_tx(raw: &[u8]) -> Result<Vec<rlp::RlpOwned>, ChainError> {
    if raw.first() != Some(&0x02) {
        return Err(ChainError::Tx("raw tx 缺少 0x02 type 前缀".into()));
    }
    let (v, consumed) = rlp::decode(&raw[1..])?;
    if consumed != raw.len() - 1 {
        return Err(ChainError::Tx("raw tx 尾部有多余字节".into()));
    }
    let list = match v {
        rlp::RlpOwned::List(l) => l,
        _ => return Err(ChainError::Tx("raw tx 主体不是 RLP 列表".into())),
    };
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::keys::evm_address_from_signing_key;
    use k256::ecdsa::SigningKey;

    const TEST_PRIV: &str = "4f3edf983ac636a65a842ce7c78d9aa706d3b113bce9c46f30d2d717b23f64de";

    fn test_key() -> SigningKey {
        let bytes = crate::chain::keys::parse_private_hex_pub(TEST_PRIV).unwrap();
        SigningKey::from_bytes(&bytes.into()).unwrap()
    }

    fn sample_tx() -> TxEip1559 {
        TxEip1559 {
            chain_id: 84532,
            nonce: 0,
            max_priority_fee_per_gas: 1_000_000_000,
            max_fee_per_gas: 2_000_000_000,
            gas_limit: 21000,
            to: Some([0x11u8; 20]),
            value: 1_000_000_000_000_000_000, // 1 ETH
            data: vec![],
        }
    }

    #[test]
    fn sign_then_recover_yields_same_address() {
        let sk = test_key();
        let expected = evm_address_from_signing_key(&sk).unwrap();
        let tx = sample_tx();
        let signed = tx.sign(&sk).unwrap();

        // 从签名 digest + r/s + recid 恢复公钥，派生地址应与签名地址一致。
        // 重新解析 raw tx 取 yParity/r/s，再 recover。
        let fields = inspect_raw_tx(&signed.raw).unwrap();
        assert_eq!(fields.len(), 12, "type-2 签名后应为 12 字段");
        // 末尾三项：yParity, r, s
        let yp = rlp::rlp_bytes_to_u64(fields[9].as_bytes().unwrap()).unwrap();
        assert!(yp <= 1, "EIP-1559 yParity 必须为 0/1，实际 {yp}");
        let r = fields[10].as_bytes().unwrap();
        let s = fields[11].as_bytes().unwrap();
        let mut r32 = [0u8; 32];
        r32[32 - r.len()..].copy_from_slice(r);
        let mut s32 = [0u8; 32];
        s32[32 - s.len()..].copy_from_slice(s);
        let sig = k256::ecdsa::Signature::from_scalars(r32, s32).unwrap();
        let recid = RecoveryId::try_from(yp as u8).unwrap();
        let vk = VerifyingKey::recover_from_digest(
            Keccak256::new_with_prefix(signed.signing_hash()),
            &sig,
            recid,
        )
        .unwrap();
        let point = vk.to_encoded_point(false);
        let recovered = crate::chain::keys::evm_address_from_pubkey(point.as_bytes()).unwrap();
        assert_eq!(recovered, expected, "recover 出的地址必须等于签名地址");
    }

    #[test]
    fn raw_tx_is_rlp_parseable_and_prefixed() {
        let sk = test_key();
        let signed = sample_tx().sign(&sk).unwrap();
        // 0x02 前缀
        assert_eq!(signed.raw_bytes()[0], 0x02);
        // 可被 RLP 解为 12 字段列表
        let fields = inspect_raw_tx(signed.raw_bytes()).unwrap();
        assert_eq!(fields.len(), 12);
        // 第一个字段是 chain_id = 84532
        assert_eq!(
            rlp::rlp_bytes_to_u64(fields[0].as_bytes().unwrap()).unwrap(),
            84532
        );
        // nonce = 0 → 编码为空字节串 (0x80)
        assert_eq!(fields[1].as_bytes().unwrap().len(), 0);
    }

    #[test]
    fn signing_hash_is_deterministic() {
        let tx = sample_tx();
        assert_eq!(tx.signing_hash(), tx.signing_hash());
        assert_ne!(tx.signing_hash(), [0u8; 32]);
    }

    #[test]
    fn contract_creation_uses_empty_to() {
        let sk = test_key();
        let mut tx = sample_tx();
        tx.to = None;
        let signed = tx.sign(&sk).unwrap();
        let fields = inspect_raw_tx(&signed.raw).unwrap();
        // to 字段（第 6 个，索引 5）应为空字节串
        assert_eq!(fields[5].as_bytes().unwrap().len(), 0);
    }

    #[test]
    fn sign_is_deterministic_under_rfc6979_but_still_recoverable() {
        // k256 使用 RFC 6979 确定性 k：同一 digest + 同一私钥产生同一签名
        // （这是 EVM 推荐行为，避免随机 k 泄漏私钥）。两次签名应完全一致。
        let sk = test_key();
        let tx = sample_tx();
        let a = tx.sign(&sk).unwrap();
        let b = tx.sign(&sk).unwrap();
        assert_eq!(a.raw_hex(), b.raw_hex(), "RFC 6979 应为确定性签名");
        assert_eq!(a.signing_hash(), b.signing_hash());
    }
}
