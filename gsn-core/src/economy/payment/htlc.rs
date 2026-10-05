//! BTC HTLC 纯校验内核（v3.9.6）。
//!
//! # 定位
//!
//! 本模块只承担沙盒可安全完成的**链下确定性校验面**：在调用方已经从受信来源取证
//! （交易输出脚本、witnessScript、当前链高度/时间、金额）后，内核确定性地回答：
//!
//! 1. 哈希锁：`SHA256(preimage) == payment_hash`（与 v3.9.3 L402 同一标准 sha2）；
//! 2. 超时锁：CLTV 绝对锁定期的整数解析、规范编码、以及在调用方显式给出的当前
//!    高度/时间下是否仍未到期（内核不读系统时钟）；
//! 3. 金额守恒：整数 sats 下 `offered + fee == total`，多/少一律具名拒绝；
//! 4. P2WSH 脚本交易结构：witnessProgram 是否等于 `SHA256(witnessScript)`，以及
//!    witnessScript 是否严格匹配接收方凭原像立即取走、超时后发送方可退的标准
//!    HTLC 模板（含 OP_HASH256 哈希锁与 OP_CHECKLOCKTIMEVERIFY 超时分支）。
//!
//! # 明确不做（fail-closed）
//!
//! - 不连接任何比特币节点 / Electrum / Esplora，不读取真实区块或 UTXO；
//! - 不构造、不签名、不广播任何交易，不持私钥，不做 secp256k1 签名验证
//!   （模板里的两个 33 字节公钥只做**长度/位置**结构校验，不验点是否在曲线上）；
//! - 不执行真正的 Bitcoin Script 解释器，不实现完整 BIP16/141 witness 校验；
//! - 不解析 bech32/segwit 地址字符串（调用方直接传 32 字节 witness program hex）；
//! - 不结算 HTLC（[`handle`] 层的 finalize 一律 `HTLC_NODE_NOT_CONFIGURED`）。
//!
//! 因此这里的“通过”只表示**给定取证数据在上述四项确定性规则下自洽**，不代表
//! HTLC 已在比特币链上被打包或最终确认。金额用整数 **sats（u64）**，全程不使用浮点。

use sha2::{Digest, Sha256};

use super::l402::{PaymentHash, Preimage};

/// OP_IF
const OP_IF: u8 = 0x63;
/// OP_ELSE
const OP_ELSE: u8 = 0x67;
/// OP_ENDIF
const OP_ENDIF: u8 = 0x68;
/// OP_DROP
const OP_DROP: u8 = 0x75;
/// OP_HASH256 = SHA256(SHA256(x))（BIP199 HTLC 哈希锁常用操作码）。
const OP_HASH256: u8 = 0xaa;
/// OP_EQUALVERIFY
const OP_EQUALVERIFY: u8 = 0x88;
/// OP_CHECKSIG
const OP_CHECKSIG: u8 = 0xac;
/// OP_CHECKLOCKTIMEVERIFY（NOP2 重定义，BIP65）。
const OP_CHECKLOCKTIMEVERIFY: u8 = 0xb1;
/// 直接下一个字节为推送长度的上限（1..=75）。
const OP_PUSH_MAX_DIRECT: u8 = 0x4b;
/// CLTV 高度/时间阈值（BIP65/共识：< 500_000_000 为区块高度，否则为 Unix 秒）。
pub const LOCKTIME_TIME_THRESHOLD: u32 = 500_000_000;
/// 压缩公钥长度。
const COMPRESSED_PUBKEY_LEN: usize = 33;
/// 哈希锁摘要长度。
const HASH_LEN: usize = 32;

/// HTLC 处理中的具名错误（全 `HTLC_*` 前缀，不静默、不 panic）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HtlcError {
    /// preimage/payment_hash 不是 32 字节 hex。
    HtlcInvalidHash,
    /// SHA256(preimage) 与声明的 payment_hash 不符。
    HtlcHashMismatch,
    /// sats 金额为 0 或溢出（checked 算术失败）。
    HtlcInvalidAmount,
    /// offered + fee != total（金额不守恒）。
    HtlcSettlementMismatch,
    /// witnessScript / witness program 为空或 hex 非法。
    HtlcBadScriptHex,
    /// witnessProgram 不是 32 字节（segwit v0 P2WSH 程序长度必须为 32）。
    HtlcBadWitnessProgram,
    /// witnessProgram != SHA256(witnessScript)。
    HtlcWitnessProgramMismatch,
    /// witnessScript 不符合标准 HTLC 模板（操作码/推送顺序/长度不符）。
    HtlcMalformedScript,
    /// CLTV 锁定期推送非最小/非规范编码（前导零或符号位置位）。
    HtlcLocktimeNonCanonical,
    /// CLTV 锁定期为 0。
    HtlcInvalidLocktime,
    /// 在调用方给定的当前高度/时间下该 HTLC 已到期（接收方取款窗口已关闭）。
    HtlcLocktimeExpired,
    /// 超时安全裕度不足（剩余块数/秒数小于调用方要求的最小值）。
    HtlcTimeoutMarginTooSmall,
    /// 生产结算/广播未配置节点：本版不持钥、不签 HTLC、不广播。
    HtlcNodeNotConfigured,
}

impl std::fmt::Display for HtlcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let m = match self {
            HtlcError::HtlcInvalidHash => "HTLC_INVALID_HASH",
            HtlcError::HtlcHashMismatch => "HTLC_HASH_MISMATCH",
            HtlcError::HtlcInvalidAmount => "HTLC_INVALID_AMOUNT",
            HtlcError::HtlcSettlementMismatch => "HTLC_SETTLEMENT_MISMATCH",
            HtlcError::HtlcBadScriptHex => "HTLC_BAD_SCRIPT_HEX",
            HtlcError::HtlcBadWitnessProgram => "HTLC_BAD_WITNESS_PROGRAM",
            HtlcError::HtlcWitnessProgramMismatch => "HTLC_WITNESS_PROGRAM_MISMATCH",
            HtlcError::HtlcMalformedScript => "HTLC_MALFORMED_SCRIPT",
            HtlcError::HtlcLocktimeNonCanonical => "HTLC_LOCKTIME_NON_CANONICAL",
            HtlcError::HtlcInvalidLocktime => "HTLC_INVALID_LOCKTIME",
            HtlcError::HtlcLocktimeExpired => "HTLC_LOCKTIME_EXPIRED",
            HtlcError::HtlcTimeoutMarginTooSmall => "HTLC_TIMEOUT_MARGIN_TOO_SMALL",
            HtlcError::HtlcNodeNotConfigured => "HTLC_NODE_NOT_CONFIGURED",
        };
        f.write_str(m)
    }
}

impl std::error::Error for HtlcError {}

/// CLTV 绝对锁定期的单位（由数值阈值区分，与 Bitcoin nLockTime 语义一致）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocktimeUnits {
    /// 值 < 500_000_000：区块高度。
    BlockHeight,
    /// 值 >= 500_000_000：Unix 秒。
    UnixSeconds,
}

impl LocktimeUnits {
    pub fn as_str(self) -> &'static str {
        match self {
            LocktimeUnits::BlockHeight => "block_height",
            LocktimeUnits::UnixSeconds => "unix_seconds",
        }
    }
}

/// 从标准 HTLC witnessScript 解析出的确定性要素。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtlcScript {
    /// OP_HASH256 锁定的 32 字节摘要。
    pub payment_hash: [u8; HASH_LEN],
    /// 超时前凭原像取走资金的接收方压缩公钥（仅长度/位置校验，未验曲线点）。
    pub receiver_pubkey: [u8; COMPRESSED_PUBKEY_LEN],
    /// 超时后退款的发送方压缩公钥（仅长度/位置校验）。
    pub sender_pubkey: [u8; COMPRESSED_PUBKEY_LEN],
    /// CLTV 绝对锁定期整数值。
    pub locktime: u32,
    /// 锁定期单位。
    pub units: LocktimeUnits,
}

impl HtlcScript {
    pub fn payment_hash_hex(&self) -> String {
        hex::encode(self.payment_hash)
    }

    pub fn receiver_pubkey_hex(&self) -> String {
        hex::encode(self.receiver_pubkey)
    }

    pub fn sender_pubkey_hex(&self) -> String {
        hex::encode(self.sender_pubkey)
    }
}

/// 解析一个最小/规范编码的正整数推送（1..=4 字节小端）。
///
/// 内核采用的**确定性规范口径**（链下自洽，对齐 Bitcoin Script 最小 number 编码，
/// 但不实现完整解释器；锁定期限定为 `< 2^31` 的无符号正整数，覆盖区块高度与
/// Unix 秒的现实取值）：
/// - 长度 1..=4 字节（u32）；
/// - 小端序，**冗余零在高位（数组末尾）**：多字节推送的最高有效字节（末字节）
///   不得为 0，否则应使用更短编码（最低字节为 0 完全合法，如 256 = `[0x00,0x01]`）；
/// - 末字节 bit7 不得置位（否则该脚本数会被解释为负数，锁定期必须为正）；
/// - 解析值必须非 0。
fn parse_canonical_locktime_push(data: &[u8]) -> Result<u32, HtlcError> {
    if data.is_empty() || data.len() > 4 {
        return Err(HtlcError::HtlcLocktimeNonCanonical);
    }
    let last = data[data.len() - 1];
    // bit7 置位 → 脚本数为负，CLTV 锁定期不允许。
    if last & 0x80 != 0 {
        return Err(HtlcError::HtlcLocktimeNonCanonical);
    }
    // 多字节推送的最高有效字节不得为 0（非最小编码）。
    if data.len() > 1 && last == 0 {
        return Err(HtlcError::HtlcLocktimeNonCanonical);
    }
    let mut v: u32 = 0;
    for (i, b) in data.iter().enumerate() {
        v |= (*b as u32) << (8 * i);
    }
    if v == 0 {
        Err(HtlcError::HtlcInvalidLocktime)
    } else {
        Ok(v)
    }
}

/// 从 `pos` 读取一个直接长度推送（opcode 1..=75），返回 `(数据切片, 新位置)`。
fn read_direct_push(
    script: &[u8],
    pos: usize,
    expected_len: usize,
) -> Result<(&[u8], usize), HtlcError> {
    if pos >= script.len() {
        return Err(HtlcError::HtlcMalformedScript);
    }
    let op = script[pos];
    if !(1..=OP_PUSH_MAX_DIRECT).contains(&op) || op as usize != expected_len {
        return Err(HtlcError::HtlcMalformedScript);
    }
    let start = pos + 1;
    let end = start
        .checked_add(expected_len)
        .ok_or(HtlcError::HtlcMalformedScript)?;
    if end > script.len() {
        return Err(HtlcError::HtlcMalformedScript);
    }
    Ok((&script[start..end], end))
}

/// 读取一个操作码并断言相等。
fn read_op(script: &[u8], pos: usize, expected: u8) -> Result<usize, HtlcError> {
    if pos >= script.len() || script[pos] != expected {
        return Err(HtlcError::HtlcMalformedScript);
    }
    Ok(pos + 1)
}

/// 解析标准 HTLC witnessScript：
///
/// ```text
/// OP_IF
///   OP_HASH256 <32 payment_hash> OP_EQUALVERIFY OP_DROP <33 receiver_key> OP_CHECKSIG
/// OP_ELSE
///   <locktime> OP_CHECKLOCKTIMEVERIFY OP_DROP <33 sender_key> OP_CHECKSIG
/// OP_ENDIF
/// ```
///
/// 严格按模板逐字节匹配；任何多余字节、错误操作码、错误推送长度都具名拒绝。
pub fn parse_htlc_script(witness_script: &[u8]) -> Result<HtlcScript, HtlcError> {
    if witness_script.is_empty() {
        return Err(HtlcError::HtlcBadScriptHex);
    }
    let mut p = 0usize;
    p = read_op(witness_script, p, OP_IF)?;
    p = read_op(witness_script, p, OP_HASH256)?;
    let (hash_bytes, np) = read_direct_push(witness_script, p, HASH_LEN)?;
    p = np;
    let mut payment_hash = [0u8; HASH_LEN];
    payment_hash.copy_from_slice(hash_bytes);
    p = read_op(witness_script, p, OP_EQUALVERIFY)?;
    p = read_op(witness_script, p, OP_DROP)?;
    let (recv_bytes, np) = read_direct_push(witness_script, p, COMPRESSED_PUBKEY_LEN)?;
    p = np;
    let mut receiver_pubkey = [0u8; COMPRESSED_PUBKEY_LEN];
    receiver_pubkey.copy_from_slice(recv_bytes);
    p = read_op(witness_script, p, OP_CHECKSIG)?;
    p = read_op(witness_script, p, OP_ELSE)?;

    // CLTV 锁定期：直接长度推送（1..=75），取其数据段做规范整数解析。
    if p >= witness_script.len() {
        return Err(HtlcError::HtlcMalformedScript);
    }
    let push_op = witness_script[p];
    if !(1..=OP_PUSH_MAX_DIRECT).contains(&push_op) {
        return Err(HtlcError::HtlcMalformedScript);
    }
    let lt_len = push_op as usize;
    let lt_start = p + 1;
    let lt_end = lt_start
        .checked_add(lt_len)
        .ok_or(HtlcError::HtlcMalformedScript)?;
    if lt_end > witness_script.len() {
        return Err(HtlcError::HtlcMalformedScript);
    }
    let locktime = parse_canonical_locktime_push(&witness_script[lt_start..lt_end])?;
    p = lt_end;

    p = read_op(witness_script, p, OP_CHECKLOCKTIMEVERIFY)?;
    p = read_op(witness_script, p, OP_DROP)?;
    let (send_bytes, np) = read_direct_push(witness_script, p, COMPRESSED_PUBKEY_LEN)?;
    p = np;
    let mut sender_pubkey = [0u8; COMPRESSED_PUBKEY_LEN];
    sender_pubkey.copy_from_slice(send_bytes);
    p = read_op(witness_script, p, OP_CHECKSIG)?;
    p = read_op(witness_script, p, OP_ENDIF)?;
    if p != witness_script.len() {
        // 模板之后还有多余字节 → 不是本内核认可的标准 HTLC。
        return Err(HtlcError::HtlcMalformedScript);
    }

    let units = if locktime < LOCKTIME_TIME_THRESHOLD {
        LocktimeUnits::BlockHeight
    } else {
        LocktimeUnits::UnixSeconds
    };
    Ok(HtlcScript {
        payment_hash,
        receiver_pubkey,
        sender_pubkey,
        locktime,
        units,
    })
}

/// 计算 P2WSH witness program = `SHA256(witnessScript)`（32 字节）。
pub fn p2wsh_program(witness_script: &[u8]) -> [u8; 32] {
    Sha256::digest(witness_script).into()
}

/// hex -> bytes（仅偶数长度、ASCII hex；非法返回具名错误，不 panic）。
fn hex_decode(s: &str) -> Result<Vec<u8>, HtlcError> {
    let h = s.strip_prefix("0x").unwrap_or(s);
    if !h.len().is_multiple_of(2) || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(HtlcError::HtlcBadScriptHex);
    }
    (0..h.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&h[i..i + 2], 16).map_err(|_| HtlcError::HtlcBadScriptHex))
        .collect()
}

/// P2WSH witness program 必须为 32 字节。
fn parse_program(program_hex: &str) -> Result<[u8; 32], HtlcError> {
    let raw = hex_decode(program_hex)?;
    if raw.len() != 32 {
        return Err(HtlcError::HtlcBadWitnessProgram);
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&raw);
    Ok(out)
}

/// 哈希锁校验：`SHA256(preimage) == payment_hash`。
pub fn verify_hashlock(preimage_hex: &str, payment_hash_hex: &str) -> Result<(), HtlcError> {
    let preimage = Preimage::from_hex(preimage_hex).ok_or(HtlcError::HtlcInvalidHash)?;
    let expected = PaymentHash::from_hex(payment_hash_hex).ok_or(HtlcError::HtlcInvalidHash)?;
    if preimage.payment_hash() != expected {
        Err(HtlcError::HtlcHashMismatch)
    } else {
        Ok(())
    }
}

/// 金额守恒：`offered_sats + fee_sats == total_sats`，且三项为正、checked 不溢出。
pub fn verify_amount_conservation(
    offered_sats: u64,
    fee_sats: u64,
    total_sats: u64,
) -> Result<(), HtlcError> {
    if offered_sats == 0 || total_sats == 0 {
        return Err(HtlcError::HtlcInvalidAmount);
    }
    let sum = offered_sats
        .checked_add(fee_sats)
        .ok_or(HtlcError::HtlcInvalidAmount)?;
    if sum != total_sats {
        Err(HtlcError::HtlcSettlementMismatch)
    } else {
        Ok(())
    }
}

/// 超时锁校验：在调用方给定的当前高度/时间下 HTLC 必须仍未到期，且剩余裕度足够。
///
/// `current` 单位必须与 CLTV 锁定期单位一致（高度对高度、秒对秒）；内核不读时钟。
/// `min_remaining` 为调用方要求的最小安全裕度（同单位），0 表示不检查裕度。
pub fn verify_timeout(
    locktime: u32,
    units: LocktimeUnits,
    current: u32,
    min_remaining: u32,
) -> Result<(), HtlcError> {
    if locktime == 0 {
        return Err(HtlcError::HtlcInvalidLocktime);
    }
    // 已经到达/超过锁定期 → 接收方凭原像取款的窗口已关闭。
    if current >= locktime {
        return Err(HtlcError::HtlcLocktimeExpired);
    }
    let remaining = locktime - current;
    if remaining < min_remaining {
        return Err(HtlcError::HtlcTimeoutMarginTooSmall);
    }
    let _ = units;
    Ok(())
}

/// 一次 HTLC 链下校验的全部输入（金额/哈希/脚本/时间均由调用方取证后显式传入）。
#[derive(Debug, Clone, serde::Deserialize)]
pub struct HtlcOffer {
    pub offered_sats: u64,
    pub fee_sats: u64,
    pub total_input_sats: u64,
    pub preimage_hex: String,
    pub payment_hash_hex: String,
    pub witness_program_hex: String,
    pub witness_script_hex: String,
    /// 当前链高度或 Unix 秒（须与锁定期单位一致）。
    pub current: u32,
    /// 要求的最小剩余裕度（同单位，0 表示不检查）。
    pub min_remaining: u32,
}

/// 校验通过后回传的确定性证据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtlcReceipt {
    pub payment_hash_hex: String,
    pub receiver_pubkey_hex: String,
    pub sender_pubkey_hex: String,
    pub locktime: u32,
    pub units: &'static str,
    pub offered_sats: u64,
    pub fee_sats: u64,
    pub total_input_sats: u64,
    pub witness_program_matches: bool,
}

impl HtlcOffer {
    /// 依次执行金额守恒 → 脚本结构 → witness program → 哈希锁 → 超时锁全部门禁。
    pub fn validate(&self) -> Result<HtlcReceipt, HtlcError> {
        verify_amount_conservation(self.offered_sats, self.fee_sats, self.total_input_sats)?;

        let script_bytes = hex_decode(&self.witness_script_hex)?;
        let parsed = parse_htlc_script(&script_bytes)?;

        // 脚本内嵌的 payment_hash 必须与调用方声明一致。
        if hex::encode(parsed.payment_hash)
            != self
                .payment_hash_hex
                .strip_prefix("0x")
                .unwrap_or(&self.payment_hash_hex)
        {
            return Err(HtlcError::HtlcHashMismatch);
        }

        let program = parse_program(&self.witness_program_hex)?;
        if p2wsh_program(&script_bytes) != program {
            return Err(HtlcError::HtlcWitnessProgramMismatch);
        }

        verify_hashlock(&self.preimage_hex, &self.payment_hash_hex)?;
        verify_timeout(
            parsed.locktime,
            parsed.units,
            self.current,
            self.min_remaining,
        )?;

        Ok(HtlcReceipt {
            payment_hash_hex: parsed.payment_hash_hex(),
            receiver_pubkey_hex: parsed.receiver_pubkey_hex(),
            sender_pubkey_hex: parsed.sender_pubkey_hex(),
            locktime: parsed.locktime,
            units: parsed.units.as_str(),
            offered_sats: self.offered_sats,
            fee_sats: self.fee_sats,
            total_input_sats: self.total_input_sats,
            witness_program_matches: true,
        })
    }
}

/// 生产结算/广播占位：本版不连比特币节点、不持私钥、不签 HTLC，一律 fail-closed。
pub fn finalize_htlc() -> Result<(), HtlcError> {
    Err(HtlcError::HtlcNodeNotConfigured)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_script(
        payment_hash: &[u8; 32],
        recv: &[u8; 33],
        send: &[u8; 33],
        locktime: &[u8],
    ) -> Vec<u8> {
        let mut s = Vec::new();
        s.push(OP_IF);
        s.push(OP_HASH256);
        s.push(0x20);
        s.extend_from_slice(payment_hash);
        s.push(OP_EQUALVERIFY);
        s.push(OP_DROP);
        s.push(0x21);
        s.extend_from_slice(recv);
        s.push(OP_CHECKSIG);
        s.push(OP_ELSE);
        s.push(locktime.len() as u8);
        s.extend_from_slice(locktime);
        s.push(OP_CHECKLOCKTIMEVERIFY);
        s.push(OP_DROP);
        s.push(0x21);
        s.extend_from_slice(send);
        s.push(OP_CHECKSIG);
        s.push(OP_ENDIF);
        s
    }

    fn min_le(mut v: u32) -> Vec<u8> {
        let mut out = Vec::new();
        while v > 0 {
            out.push((v & 0xff) as u8);
            v >>= 8;
        }
        out
    }

    #[test]
    fn parses_canonical_htlc_script() {
        let pre = [9u8; 32];
        let hash: [u8; 32] = Sha256::digest(pre).into();
        let recv = [2u8; 33];
        let send = [3u8; 33];
        // 700_000 区块高度，最小 LE 编码为 3 字节 [0x60,0xAE,0x0A]。
        let script = sample_script(&hash, &recv, &send, &min_le(700_000));
        let parsed = parse_htlc_script(&script).unwrap();
        assert_eq!(parsed.payment_hash, hash);
        assert_eq!(parsed.receiver_pubkey, recv);
        assert_eq!(parsed.sender_pubkey, send);
        assert_eq!(parsed.locktime, 700_000);
        assert_eq!(parsed.units, LocktimeUnits::BlockHeight);
        // witness program = SHA256(script)。
        assert_eq!(p2wsh_program(&script), Sha256::digest(&script).as_slice());
    }

    #[test]
    fn distinguishes_time_vs_height_units() {
        let hash = [0u8; 32];
        let k = [0u8; 33];
        let height = sample_script(&hash, &k, &k, &min_le(100));
        assert_eq!(
            parse_htlc_script(&height).unwrap().units,
            LocktimeUnits::BlockHeight
        );
        // >= 500_000_000 => Unix 秒（取一个不置符号位、最小 4 字节的值）。
        let t: u32 = 1_900_000_000;
        let time = sample_script(&hash, &k, &k, &min_le(t));
        assert_eq!(
            parse_htlc_script(&time).unwrap().units,
            LocktimeUnits::UnixSeconds
        );
    }

    #[test]
    fn rejects_malformed_and_noncanonical_scripts() {
        let hash = [1u8; 32];
        let k = [2u8; 33];
        let mut s = sample_script(&hash, &k, &k, &min_le(700));
        assert!(parse_htlc_script(&s).is_ok());

        // 改第一个操作码（OP_IF -> 别的）。
        s[0] = OP_ELSE;
        assert_eq!(
            parse_htlc_script(&s).err(),
            Some(HtlcError::HtlcMalformedScript)
        );
        let mut s = sample_script(&hash, &k, &k, &min_le(700));

        // 尾部多一个字节。
        s.push(0x00);
        assert_eq!(
            parse_htlc_script(&s).err(),
            Some(HtlcError::HtlcMalformedScript)
        );

        // 高位冗余零的非最小编码：值=2 却用 4 字节 [0x02,0x00,0x00,0x00]（末字节 0）。
        let bad = sample_script(&hash, &k, &k, &[0x02, 0x00, 0x00, 0x00]);
        assert_eq!(
            parse_htlc_script(&bad).err(),
            Some(HtlcError::HtlcLocktimeNonCanonical)
        );
        // 锁定期 = 0（单字节 0x00）。
        let zero = sample_script(&hash, &k, &k, &[0x00]);
        assert_eq!(
            parse_htlc_script(&zero).err(),
            Some(HtlcError::HtlcInvalidLocktime)
        );
        // 符号位置位（末字节 0x80，脚本数为负）。
        let neg = sample_script(&hash, &k, &k, &[0x00, 0x00, 0x00, 0x80]);
        assert_eq!(
            parse_htlc_script(&neg).err(),
            Some(HtlcError::HtlcLocktimeNonCanonical)
        );
        // 低位字节为 0 但属合法最小编码：256 = [0x00,0x01]。
        let low_zero = sample_script(&hash, &k, &k, &[0x00, 0x01]);
        assert_eq!(parse_htlc_script(&low_zero).unwrap().locktime, 256);
    }

    #[test]
    fn hashlock_amount_timeout_and_program_gates() {
        let pre = [9u8; 32];
        let pre_hex = hex::encode(pre);
        let hash: [u8; 32] = Sha256::digest(pre).into();
        let hash_hex = hex::encode(hash);
        let recv = [2u8; 33];
        let send = [3u8; 33];
        let script = sample_script(&hash, &recv, &send, &min_le(700_000));
        let program = p2wsh_program(&script);

        let ok = HtlcOffer {
            offered_sats: 99_000,
            fee_sats: 1_000,
            total_input_sats: 100_000,
            preimage_hex: pre_hex.clone(),
            payment_hash_hex: hash_hex.clone(),
            witness_program_hex: hex::encode(program),
            witness_script_hex: hex::encode(&script),
            current: 690_000,
            min_remaining: 1_000,
        };
        let receipt = ok.validate().unwrap();
        assert!(receipt.witness_program_matches);
        assert_eq!(receipt.locktime, 700_000);

        // 金额不守恒。
        let mut bad = ok.clone();
        bad.total_input_sats = 100_001;
        assert_eq!(
            bad.validate().err(),
            Some(HtlcError::HtlcSettlementMismatch)
        );
        // 零金额。
        let mut bad = ok.clone();
        bad.offered_sats = 0;
        assert_eq!(bad.validate().err(), Some(HtlcError::HtlcInvalidAmount));
        // 哈希锁不符。
        let mut bad = ok.clone();
        bad.preimage_hex = hex::encode([0u8; 32]);
        assert_eq!(bad.validate().err(), Some(HtlcError::HtlcHashMismatch));
        // witness program 不符。
        let mut bad = ok.clone();
        bad.witness_program_hex = hex::encode([0u8; 32]);
        assert_eq!(
            bad.validate().err(),
            Some(HtlcError::HtlcWitnessProgramMismatch)
        );
        // witness program 长度错。
        let mut bad = ok.clone();
        bad.witness_program_hex = hex::encode([0u8; 31]);
        assert_eq!(bad.validate().err(), Some(HtlcError::HtlcBadWitnessProgram));
        // 已到期。
        let mut bad = ok.clone();
        bad.current = 700_000;
        assert_eq!(bad.validate().err(), Some(HtlcError::HtlcLocktimeExpired));
        // 裕度不足。
        let mut bad = ok.clone();
        bad.min_remaining = 20_000;
        assert_eq!(
            bad.validate().err(),
            Some(HtlcError::HtlcTimeoutMarginTooSmall)
        );
        // 脚本内嵌 hash 与声明 hash 不一致。
        let other_hash = [7u8; 32];
        let script2 = sample_script(&other_hash, &recv, &send, &min_le(700_000));
        let prog2 = p2wsh_program(&script2);
        let mut bad = ok.clone();
        bad.witness_program_hex = hex::encode(prog2);
        bad.witness_script_hex = hex::encode(script2);
        assert_eq!(bad.validate().err(), Some(HtlcError::HtlcHashMismatch));
    }

    #[test]
    fn bad_hex_and_finalize_fail_closed() {
        assert_eq!(
            verify_hashlock("zz", &hex::encode([0u8; 32])).err(),
            Some(HtlcError::HtlcInvalidHash)
        );
        // 非法/奇数长度 hex。
        assert!(hex_decode("xyz").is_err());
        assert!(hex_decode("abc").is_err());
        // 空脚本。
        assert_eq!(
            parse_htlc_script(&[]).err(),
            Some(HtlcError::HtlcBadScriptHex)
        );
        // 生产结算一律 fail-closed。
        assert_eq!(
            finalize_htlc().err(),
            Some(HtlcError::HtlcNodeNotConfigured)
        );
    }
}
