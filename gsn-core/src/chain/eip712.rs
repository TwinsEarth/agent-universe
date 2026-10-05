//! EIP-712 类型化数据签名 + EIP-3009 `transferWithAuthorization` 载荷。
//!
//! # 用途
//!
//! 让用户（或本节点）对 USDC 的「授权转账」签名：链上 USDC 合约据此把 `from` 的币
//! 直接转给 `to`，无需先 `approve`。本模块只负责**离线构造 digest 并签名**，不发交易。
//!
//! # 关键安全属性
//!
//! - **domain 隔离**：`domainSeparator` 绑定 `name/version/chainId/verifyingContract`，
//!   本签名在别的合约 / 别的链 / 别的 USDC 版本上**无效**。
//! - **deadline**：`validAfter`/`validBefore` 时间窗，过期不可用。
//! - **nonce 重放保护**：32 字节 nonce 一次性消费；链上合约保证同一 (from,nonce) 只执行一次。
//!
//! # 需外部审计
//!
//! 本模块所有 keccak 编码顺序、类型字符串与签名拼接均为资金信任根，上线前必须外部审计。
//!
//! # 诚实性
//!
//! 本模块**不验证**签名在真实 USDC 合约上是否被接受；那需要真实测试网部署。

use k256::ecdsa::SigningKey;
use sha3::{Digest, Keccak256};

use crate::chain::config::ChainError;
use crate::chain::keys::keccak256;

/// EIP-712 域（绑定 name/version/chainId/verifyingContract，标准四字段形式）。
#[derive(Debug, Clone)]
pub struct Eip712Domain {
    pub name: String,
    pub version: String,
    pub chain_id: u64,
    /// USDC 合约地址（20 字节）。
    pub verifying_contract: [u8; 20],
}

/// EIP712Domain 类型哈希 = keccak256("EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)")。
pub const EIP712_DOMAIN_TYPEHASH: [u8; 32] = [
    0x8b, 0x73, 0xc3, 0xc6, 0x9b, 0xb8, 0xfe, 0x3d, 0x51, 0x2e, 0xcc, 0x4c, 0xf7, 0x59, 0xcc, 0x79,
    0x23, 0x9f, 0x7b, 0x17, 0x9b, 0x0f, 0xfa, 0xca, 0xa9, 0xa7, 0x5d, 0x52, 0x2b, 0x39, 0x40, 0x0f,
];

impl Eip712Domain {
    /// 计算 domainSeparator = keccak256(typeHash ‖ encData)。
    pub fn separator(&self) -> [u8; 32] {
        let mut preimage = Vec::with_capacity(32 * 5);
        preimage.extend_from_slice(&EIP712_DOMAIN_TYPEHASH);
        preimage.extend_from_slice(&keccak256(self.name.as_bytes()));
        preimage.extend_from_slice(&keccak256(self.version.as_bytes()));
        // uint256 chainId → 32 字节大端
        preimage.extend_from_slice(&pad32_be_u64(self.chain_id));
        // address verifyingContract → 12 零字节 + 20 字节
        preimage.extend_from_slice(&pad20_to_32(&self.verifying_contract));
        keccak256(&preimage)
    }
}

/// 把 20 字节地址左补零到 32 字节（EVM ABI 编码 address）。
fn pad20_to_32(addr: &[u8; 20]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[12..32].copy_from_slice(addr);
    out
}

/// 把 u64 左补零到 32 字节大端（EVM ABI 编码 uint256）。
fn pad32_be_u64(v: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[24..32].copy_from_slice(&v.to_be_bytes());
    out
}

/// 把 u128 左补零到 32 字节大端。
fn pad32_be_u128(v: u128) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[16..32].copy_from_slice(&v.to_be_bytes());
    out
}

/// EIP-3009 `transferWithAuthorization` 授权参数。
#[derive(Debug, Clone)]
pub struct TransferWithAuthorization {
    pub from: [u8; 20],
    pub to: [u8; 20],
    /// 金额（最小单位，6 位 USDC）。
    pub value: u128,
    /// 生效时间（unix 秒，0 = 立即）。
    pub valid_after: u64,
    /// 过期时间（unix 秒）。
    pub valid_before: u64,
    /// 一次性 nonce（32 字节）。
    pub nonce: [u8; 32],
}

/// transferWithAuthorization 结构体类型哈希。
pub const TRANSFER_WITH_AUTHORIZATION_TYPEHASH: [u8; 32] = [
    0x7c, 0x7c, 0x6c, 0xdb, 0x67, 0xa1, 0x87, 0x43, 0xf4, 0x9e, 0xc6, 0xfa, 0x9b, 0x35, 0xf5, 0x0d,
    0x52, 0xed, 0x05, 0xcb, 0xed, 0x4c, 0xc5, 0x92, 0xe1, 0x3b, 0x44, 0x50, 0x1c, 0x1a, 0x22, 0x67,
];

impl TransferWithAuthorization {
    /// structHash = keccak256(typeHash ‖ encFields)。
    pub fn struct_hash(&self) -> [u8; 32] {
        let mut preimage = Vec::with_capacity(32 * 7);
        preimage.extend_from_slice(&TRANSFER_WITH_AUTHORIZATION_TYPEHASH);
        preimage.extend_from_slice(&pad20_to_32(&self.from));
        preimage.extend_from_slice(&pad20_to_32(&self.to));
        preimage.extend_from_slice(&pad32_be_u128(self.value));
        preimage.extend_from_slice(&pad32_be_u64(self.valid_after));
        preimage.extend_from_slice(&pad32_be_u64(self.valid_before));
        preimage.extend_from_slice(&self.nonce);
        keccak256(&preimage)
    }

    /// 最终 EIP-712 digest = keccak256(0x1901 ‖ domainSeparator ‖ structHash)。
    pub fn digest(&self, domain: &Eip712Domain) -> [u8; 32] {
        let mut preimage = Vec::with_capacity(66);
        preimage.push(0x19);
        preimage.push(0x01);
        preimage.extend_from_slice(&domain.separator());
        preimage.extend_from_slice(&self.struct_hash());
        keccak256(&preimage)
    }

    /// 对 digest 做 ECDSA 签名，返回 (r,s,v)（v ∈ {0,1}，EIP-155 未参与这里——
    /// EIP-3009 用 EIP-712，recovery id 直接 0/1）。
    pub fn sign(
        &self,
        domain: &Eip712Domain,
        sk: &SigningKey,
    ) -> Result<([u8; 32], [u8; 32], u8), ChainError> {
        let digest = self.digest(domain);
        let (sig, recid) = sk
            .sign_digest_recoverable(Keccak256::new_with_prefix(digest))
            .map_err(ChainError::from)?;
        let r: [u8; 32] = sig
            .r()
            .to_bytes()
            .as_slice()
            .try_into()
            .unwrap_or([0u8; 32]);
        let s: [u8; 32] = sig
            .s()
            .to_bytes()
            .as_slice()
            .try_into()
            .unwrap_or([0u8; 32]);
        Ok((r, s, recid.to_byte()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::keys::parse_private_hex_pub;
    use k256::ecdsa::SigningKey;

    fn domain() -> Eip712Domain {
        Eip712Domain {
            name: "USD Coin".into(),
            version: "2".into(),
            chain_id: 84532,
            verifying_contract: [0x55u8; 20],
        }
    }

    #[test]
    fn known_typehash_constants_match_keccak() {
        // 自校验：内置常量 == 对类型字符串算 keccak。
        let t = keccak256(
            b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
        );
        assert_eq!(t, EIP712_DOMAIN_TYPEHASH);
        let s = keccak256(
            b"TransferWithAuthorization(address from,address to,uint256 value,uint256 validAfter,uint256 validBefore,bytes32 nonce)",
        );
        assert_eq!(s, TRANSFER_WITH_AUTHORIZATION_TYPEHASH);
    }

    #[test]
    fn domain_separator_is_deterministic_and_isolated() {
        let d = domain();
        let sep = d.separator();
        assert_eq!(sep, d.separator());
        assert_ne!(sep, [0u8; 32]);
        // 改 chainId → domain 隔离，digest 必须变。
        let mut d2 = d.clone();
        d2.chain_id = 8453;
        assert_ne!(d2.separator(), sep);
        // 改 verifyingContract → 也隔离。
        let mut d3 = d.clone();
        d3.verifying_contract = [0x66u8; 20];
        assert_ne!(d3.separator(), sep);
    }

    #[test]
    fn digest_changes_with_any_field() {
        let d = domain();
        let auth = TransferWithAuthorization {
            from: [0x11u8; 20],
            to: [0x22u8; 20],
            value: 1_000_000,
            valid_after: 1_000,
            valid_before: 2_000,
            nonce: [0xabu8; 32],
        };
        let base = auth.digest(&d);
        let mut a2 = auth.clone();
        a2.to = [0x33u8; 20];
        assert_ne!(a2.digest(&d), base);
        let mut a3 = auth.clone();
        a3.valid_before = 9_999;
        assert_ne!(a3.digest(&d), base);
        let mut a4 = auth.clone();
        a4.nonce = [0x0cu8; 32];
        assert_ne!(a4.digest(&d), base);
    }

    #[test]
    fn eip3009_signature_recovers() {
        let sk = SigningKey::from_bytes(
            &parse_private_hex_pub(
                "4f3edf983ac636a65a842ce7c78d9aa706d3b113bce9c46f30d2d717b23f64de",
            )
            .unwrap()
            .into(),
        )
        .unwrap();
        let d = domain();
        let auth = TransferWithAuthorization {
            from: [0x11u8; 20],
            to: [0x22u8; 20],
            value: 1_000_000,
            valid_after: 1_000,
            valid_before: 2_000,
            nonce: [0xabu8; 32],
        };
        let digest = auth.digest(&d);
        let (r, s, v) = auth.sign(&d, &sk).unwrap();
        assert!(v <= 1);
        // 用 digest + (r,s,v) 恢复，应得到原公钥。
        let sig = k256::ecdsa::Signature::from_scalars(r, s).unwrap();
        let recid = k256::ecdsa::RecoveryId::try_from(v).unwrap();
        let vk = k256::ecdsa::VerifyingKey::recover_from_digest(
            Keccak256::new_with_prefix(digest),
            &sig,
            recid,
        )
        .unwrap();
        assert_eq!(vk, *sk.verifying_key());
    }
}
