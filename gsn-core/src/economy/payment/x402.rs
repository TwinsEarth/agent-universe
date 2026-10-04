//! v3.9.2 EVM x402（USDC 稳定币）**纯确定性协议内核**。
//!
//! # 只构造/校验，不广播、不动钱
//!
//! x402 复用 HTTP `402 Payment Required`：资源方返回一个 payment requirement，
//! 客户端据此构造一笔稳定币授权支付，附上支付证明后重试，由 facilitator 链上结算。
//! 在 EVM/USDC 轨道，授权支付采用 **EIP-3009 `transferWithAuthorization`**，其待签
//! 摘要是标准 **EIP-712** typed-data digest。
//!
//! 本模块只做**与环境无关的确定性工作**：
//!
//! - 解析并校验 402 challenge（scheme/network/resource/payTo/金额/到期/资产）；
//! - 用纯 Rust、无第三方依赖、零 unsafe 的 [`keccak256`] 计算 EIP-712 摘要；
//! - 构造 `transferWithAuthorization` 授权结构与其待签 digest；
//! - 对 facilitator 结算回执做**精确金额守恒**校验（不多付不少付）。
//!
//! 它**不**：
//!
//! - 持有/生成 secp256k1 私钥，不产出 EVM 签名（本版没有 EVM 宿主签名后端，
//!   任何签发请求一律 [`X402Error::EvmSignerNotConfigured`] 具名 fail-closed，绝不伪造）；
//! - 连接 EVM RPC/facilitator，不广播、不划转、不托管、不兑换；
//! - 读系统时钟——过期判定的 `now_unix` 由调用方显式传入（保持纯函数可测）。
//!
//! 金额一律链上最小单位 raw units（USDC 为 6 位小数的整数），**零浮点**；全部比较为
//! 整数比较，零 syscall、零 unsafe、无 panic 路径。
//!
//! # 正确性锚点
//!
//! [`keccak256`] 用空串与 "abc" 的标准 Keccak-256 向量校验；EIP-3009/EIP-712 的两个
//! TYPEHASH 用 USDC 官方 ABI 的公开常量校验，从而同时锚定「Keccak 填充域（0x01/0x80，
//! 区别于 SHA3 的 0x06）」与「类型串/ABI 编码」两处易错点。

use serde::{Deserialize, Serialize};

// ───────────────────────────────────────────────────────────────────────────
// Keccak-256（纯 Rust，无依赖，零 unsafe）
// ───────────────────────────────────────────────────────────────────────────

const RATE: usize = 136; // 1088-bit rate for Keccak-256
const ROUND_CONSTANTS: [u64; 24] = [
    0x0000000000000001,
    0x0000000000008082,
    0x800000000000808a,
    0x8000000080008000,
    0x000000000000808b,
    0x0000000080000001,
    0x8000000080008081,
    0x8000000000008009,
    0x000000000000008a,
    0x0000000000000088,
    0x0000000080008009,
    0x000000008000000a,
    0x000000008000808b,
    0x800000000000008b,
    0x8000000000008089,
    0x8000000000008003,
    0x8000000000008002,
    0x8000000000000080,
    0x000000000000800a,
    0x800000008000000a,
    0x8000000080008081,
    0x8000000000008080,
    0x0000000080000001,
    0x8000000080008008,
];
const ROTATIONS: [u32; 25] = [
    0, 1, 62, 28, 27, 36, 44, 6, 55, 20, 3, 10, 43, 25, 39, 41, 45, 15, 21, 8, 18, 2, 61, 56, 14,
];

fn keccak_f(state: &mut [u64; 25]) {
    for rc in ROUND_CONSTANTS {
        // θ
        let mut c = [0u64; 5];
        for x in 0..5 {
            c[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
        }
        let mut d = [0u64; 5];
        for x in 0..5 {
            d[x] = c[(x + 4) % 5] ^ c[(x + 1) % 5].rotate_left(1);
        }
        for x in 0..5 {
            for y in 0..5 {
                state[x + 5 * y] ^= d[x];
            }
        }
        // ρ + π
        let mut b = [0u64; 25];
        for x in 0..5 {
            for y in 0..5 {
                let idx = x + 5 * y;
                let new_x = y;
                let new_y = (2 * x + 3 * y) % 5;
                b[new_x + 5 * new_y] = state[idx].rotate_left(ROTATIONS[idx]);
            }
        }
        // χ
        for x in 0..5 {
            for y in 0..5 {
                let idx = x + 5 * y;
                state[idx] = b[idx] ^ ((!b[(x + 1) % 5 + 5 * y]) & b[(x + 2) % 5 + 5 * y]);
            }
        }
        // ι
        state[0] ^= rc;
    }
}

/// 纯 Keccak-256（Ethereum 使用；填充域 0x01/0x80，区别于 NIST SHA3-256 的 0x06）。
pub fn keccak256(input: &[u8]) -> [u8; 32] {
    let mut state = [0u64; 25];
    let mut offset = 0;

    // 吸收完整块。
    while input.len() - offset >= RATE {
        for i in 0..RATE / 8 {
            let chunk = &input[offset + i * 8..offset + i * 8 + 8];
            state[i] ^= u64::from_le_bytes([
                chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
            ]);
        }
        keccak_f(&mut state);
        offset += RATE;
    }

    // 末块 + Keccak 填充（0x01 ... 0x80）。
    let mut block = [0u8; RATE];
    let rem = &input[offset..];
    block[..rem.len()].copy_from_slice(rem);
    block[rem.len()] ^= 0x01;
    block[RATE - 1] ^= 0x80;
    for i in 0..RATE / 8 {
        let mut word = [0u8; 8];
        word.copy_from_slice(&block[i * 8..i * 8 + 8]);
        state[i] ^= u64::from_le_bytes(word);
    }
    keccak_f(&mut state);

    // 挤出 32 字节。
    let mut out = [0u8; 32];
    for i in 0..4 {
        out[i * 8..i * 8 + 8].copy_from_slice(&state[i].to_le_bytes());
    }
    out
}

// ───────────────────────────────────────────────────────────────────────────
// 基础类型
// ───────────────────────────────────────────────────────────────────────────

/// 20 字节 EVM 地址；序列化为 `0x` + 40 hex。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EvmAddress(pub [u8; 20]);

impl EvmAddress {
    pub const ZERO: EvmAddress = EvmAddress([0u8; 20]);

    /// 从 `0x`+40hex（大小写不限）解析；非法长度/hex 返回 None。
    pub fn from_hex(s: &str) -> Option<Self> {
        let h = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"))?;
        if h.len() != 40 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let mut bytes = [0u8; 20];
        for i in 0..20 {
            bytes[i] = u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).ok()?;
        }
        Some(EvmAddress(bytes))
    }

    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(42);
        s.push_str("0x");
        for b in &self.0 {
            s.push_str(&format!("{b:02x}"));
        }
        s
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 20]
    }
}

impl Serialize for EvmAddress {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for EvmAddress {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let s = String::deserialize(de)?;
        EvmAddress::from_hex(&s).ok_or_else(|| serde::de::Error::custom("invalid EVM address"))
    }
}

/// 32 字节 nonce；序列化为 `0x`+64hex。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nonce32(pub [u8; 32]);

impl Nonce32 {
    pub fn from_hex(s: &str) -> Option<Self> {
        let h = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"))?;
        if h.len() != 64 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let mut bytes = [0u8; 32];
        for i in 0..32 {
            bytes[i] = u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).ok()?;
        }
        Some(Nonce32(bytes))
    }

    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(66);
        s.push_str("0x");
        for b in &self.0 {
            s.push_str(&format!("{b:02x}"));
        }
        s
    }
}

impl Serialize for Nonce32 {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Nonce32 {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let s = String::deserialize(de)?;
        Nonce32::from_hex(&s).ok_or_else(|| serde::de::Error::custom("invalid 32-byte nonce"))
    }
}

// ───────────────────────────────────────────────────────────────────────────
// 领域错误
// ───────────────────────────────────────────────────────────────────────────

/// x402/USDC 协议内核的类型化拒绝（不静默降级、不动钱）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum X402Error {
    /// scheme 非本内核支持的 "exact"。
    UnsupportedScheme { got: String },
    /// network 为空。
    EmptyNetwork,
    /// resource 为空。
    EmptyResource,
    /// 金额必须为正 raw units。
    NonPositiveAmount,
    /// 资产小数位非法（>36）。
    BadDecimals { got: u8 },
    /// 资产 symbol 为空。
    EmptySymbol,
    /// challenge 已过期（deadline <= now）。
    ChallengeExpired { deadline: u64, now: u64 },
    /// max_timeout_seconds 必须为正。
    NonPositiveTimeout,
    /// 付款方为零地址。
    ZeroSender,
    /// 授权窗口非法：valid_after > valid_before，或 valid_before 晚于 challenge deadline。
    InvalidAuthorizationWindow {
        valid_after: u64,
        valid_before: u64,
        deadline: u64,
    },
    /// EIP-712 domain 的 verifyingContract 与 challenge 资产（USDC 合约）不一致。
    VerifyingContractMismatch { asset: String, verifying: String },
    /// EIP-712 domain 的 name/version 为空。
    EmptyDomainField { field: String },
    /// facilitator 结算金额与应付金额不相等（多付或少付）。
    SettlementMismatch { required: u128, paid: u128 },
    /// 本版没有 EVM(secp256k1) 宿主签名后端——fail-closed，绝不伪造链上签名。
    EvmSignerNotConfigured,
}

impl std::fmt::Display for X402Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            X402Error::UnsupportedScheme { got } => {
                write!(f, "X402_UNSUPPORTED_SCHEME: only exact is supported, got {got}")
            }
            X402Error::EmptyNetwork => write!(f, "X402_EMPTY_NETWORK: network must be non-blank"),
            X402Error::EmptyResource => write!(f, "X402_EMPTY_RESOURCE: resource must be non-blank"),
            X402Error::NonPositiveAmount => {
                write!(f, "X402_NON_POSITIVE_AMOUNT: amount_raw must be > 0")
            }
            X402Error::BadDecimals { got } => {
                write!(f, "X402_BAD_DECIMALS: asset decimals must be <=36, got {got}")
            }
            X402Error::EmptySymbol => write!(f, "X402_EMPTY_SYMBOL: asset symbol required"),
            X402Error::ChallengeExpired { deadline, now } => write!(
                f,
                "X402_CHALLENGE_EXPIRED: deadline {deadline} <= now {now}"
            ),
            X402Error::NonPositiveTimeout => {
                write!(f, "X402_NON_POSITIVE_TIMEOUT: max_timeout_seconds must be > 0")
            }
            X402Error::ZeroSender => write!(f, "X402_ZERO_SENDER: payer from must be non-zero"),
            X402Error::InvalidAuthorizationWindow {
                valid_after,
                valid_before,
                deadline,
            } => write!(
                f,
                "X402_INVALID_AUTH_WINDOW: require valid_after<=valid_before<=deadline, got {valid_after}/{valid_before}/{deadline}"
            ),
            X402Error::VerifyingContractMismatch { asset, verifying } => write!(
                f,
                "X402_VERIFYING_CONTRACT_MISMATCH: asset {asset} != verifying_contract {verifying}"
            ),
            X402Error::EmptyDomainField { field } => {
                write!(f, "X402_EMPTY_DOMAIN_FIELD: EIP-712 {field} must be non-blank")
            }
            X402Error::SettlementMismatch { required, paid } => write!(
                f,
                "X402_SETTLEMENT_MISMATCH: exact scheme requires paid({paid}) == required({required})"
            ),
            X402Error::EvmSignerNotConfigured => write!(
                f,
                "X402_EVM_SIGNER_NOT_CONFIGURED: no secp256k1 host signer backend (refusing to fabricate an on-chain signature)"
            ),
        }
    }
}

impl std::error::Error for X402Error {}

// ───────────────────────────────────────────────────────────────────────────
// Challenge（402 Payment Required 的 payment requirement 子集）
// ───────────────────────────────────────────────────────────────────────────

/// 被支付的稳定币资产（USDC）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct X402Asset {
    /// 代币合约地址（须等于 EIP-712 domain.verifyingContract）。
    pub contract: EvmAddress,
    /// 小数位（USDC 主网为 6）；金额 raw 已按此最小单位给出。
    pub decimals: u8,
    /// 如 "USDC"。
    pub symbol: String,
}

/// x402 `402 Payment Required` 中本内核校验所需的 payment requirement（确定性子集）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct X402Challenge {
    /// 结算模式；本内核只支持精确金额 "exact"。
    pub scheme: String,
    /// 网络标识（如 "base"）；非空，内核不内置网络→chainId 映射。
    pub network: String,
    /// 触发 402 的资源标识。
    pub resource: String,
    /// 收款地址（authorization.to）。
    pub pay_to: EvmAddress,
    /// 精确应付金额（链上最小单位 raw units）。
    pub max_amount_required_raw: u128,
    /// 稳定币资产。
    pub asset: X402Asset,
    /// challenge 有效截止（unix 秒）。
    pub deadline_unix: u64,
    /// facilitator 结算允许的最大耗时（秒）。
    pub max_timeout_seconds: u32,
}

impl X402Challenge {
    /// 纯校验。`now_unix` 由调用方显式传入（内核不读时钟）。
    pub fn validate(&self, now_unix: u64) -> Result<(), X402Error> {
        if self.scheme != "exact" {
            return Err(X402Error::UnsupportedScheme {
                got: self.scheme.clone(),
            });
        }
        if self.network.trim().is_empty() {
            return Err(X402Error::EmptyNetwork);
        }
        if self.resource.trim().is_empty() {
            return Err(X402Error::EmptyResource);
        }
        if self.max_amount_required_raw == 0 {
            return Err(X402Error::NonPositiveAmount);
        }
        if self.asset.decimals > 36 {
            return Err(X402Error::BadDecimals {
                got: self.asset.decimals,
            });
        }
        if self.asset.symbol.trim().is_empty() {
            return Err(X402Error::EmptySymbol);
        }
        if self.max_timeout_seconds == 0 {
            return Err(X402Error::NonPositiveTimeout);
        }
        if self.deadline_unix <= now_unix {
            return Err(X402Error::ChallengeExpired {
                deadline: self.deadline_unix,
                now: now_unix,
            });
        }
        Ok(())
    }
}

// ───────────────────────────────────────────────────────────────────────────
// EIP-3009 transferWithAuthorization + EIP-712 摘要（纯构造，不签名）
// ───────────────────────────────────────────────────────────────────────────

/// EIP-712 domain（USDC：name="USD Coin", version="2"）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct X402Domain {
    pub name: String,
    pub version: String,
    pub chain_id: u64,
    pub verifying_contract: EvmAddress,
}

/// EIP-3009 `TransferWithAuthorization(address from,address to,uint256 value,uint256
/// validAfter,uint256 validBefore,bytes32 nonce)` 的类型哈希（EIP-3009 官方常量，
/// 见 eips.ethereum.org/EIPS/eip-3009）。
pub const TRANSFER_WITH_AUTHORIZATION_TYPEHASH_HEX: &str =
    "0x7c7c6cdb67a18743f49ec6fa9b35f50d52ed05cbed4cc592e13b44501c1a2267";

/// `EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)` 类型哈希。
pub const DOMAIN_TYPEHASH_HEX: &str =
    "0x8b73c3c69bb8fe3d512ecc4cf759cc79239f7b179b0ffacaa9a75d522b39400f";

/// 与 [`TRANSFER_WITH_AUTHORIZATION_TYPEHASH_HEX`] 等价的编译期字节常量（避免任何运行期解析）。
const TRANSFER_TYPEHASH_BYTES: [u8; 32] = [
    124, 124, 108, 219, 103, 161, 135, 67, 244, 158, 198, 250, 155, 53, 245, 13, 82, 237, 5, 203,
    237, 76, 197, 146, 225, 59, 68, 80, 28, 26, 34, 103,
];

/// 与 [`DOMAIN_TYPEHASH_HEX`] 等价的编译期字节常量。
const DOMAIN_TYPEHASH_BYTES: [u8; 32] = [
    139, 115, 195, 198, 155, 184, 254, 61, 81, 46, 204, 76, 247, 89, 204, 121, 35, 159, 123, 23,
    155, 15, 250, 202, 169, 167, 93, 82, 43, 57, 64, 15,
];

fn word_u256(v: u128) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[16..].copy_from_slice(&v.to_be_bytes());
    w
}

fn word_u64(v: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..].copy_from_slice(&v.to_be_bytes());
    w
}

fn word_address(a: &EvmAddress) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[12..].copy_from_slice(&a.0);
    w
}

fn encode_dynamic_string(s: &str) -> [u8; 32] {
    keccak256(s.as_bytes())
}

impl X402Domain {
    fn validate(&self) -> Result<(), X402Error> {
        if self.name.trim().is_empty() {
            return Err(X402Error::EmptyDomainField {
                field: "name".to_string(),
            });
        }
        if self.version.trim().is_empty() {
            return Err(X402Error::EmptyDomainField {
                field: "version".to_string(),
            });
        }
        Ok(())
    }

    /// EIP-712 domainSeparator。
    pub fn separator(&self) -> [u8; 32] {
        let mut buf = Vec::with_capacity(32 * 5);
        buf.extend_from_slice(&DOMAIN_TYPEHASH_BYTES);
        buf.extend_from_slice(&encode_dynamic_string(&self.name));
        buf.extend_from_slice(&encode_dynamic_string(&self.version));
        buf.extend_from_slice(&word_u64(self.chain_id));
        buf.extend_from_slice(&word_address(&self.verifying_contract));
        keccak256(&buf)
    }
}

/// 构造好的 EIP-3009 授权（含待签 EIP-712 digest，但**不含签名**）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferAuthorization {
    pub from: EvmAddress,
    pub to: EvmAddress,
    pub value_raw: u128,
    pub valid_after: u64,
    pub valid_before: u64,
    pub nonce: Nonce32,
    pub chain_id: u64,
    pub verifying_contract: EvmAddress,
    /// EIP-712 digest（这是应交给 secp256k1 宿主签名后端的 32 字节消息）。
    pub digest_hex: String,
    /// 始终为 None：本版不产出 EVM 签名。
    pub signature_hex: Option<String>,
}

fn bytes_to_hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(2 + b.len() * 2);
    s.push('0');
    s.push('x');
    for x in b {
        s.push_str(&format!("{x:02x}"));
    }
    s
}

impl X402Challenge {
    /// 据 challenge 构造 EIP-3009 授权并计算 EIP-712 待签摘要。
    ///
    /// `now_unix` 用于先校验 challenge 未过期；`valid_before` 不得晚于 challenge deadline，
    /// `value` 精确等于应付金额，`to=pay_to`；`domain.verifying_contract` 必须就是资产合约。
    pub fn build_authorization(
        &self,
        now_unix: u64,
        domain: &X402Domain,
        from: EvmAddress,
        valid_after: u64,
        valid_before: u64,
        nonce: Nonce32,
    ) -> Result<TransferAuthorization, X402Error> {
        self.validate(now_unix)?;
        domain.validate()?;
        if from.is_zero() {
            return Err(X402Error::ZeroSender);
        }
        if valid_after > valid_before || valid_before > self.deadline_unix {
            return Err(X402Error::InvalidAuthorizationWindow {
                valid_after,
                valid_before,
                deadline: self.deadline_unix,
            });
        }
        if domain.verifying_contract != self.asset.contract {
            return Err(X402Error::VerifyingContractMismatch {
                asset: self.asset.contract.to_hex(),
                verifying: domain.verifying_contract.to_hex(),
            });
        }

        // structHash
        let mut struct_buf = Vec::with_capacity(32 * 7);
        struct_buf.extend_from_slice(&TRANSFER_TYPEHASH_BYTES);
        struct_buf.extend_from_slice(&word_address(&from));
        struct_buf.extend_from_slice(&word_address(&self.pay_to));
        struct_buf.extend_from_slice(&word_u256(self.max_amount_required_raw));
        struct_buf.extend_from_slice(&word_u64(valid_after));
        struct_buf.extend_from_slice(&word_u64(valid_before));
        struct_buf.extend_from_slice(&nonce.0);
        let struct_hash = keccak256(&struct_buf);

        // digest = keccak(0x1901 || domainSeparator || structHash)
        let sep = domain.separator();
        let mut digest_buf = Vec::with_capacity(2 + 32 + 32);
        digest_buf.extend_from_slice(&[0x19, 0x01]);
        digest_buf.extend_from_slice(&sep);
        digest_buf.extend_from_slice(&struct_hash);
        let digest = keccak256(&digest_buf);

        Ok(TransferAuthorization {
            from,
            to: self.pay_to,
            value_raw: self.max_amount_required_raw,
            valid_after,
            valid_before,
            nonce,
            chain_id: domain.chain_id,
            verifying_contract: domain.verifying_contract,
            digest_hex: bytes_to_hex(&digest),
            signature_hex: None,
        })
    }
}

/// facilitator 结算回执的精确守恒校验：exact 模式下实付必须恰等于应付。
pub fn verify_settlement(required_raw: u128, paid_raw: u128) -> Result<(), X402Error> {
    if required_raw == 0 {
        return Err(X402Error::NonPositiveAmount);
    }
    if paid_raw != required_raw {
        return Err(X402Error::SettlementMismatch {
            required: required_raw,
            paid: paid_raw,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_to_bytes(hex: &str, out: &mut [u8]) {
        let h = hex.strip_prefix("0x").unwrap_or(hex);
        for i in 0..out.len() {
            out[i] = u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).unwrap();
        }
    }

    #[test]
    fn keccak_known_vectors() {
        // 标准 Keccak-256（区别于 NIST SHA3-256）。
        assert_eq!(
            bytes_to_hex(&keccak256(b"")),
            "0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
        );
        assert_eq!(
            bytes_to_hex(&keccak256(b"abc")),
            "0x4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45"
        );
        // 多块（>rate=136）吸收路径：与独立实现 pycryptodome 的结果交叉锚定。
        let pat = b"x402-agent-universe-keccak-multiblock-vector|";
        let mut multi = pat.repeat(8);
        multi.truncate(200);
        assert_eq!(multi.len(), 200);
        assert_eq!(
            bytes_to_hex(&keccak256(&multi)),
            "0x5322d46a85e4bd44d338df264c630ee74374f9e1d53a34af37cf012ca8b165fe"
        );
    }

    #[test]
    fn eip712_and_eip3009_typehashes_match_known_constants() {
        let calc_struct = bytes_to_hex(&keccak256(
            b"TransferWithAuthorization(address from,address to,uint256 value,uint256 validAfter,uint256 validBefore,bytes32 nonce)",
        ));
        assert_eq!(calc_struct, TRANSFER_WITH_AUTHORIZATION_TYPEHASH_HEX);
        let calc_domain = bytes_to_hex(&keccak256(
            b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
        ));
        assert_eq!(calc_domain, DOMAIN_TYPEHASH_HEX);
    }

    #[test]
    fn address_parse_roundtrip_and_zero() {
        let a = EvmAddress::from_hex("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913").unwrap();
        assert_eq!(a.to_hex(), "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913");
        assert!(EvmAddress::from_hex("0x1234").is_none());
        assert!(EvmAddress::from_hex("0x").is_none());
        assert!(EvmAddress::from_hex("0xzz3589fcd6edb6e08f4c7c32d4f71b54bda029130").is_none());
        assert!(EvmAddress::ZERO.is_zero());
        assert!(!a.is_zero());
    }

    fn usdc() -> EvmAddress {
        EvmAddress::from_hex("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913").unwrap()
        // Base USDC
    }
    fn payee() -> EvmAddress {
        EvmAddress::from_hex("0x1111111111111111111111111111111111111111").unwrap()
    }
    fn payer() -> EvmAddress {
        EvmAddress::from_hex("0x2222222222222222222222222222222222222222").unwrap()
    }

    fn good_challenge() -> X402Challenge {
        X402Challenge {
            scheme: "exact".to_string(),
            network: "base".to_string(),
            resource: "https://api.example.local/compute".to_string(),
            pay_to: payee(),
            max_amount_required_raw: 1_000_000, // 1.0 USDC (6 decimals)
            asset: X402Asset {
                contract: usdc(),
                decimals: 6,
                symbol: "USDC".to_string(),
            },
            deadline_unix: 2_000_000_000,
            max_timeout_seconds: 30,
        }
    }

    fn base_domain() -> X402Domain {
        X402Domain {
            name: "USD Coin".to_string(),
            version: "2".to_string(),
            chain_id: 8453,
            verifying_contract: usdc(),
        }
    }

    #[test]
    fn challenge_validate_accepts_good_and_rejects_each_failure() {
        assert!(good_challenge().validate(1_000_000).is_ok());

        let mut c = good_challenge();
        c.scheme = "percent".to_string();
        assert!(matches!(
            c.validate(1_000_000),
            Err(X402Error::UnsupportedScheme { .. })
        ));

        let mut c = good_challenge();
        c.network = "  ".to_string();
        assert!(matches!(
            c.validate(1_000_000),
            Err(X402Error::EmptyNetwork)
        ));

        let mut c = good_challenge();
        c.resource = String::new();
        assert!(matches!(
            c.validate(1_000_000),
            Err(X402Error::EmptyResource)
        ));

        let mut c = good_challenge();
        c.max_amount_required_raw = 0;
        assert!(matches!(
            c.validate(1_000_000),
            Err(X402Error::NonPositiveAmount)
        ));

        let mut c = good_challenge();
        c.asset.decimals = 99;
        assert!(matches!(
            c.validate(1_000_000),
            Err(X402Error::BadDecimals { .. })
        ));

        let mut c = good_challenge();
        c.asset.symbol = String::new();
        assert!(matches!(c.validate(1_000_000), Err(X402Error::EmptySymbol)));

        let mut c = good_challenge();
        c.max_timeout_seconds = 0;
        assert!(matches!(
            c.validate(1_000_000),
            Err(X402Error::NonPositiveTimeout)
        ));

        // 已过期（deadline <= now）。
        assert!(matches!(
            good_challenge().validate(2_000_000_000),
            Err(X402Error::ChallengeExpired { .. })
        ));
    }

    #[test]
    fn authorization_digest_is_deterministic_and_32_bytes_without_signature() {
        let c = good_challenge();
        let d = base_domain();
        let nonce = Nonce32([9u8; 32]);
        let a = c
            .build_authorization(1_000_000, &d, payer(), 1_000_000, 1_999_999_999, nonce)
            .unwrap();
        let b = c
            .build_authorization(1_000_000, &d, payer(), 1_000_000, 1_999_999_999, nonce)
            .unwrap();
        assert_eq!(a, b);
        assert!(a.digest_hex.starts_with("0x"));
        assert_eq!(a.digest_hex.len(), 66);
        assert_eq!(a.signature_hex, None);
        assert_eq!(a.value_raw, 1_000_000);
        assert_eq!(a.to, payee());
        // 32 字节且非全零。
        let mut dg = [0u8; 32];
        hex_to_bytes(&a.digest_hex, &mut dg);
        assert!(dg.iter().any(|x| *x != 0));
    }

    #[test]
    fn authorization_rejects_zero_payer_bad_window_and_contract_mismatch() {
        let c = good_challenge();
        let d = base_domain();
        let nonce = Nonce32([1u8; 32]);
        assert!(matches!(
            c.build_authorization(1_000_000, &d, EvmAddress::ZERO, 0, 1_999_999_999, nonce),
            Err(X402Error::ZeroSender)
        ));
        // valid_after > valid_before
        assert!(matches!(
            c.build_authorization(1_000_000, &d, payer(), 100, 90, nonce),
            Err(X402Error::InvalidAuthorizationWindow { .. })
        ));
        // valid_before > deadline
        assert!(matches!(
            c.build_authorization(1_000_000, &d, payer(), 0, 2_000_000_001, nonce),
            Err(X402Error::InvalidAuthorizationWindow { .. })
        ));
        // 用过期 challenge 构造也失败。
        assert!(c
            .build_authorization(3_000_000_000, &d, payer(), 0, 1_999_999_999, nonce)
            .is_err());

        let mut wrong = d.clone();
        wrong.verifying_contract = payee();
        assert!(matches!(
            c.build_authorization(1_000_000, &wrong, payer(), 0, 1_999_999_999, nonce),
            Err(X402Error::VerifyingContractMismatch { .. })
        ));

        let mut no_name = d.clone();
        no_name.name = String::new();
        assert!(matches!(
            c.build_authorization(1_000_000, &no_name, payer(), 0, 1_999_999_999, nonce),
            Err(X402Error::EmptyDomainField { .. })
        ));
    }

    #[test]
    fn digest_changes_when_any_field_changes() {
        let c = good_challenge();
        let d = base_domain();
        let base = c
            .build_authorization(1_000_000, &d, payer(), 0, 1_999_999_999, Nonce32([1u8; 32]))
            .unwrap()
            .digest_hex;

        let mut c2 = c.clone();
        c2.max_amount_required_raw = 2_000_000;
        let other = c2
            .build_authorization(1_000_000, &d, payer(), 0, 1_999_999_999, Nonce32([1u8; 32]))
            .unwrap()
            .digest_hex;
        assert_ne!(base, other);

        let mut d2 = d.clone();
        d2.chain_id = 1;
        let other = c
            .build_authorization(
                1_000_000,
                &d2,
                payer(),
                0,
                1_999_999_999,
                Nonce32([1u8; 32]),
            )
            .unwrap()
            .digest_hex;
        assert_ne!(base, other);

        let other = c
            .build_authorization(1_000_000, &d, payer(), 0, 1_999_999_999, Nonce32([2u8; 32]))
            .unwrap()
            .digest_hex;
        assert_ne!(base, other);
    }

    #[test]
    fn settlement_exact_conservation() {
        assert!(verify_settlement(1_000_000, 1_000_000).is_ok());
        assert!(matches!(
            verify_settlement(1_000_000, 999_999),
            Err(X402Error::SettlementMismatch { .. })
        ));
        assert!(matches!(
            verify_settlement(1_000_000, 1_000_001),
            Err(X402Error::SettlementMismatch { .. })
        ));
        assert!(matches!(
            verify_settlement(0, 0),
            Err(X402Error::NonPositiveAmount)
        ));
    }
}
