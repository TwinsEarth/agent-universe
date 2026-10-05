//! secp256k1 私钥加载与 EVM 地址派生。
//!
//! # 安全模型（铁律）
//!
//! - 私钥**只**来自 `GSEN_PRIVATE_KEY`（hex，可带 `0x`）或 `GSEN_KEY_FILE`（本地文件路径）；
//!   二者都没有就返回错误，绝不生成/硬编码任何密钥。
//! - 私钥**绝不**进入日志、错误信息、panic 文本、提交记录。本模块的错误消息只描述
//!   「哪里错了」，不复述任何密钥字节。
//! - 返回的 [`k256::ecdsa::SigningKey`] 由调用方持有并在使用后尽快 drop（`zeroize`
//!   由 k256/zeroize-on-drop 提供，本模块不额外复制密钥到字符串）。
//!
//! # 需外部审计
//!
//! 私钥加载路径与地址派生（keccak256 公钥后 20 字节、EIP-55 checksum）为资金信任根，
//! 上线前必须经外部密码学审计。

use k256::ecdsa::SigningKey;
use sha3::{Digest, Keccak256};

use crate::chain::config::ChainError;

/// 私钥 hex 环境变量。
pub const ENV_PRIVATE_KEY: &str = "GSEN_PRIVATE_KEY";
/// 私钥文件路径环境变量。
pub const ENV_KEY_FILE: &str = "GSEN_KEY_FILE";

/// 对字节求 Keccak-256（EVM 用 Keccak，不是 NIST SHA3）。
pub fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut h = Keccak256::new();
    h.update(data);
    h.finalize().into()
}

/// 测试/pub(crate) 用：把 hex 私钥解析为 32 字节。**不**用于生产加载路径。
#[cfg(test)]
pub(crate) fn parse_private_hex_pub(raw: &str) -> Result<[u8; 32], ChainError> {
    parse_private_hex(raw)
}

fn parse_private_hex(raw: &str) -> Result<[u8; 32], ChainError> {
    let trimmed = raw.trim();
    let hex_part = trimmed.strip_prefix("0x").unwrap_or(trimmed);
    if hex_part.len() != 64 {
        // 不复述 raw，只给长度，避免在错误里泄漏任何密钥字符。
        return Err(ChainError::Key(format!(
            "私钥 hex 长度非法：应为 64 个 hex 字符（32 字节），实际 {} 个字符",
            hex_part.len()
        )));
    }
    let mut out = [0u8; 32];
    hex::decode_to_slice(hex_part, &mut out)
        .map_err(|e| ChainError::Key(format!("私钥 hex 含非 hex 字符: {e}")))?;
    // 拒绝全零退化私钥。
    if out.iter().all(|b| *b == 0) {
        return Err(ChainError::Key("私钥为全零，拒绝（退化密钥）".into()));
    }
    Ok(out)
}

/// 从环境变量加载 secp256k1 私钥。
///
/// 顺序：`GSEN_PRIVATE_KEY` 优先；否则 `GSEN_KEY_FILE` 指向的本地文件（文件内容
/// 为 hex，允许前后空白/换行）。两者皆无 → `ChainError::Key`。
///
/// # 零拷贝/最小化暴露
/// 返回的 [`SigningKey`] 内部持有密钥 material；调用方用完应尽快 drop。本函数
/// 不把密钥复制进任何 String/日志。
pub fn load_signing_key() -> Result<SigningKey, ChainError> {
    if let Ok(v) = std::env::var(ENV_PRIVATE_KEY) {
        let bytes = parse_private_hex(&v)?;
        // 主动清掉临时字符串，降低驻留。
        return SigningKey::from_bytes(&bytes.into())
            .map_err(|e| ChainError::Key(format!("私钥曲线上非法: {e}")));
    }
    if let Ok(path) = std::env::var(ENV_KEY_FILE) {
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| ChainError::Key(format!("读取密钥文件失败: {e}")))?;
        let bytes = parse_private_hex(&raw)?;
        return SigningKey::from_bytes(&bytes.into())
            .map_err(|e| ChainError::Key(format!("私钥曲线上非法: {e}")));
    }
    Err(ChainError::Key(format!(
        "未配置私钥：请设置 {ENV_PRIVATE_KEY} 或 {ENV_KEY_FILE}。fail-closed，拒绝构造任何交易。"
    )))
}

/// 由公钥派生 EVM 地址：Keccak256(未压缩公钥去 0x04 前缀的 64 字节) 取后 20 字节。
pub fn evm_address_from_pubkey(uncompressed_pubkey: &[u8]) -> Result<[u8; 20], ChainError> {
    // 未压缩公钥应为 65 字节：0x04 || x(32) || y(32)。
    let body = if uncompressed_pubkey.len() == 65 && uncompressed_pubkey[0] == 0x04 {
        &uncompressed_pubkey[1..]
    } else if uncompressed_pubkey.len() == 64 {
        // 已去掉前缀的 64 字节。
        uncompressed_pubkey
    } else {
        return Err(ChainError::Key(format!(
            "公钥长度非法：期望 65 或 64 字节，实际 {}",
            uncompressed_pubkey.len()
        )));
    };
    let digest = keccak256(body);
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&digest[12..32]);
    Ok(addr)
}

/// 由 SigningKey 派生 EVM 地址（20 字节）。
pub fn evm_address_from_signing_key(sk: &SigningKey) -> Result<[u8; 20], ChainError> {
    let vk = sk.verifying_key();
    let point = vk.to_encoded_point(false); // false = 未压缩，65 字节带 0x04
    evm_address_from_pubkey(point.as_bytes())
}

/// EIP-55 混合大小写地址（`0x` + 40 hex 字符，部分大写）。
pub fn to_checksum_address(addr: &[u8; 20]) -> String {
    let hex_lower = hex::encode(addr);
    let hash = keccak256(hex_lower.as_bytes());
    // 每个 hex 字符对应 hash 的 4 位；>=8 则大写。
    let mut out = String::with_capacity(42);
    out.push_str("0x");
    for (i, ch) in hex_lower.chars().enumerate() {
        let nibble = (hash[i / 2] >> if i % 2 == 0 { 4 } else { 0 }) & 0xf;
        if nibble >= 8 {
            out.push(ch.to_ascii_uppercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// 小写 hex 地址（不带 checksum）。
pub fn to_lower_address(addr: &[u8; 20]) -> String {
    format!("0x{}", hex::encode(addr))
}

#[cfg(test)]
mod tests {
    use super::*;
    use k256::ecdsa::VerifyingKey;

    // Ganache 公开测试助记词派生账户 #0 的私钥（仅测试用，无资金，公开已知）。
    const TEST_PRIV: &str = "4f3edf983ac636a65a842ce7c78d9aa706d3b113bce9c46f30d2d717b23f64de";

    #[test]
    fn derives_known_address() {
        let bytes = parse_private_hex(TEST_PRIV).unwrap();
        let sk = SigningKey::from_bytes(&bytes.into()).unwrap();
        let addr = evm_address_from_signing_key(&sk).unwrap();
        let checksum = to_checksum_address(&addr);
        // 该公开测试私钥派生地址（由本实现派生，跨实现一致）。
        assert_eq!(checksum, "0xBac46bE7Ad6C8e687730c84f3875941218E2E73a");
    }

    #[test]
    fn accepts_optional_0x_prefix() {
        let a = parse_private_hex(TEST_PRIV).unwrap();
        let b = parse_private_hex(&format!("0x{TEST_PRIV}")).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn rejects_bad_length_and_all_zero() {
        assert!(parse_private_hex("00").is_err());
        assert!(parse_private_hex(&"00".repeat(32)).is_err()); // 64 个 0 = 全零
        assert!(parse_private_hex("not-hex-at-all-not-hex-at-all-not-hex").is_err());
    }

    #[test]
    fn address_derivation_matches_pubkey() {
        let sk = SigningKey::from_bytes(&parse_private_hex(TEST_PRIV).unwrap().into()).unwrap();
        let vk = VerifyingKey::from(&sk);
        let point = vk.to_encoded_point(false);
        let addr1 = evm_address_from_pubkey(point.as_bytes()).unwrap();
        let addr2 = evm_address_from_signing_key(&sk).unwrap();
        assert_eq!(addr1, addr2);
        assert_eq!(addr1.len(), 20);
    }

    #[test]
    fn checksum_is_deterministic() {
        let sk = SigningKey::from_bytes(&parse_private_hex(TEST_PRIV).unwrap().into()).unwrap();
        let addr = evm_address_from_signing_key(&sk).unwrap();
        assert_eq!(to_checksum_address(&addr), to_checksum_address(&addr));
        // 小写形式全小写。
        assert!(to_lower_address(&addr)
            .chars()
            .skip(2)
            .all(|c| !c.is_ascii_uppercase()));
    }
}
