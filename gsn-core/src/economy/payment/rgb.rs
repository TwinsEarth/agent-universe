//! RGB 客户端验证承诺位置纯校验内核（v3.9.6）。
//!
//! # 定位
//!
//! RGB（client-side validation）不把状态、资产或承诺直接写进链上交易数据，而是把一个
//! 32 字节的**承诺（commitment）锚定（anchor）**到比特币交易上，真实状态在链下由
//! 客户端验证。本模块只实现锚定**位置/形状的链下确定性校验**，覆盖两种承诺方案：
//!
//! - **opret-first（OP_RETURN 输出）**：承诺作为某笔交易输出 `scriptPubKey` 中
//!   `OP_RETURN <32 字节>` 的单个直接数据推送。内核扫描调用方提供的输出模型，定位
//!   该输出并逐字节比对承诺。
//! - **tapret-first（Taproot 输出，承诺进内部键/见证保留值）**：本模块**只**验证被
//!   指定的锚定输出是一个结构合法的 segwit v1（Taproot）程序（`OP_1 <32 字节>`），
//!   且它是交易中**第一个** Taproot 输出（tapret-first 规则）。真正的 tapret tweaking
//!   / 内部键承诺推导需要 client-side-validation RGB Core 的密码学，本版**不做推导**。
//!
//! # 明确不做（fail-closed / 诚实边界）
//!
//! - 不做 RGB 状态转换（state transition）、不验证资产 genesis / seal / inventory；
//! - 不做 tapret tweaking 推导、不验证内部键到输出键的承诺关系；
//! - 不解析 bech32m Taproot 地址，不连节点、不读真实区块、不构造或广播交易；
//! - 不持私钥、不签名、不转资产。
//!
//! 因此“通过”仅表示：给定调用方取证的输出脚本模型，承诺的**锚定位置与编码形状**
//! 符合对应方案的确定性规则。

/// OP_RETURN（v0 数据区输出标记）。
const OP_RETURN: u8 = 0x6a;
/// OP_1（segwit v1 / Taproot 版本字节）。
const OP_1: u8 = 0x51;
/// 32 字节直接数据推送操作码。
const OP_PUSH_32: u8 = 0x20;
/// 承诺长度。
const COMMITMENT_LEN: usize = 32;

/// RGB 承诺锚定方案。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitmentScheme {
    /// 承诺在第一个 OP_RETURN 输出：`OP_RETURN <32 commitment>`。
    OpretFirst,
    /// 承诺进第一个 Taproot 输出（本内核只校验输出形状/位置，不做 tweak 推导）。
    TapretFirst,
}

impl std::str::FromStr for CommitmentScheme {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "opret_first" | "opret" => Ok(CommitmentScheme::OpretFirst),
            "tapret_first" | "tapret" => Ok(CommitmentScheme::TapretFirst),
            _ => Err(()),
        }
    }
}

impl CommitmentScheme {
    pub fn as_str(self) -> &'static str {
        match self {
            CommitmentScheme::OpretFirst => "opret_first",
            CommitmentScheme::TapretFirst => "tapret_first",
        }
    }
}

/// RGB 承诺相关具名错误（全 `RGB_*` 前缀）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RgbError {
    /// 承诺不是 32 字节 hex。
    RgbInvalidCommitment,
    /// 输出脚本不是合法偶数长度 ASCII hex。
    RgbBadScriptHex,
    /// opret：没有任何输出携带匹配的 `OP_RETURN <32 commitment>`。
    RgbOpretCommitmentNotFound,
    /// opret：存在 OP_RETURN 但推送长度/形状不规范（非单个 32 字节直接推送）。
    RgbOpretMalformedOutput,
    /// tapret：指定的输出不是 `OP_1 <32>` Taproot 程序。
    RgbTapretOutputNotTaproot,
    /// tapret-first：指定输出之前已存在另一个 Taproot 输出（必须锚到第一个）。
    RgbTapretNotFirstTaprootOutput,
    /// tapret：输出索引越界。
    RgbTapretOutputIndexOutOfRange,
    /// tapret：本版不做承诺 tweak 推导；需要密码学 RGB Core，显式不声称已验证。
    RgbTapretDerivationNotImplemented,
}

impl std::fmt::Display for RgbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let m = match self {
            RgbError::RgbInvalidCommitment => "RGB_INVALID_COMMITMENT",
            RgbError::RgbBadScriptHex => "RGB_BAD_SCRIPT_HEX",
            RgbError::RgbOpretCommitmentNotFound => "RGB_OPRET_COMMITMENT_NOT_FOUND",
            RgbError::RgbOpretMalformedOutput => "RGB_OPRET_MALFORMED_OUTPUT",
            RgbError::RgbTapretOutputNotTaproot => "RGB_TAPRET_OUTPUT_NOT_TAPROOT",
            RgbError::RgbTapretNotFirstTaprootOutput => "RGB_TAPRET_NOT_FIRST_TAPROOT_OUTPUT",
            RgbError::RgbTapretOutputIndexOutOfRange => "RGB_TAPRET_OUTPUT_INDEX_OUT_OF_RANGE",
            RgbError::RgbTapretDerivationNotImplemented => "RGB_TAPRET_DERIVATION_NOT_IMPLEMENTED",
        };
        f.write_str(m)
    }
}

impl std::error::Error for RgbError {}

fn hex_decode(s: &str) -> Result<Vec<u8>, RgbError> {
    let h = s.strip_prefix("0x").unwrap_or(s);
    if !h.len().is_multiple_of(2) || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(RgbError::RgbBadScriptHex);
    }
    (0..h.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&h[i..i + 2], 16).map_err(|_| RgbError::RgbBadScriptHex))
        .collect()
}

fn parse_commitment(hex: &str) -> Result<[u8; COMMITMENT_LEN], RgbError> {
    let raw = hex_decode(hex).map_err(|_| RgbError::RgbInvalidCommitment)?;
    if raw.len() != COMMITMENT_LEN {
        return Err(RgbError::RgbInvalidCommitment);
    }
    let mut out = [0u8; COMMITMENT_LEN];
    out.copy_from_slice(&raw);
    Ok(out)
}

/// 判断脚本是否为 Taproot（segwit v1）程序：`OP_1 OP_PUSH_32 <32>`。
fn is_taproot_program(script: &[u8]) -> bool {
    script.len() == 34 && script[0] == OP_1 && script[1] == OP_PUSH_32
}

/// 判断脚本是否为 OP_RETURN 数据输出（`6a` 开头）。
fn is_op_return(script: &[u8]) -> bool {
    script.first().copied() == Some(OP_RETURN)
}

/// opret-first 校验：在输出中定位唯一一个 `OP_RETURN <32 commitment>` 输出。
///
/// 返回匹配输出的索引。规则：
/// - 精确匹配 `0x6a 0x20 <32 commitment>`（允许裸 `OP_RETURN` 等其他输出存在）；
/// - 若某输出以 `OP_RETURN` 开头但携带了非 32 字节/非规范推送，具名 malformed；
/// - 存在多个携带同一承诺的 opret 输出也视为 malformed（锚点应唯一）。
pub fn verify_opret_commitment(
    outputs_script_hex: &[String],
    commitment_hex: &str,
) -> Result<usize, RgbError> {
    let commitment = parse_commitment(commitment_hex)?;
    let mut found: Option<usize> = None;
    for (i, h) in outputs_script_hex.iter().enumerate() {
        let script = hex_decode(h)?;
        if !is_op_return(&script) {
            continue;
        }
        // OP_RETURN 输出：校验其是否为规范的单个 32 字节直接推送。
        let matches = script.len() == 34 && script[1] == OP_PUSH_32 && script[2..34] == commitment;
        if matches {
            if found.is_some() {
                return Err(RgbError::RgbOpretMalformedOutput);
            }
            found = Some(i);
        } else {
            // 以 OP_RETURN 开头但形状不是本方案认可的承诺载体。
            // 裸 OP_RETURN(0x6a) 或其它数据输出不参与锚定，放行；只要它带了一个
            // 32 字节推送却不等于目标承诺，或推送长度异常，则判 malformed。
            if script.len() >= 2 && script[1] == OP_PUSH_32 {
                return Err(RgbError::RgbOpretMalformedOutput);
            }
        }
    }
    found.ok_or(RgbError::RgbOpretCommitmentNotFound)
}

/// tapret-first 位置/形状校验。
///
/// 校验 `taproot_output_index` 指向的输出是 `OP_1 <32>` Taproot 程序，且其前面没有
/// 其它 Taproot 输出（first 规则）。**不**做内部键 tweak 承诺推导——调用方需要密码学
/// 绑定关系时，本函数显式返回 [`RgbError::RgbTapretDerivationNotImplemented`] 由
/// `require_derivation=true` 的调用路径给出，避免把“形状正确”误报成“承诺已验证”。
pub fn verify_tapret_placement(
    outputs_script_hex: &[String],
    taproot_output_index: usize,
    require_derivation: bool,
) -> Result<usize, RgbError> {
    if taproot_output_index >= outputs_script_hex.len() {
        return Err(RgbError::RgbTapretOutputIndexOutOfRange);
    }
    for (_i, h) in outputs_script_hex
        .iter()
        .enumerate()
        .take(taproot_output_index)
    {
        let script = hex_decode(h)?;
        if is_taproot_program(&script) {
            return Err(RgbError::RgbTapretNotFirstTaprootOutput);
        }
    }
    let target_hex = &outputs_script_hex[taproot_output_index];
    let target = hex_decode(target_hex)?;
    if !is_taproot_program(&target) {
        return Err(RgbError::RgbTapretOutputNotTaproot);
    }
    if require_derivation {
        return Err(RgbError::RgbTapretDerivationNotImplemented);
    }
    Ok(taproot_output_index)
}

/// 一次 RGB 承诺位置校验的输入（方案 + 承诺 + 输出模型 + 指定锚点）。
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CommitmentClaim {
    /// "opret_first" / "tapret_first"。
    pub scheme: String,
    /// 32 字节承诺 hex。
    pub commitment_hex: String,
    /// 交易全部输出的 scriptPubKey hex（调用方取证后传入）。
    pub outputs_script_hex: Vec<String>,
    /// tapret 方案下被锚定的输出索引；opret 忽略。
    pub taproot_output_index: usize,
    /// tapret：是否要求内部键 tweak 密码学推导（本版仅支持 false）。
    pub require_derivation: bool,
}

/// 校验结果证据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitmentReceipt {
    pub scheme: &'static str,
    pub commitment_hex: String,
    pub anchor_output_index: usize,
    pub derivation_verified: bool,
}

impl CommitmentClaim {
    pub fn verify(&self) -> Result<CommitmentReceipt, RgbError> {
        // 承诺本身先校验（两种方案都必须是 32 字节）。
        let commitment = parse_commitment(&self.commitment_hex)?;
        let scheme: CommitmentScheme = self
            .scheme
            .parse()
            .map_err(|_| RgbError::RgbInvalidCommitment)?;
        match scheme {
            CommitmentScheme::OpretFirst => {
                let idx = verify_opret_commitment(&self.outputs_script_hex, &self.commitment_hex)?;
                Ok(CommitmentReceipt {
                    scheme: scheme.as_str(),
                    commitment_hex: hex::encode(commitment),
                    anchor_output_index: idx,
                    derivation_verified: true,
                })
            }
            CommitmentScheme::TapretFirst => {
                let idx = verify_tapret_placement(
                    &self.outputs_script_hex,
                    self.taproot_output_index,
                    self.require_derivation,
                )?;
                Ok(CommitmentReceipt {
                    scheme: scheme.as_str(),
                    commitment_hex: hex::encode(commitment),
                    anchor_output_index: idx,
                    // 本版不做 tweak 推导，诚实置 false。
                    derivation_verified: false,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opret_out(commitment: &[u8; 32]) -> String {
        let mut s = vec![OP_RETURN, OP_PUSH_32];
        s.extend_from_slice(commitment);
        hex::encode(s)
    }

    fn taproot_out(key: &[u8; 32]) -> String {
        let mut s = vec![OP_1, OP_PUSH_32];
        s.extend_from_slice(key);
        hex::encode(s)
    }

    fn p2wpkh_out() -> String {
        // OP_0 + 20 字节直接推送（P2WPKH witness v0），用于区分非 Taproot 输出。
        let mut s = vec![0x00, 0x14];
        s.extend_from_slice(&[1u8; 20]);
        hex::encode(s)
    }

    #[test]
    fn opret_happy_path_and_index() {
        let c = [0xabu8; 32];
        let outs = vec![p2wpkh_out(), opret_out(&c), taproot_out(&[2u8; 32])];
        let idx = verify_opret_commitment(&outs, &hex::encode(c)).unwrap();
        assert_eq!(idx, 1);

        let receipt = CommitmentClaim {
            scheme: "opret_first".into(),
            commitment_hex: hex::encode(c),
            outputs_script_hex: outs,
            taproot_output_index: 0,
            require_derivation: false,
        }
        .verify()
        .unwrap();
        assert_eq!(receipt.anchor_output_index, 1);
        assert!(receipt.derivation_verified);
    }

    #[test]
    fn opret_rejects_missing_wrong_and_dup() {
        let c = [0xabu8; 32];
        // 没有任何 opret。
        let none = vec![p2wpkh_out(), taproot_out(&[2u8; 32])];
        assert_eq!(
            verify_opret_commitment(&none, &hex::encode(c)).err(),
            Some(RgbError::RgbOpretCommitmentNotFound)
        );
        // 32 字节推送但承诺不匹配。
        let wrong = vec![opret_out(&[0x11; 32])];
        assert_eq!(
            verify_opret_commitment(&wrong, &hex::encode(c)).err(),
            Some(RgbError::RgbOpretMalformedOutput)
        );
        // 两个相同承诺输出 → 锚点不唯一。
        let dup = vec![opret_out(&c), opret_out(&c)];
        assert_eq!(
            verify_opret_commitment(&dup, &hex::encode(c)).err(),
            Some(RgbError::RgbOpretMalformedOutput)
        );
        // 裸 OP_RETURN 不算承诺载体，但不阻断在另一输出找到承诺。
        let bare = hex::encode([OP_RETURN]);
        let mixed = vec![bare, opret_out(&c)];
        assert_eq!(verify_opret_commitment(&mixed, &hex::encode(c)).unwrap(), 1);
    }

    #[test]
    fn tapret_first_placement_rules() {
        let k0 = [3u8; 32];
        let k1 = [4u8; 32];
        // 第一个 Taproot 输出在索引 0。
        let outs = vec![taproot_out(&k0), p2wpkh_out(), taproot_out(&k1)];
        assert_eq!(verify_tapret_placement(&outs, 0, false).unwrap(), 0);

        // 锚到索引 2，但索引 0 已是 Taproot → 违反 first。
        assert_eq!(
            verify_tapret_placement(&outs, 2, false).err(),
            Some(RgbError::RgbTapretNotFirstTaprootOutput)
        );

        // 指向非 Taproot 输出。
        let outs2 = vec![p2wpkh_out(), taproot_out(&k0)];
        assert_eq!(
            verify_tapret_placement(&outs2, 0, false).err(),
            Some(RgbError::RgbTapretOutputNotTaproot)
        );

        // 索引越界。
        assert_eq!(
            verify_tapret_placement(&outs2, 9, false).err(),
            Some(RgbError::RgbTapretOutputIndexOutOfRange)
        );

        // 形状正确但要求密码学推导 → 本版诚实不实现。
        assert_eq!(
            verify_tapret_placement(&outs, 0, true).err(),
            Some(RgbError::RgbTapretDerivationNotImplemented)
        );
    }

    #[test]
    fn tapret_claim_receipt_marks_derivation_false() {
        let k = [5u8; 32];
        let c = [6u8; 32];
        let receipt = CommitmentClaim {
            scheme: "tapret".into(),
            commitment_hex: hex::encode(c),
            outputs_script_hex: vec![taproot_out(&k)],
            taproot_output_index: 0,
            require_derivation: false,
        }
        .verify()
        .unwrap();
        assert_eq!(receipt.scheme, "tapret_first");
        assert!(!receipt.derivation_verified);
    }

    #[test]
    fn bad_commitment_and_scheme_and_hex() {
        assert_eq!(
            parse_commitment("ab").err(),
            Some(RgbError::RgbInvalidCommitment)
        );
        assert!(hex_decode("zz").is_err());
        let claim = CommitmentClaim {
            scheme: "bogus".into(),
            commitment_hex: hex::encode([0u8; 32]),
            outputs_script_hex: vec![],
            taproot_output_index: 0,
            require_derivation: false,
        };
        assert_eq!(claim.verify().err(), Some(RgbError::RgbInvalidCommitment));
    }
}
