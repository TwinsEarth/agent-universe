//! L402 / 比特币闪电网络纯协议内核（v3.9.3）。
//!
//! # 定位
//!
//! L402（原 LSAT）把 HTTP `402 Payment Required` 与闪电发票绑定：服务端在
//! `WWW-Authenticate: L402 macaroon="…", invoice="lnbc…"` 中给出不透明 macaroon 与
//! BOLT11 发票；客户端支付后拿到支付原像 preimage，再以
//! `Authorization: L402 <macaroon_b64>:<preimage_hex>` 重放请求。服务端校验
//! `SHA256(preimage) == payment_hash` 即视为已支付。
//!
//! 与 v3.9.2 的 EVM x402 一致，本模块是**沙盒可安全承担的纯协议面**：只做确定性的
//! 头解析、金额前缀解析、原像/哈希关系与精确金额守恒，绝不：
//!
//! - 连接任何闪电节点 / LND / Eclair / LDK，不发起或结算真实 HTLC；
//! - 持私钥、签名、广播、划转；
//! - 解码/校验 BOLT11 的 bech32 数据段（内含节点签名与 payment_hash）；
//! - 校验 macaroon 签名（那需要服务端 root key）。
//!
//! 因此调用方必须从**受信任的闪电节点/发票解码服务**取得发票对应的 `payment_hash`
//! （32 字节）后传入本内核；内核负责的是“给定可信 payment_hash，客户端提交的
//! preimage 是否解出同一哈希、金额是否精确匹配、票据是否与挑战一致”。
//!
//! 金额用整数 **msats（u64）**，全程不使用浮点。外部报道的闪电费率/采用量数字均为
//! 第三方口径，非本仓复测，内核不内置。

use sha2::{Digest, Sha256};

/// L402 方案名（RFC 7235 auth-scheme 大小写不敏感，统一以大写比较）。
pub const L402_SCHEME: &str = "L402";

/// 32 字节支付哈希 / 原像。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PaymentHash(pub [u8; 32]);

impl PaymentHash {
    /// 从 0x 可选前缀的小写/大写 hex 解析 32 字节；非法长度/字符返回 None（生产路径不 panic）。
    pub fn from_hex(s: &str) -> Option<Self> {
        let h = s.strip_prefix("0x").unwrap_or(s);
        if h.len() != 64 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let mut out = [0u8; 32];
        for i in 0..32 {
            out[i] = u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).ok()?;
        }
        Some(PaymentHash(out))
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

/// 支付原像（客户端支付后从闪电网络获得，必须保密且一次性）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Preimage(pub [u8; 32]);

impl Preimage {
    pub fn from_hex(s: &str) -> Option<Self> {
        PaymentHash::from_hex(s).map(|h| Preimage(h.0))
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// BOLT：payment_hash = SHA256(preimage)。
    pub fn payment_hash(&self) -> PaymentHash {
        PaymentHash(Sha256::digest(self.0).into())
    }
}

/// L402 处理过程中的具名错误（全 `L402_*`，不静默、不 panic）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum L402Error {
    /// 402 挑战缺少 L402 scheme 或格式不符。
    L402MalformedChallenge,
    /// 缺少 macaroon/invoice 参数，或引号配对错误。
    L402MissingParam,
    /// macaroon 为空或两次出现的 macaroon 不一致（凭证不匹配挑战）。
    L402MacaroonMismatch,
    /// 发票前缀不是受支持的 BOLT11 网络（lnbc/lntb/lnbcrt）。
    L402UnsupportedInvoiceNetwork,
    /// 金额段为空、含非数字、或 multiplier 非法。
    L402InvalidAmount,
    /// pico-BTC 金额无法表示为整数 msat（必须为 10 的倍数）。
    L402SubMsatAmount,
    /// 金额溢出 u64 msat。
    L402AmountOverflow,
    /// preimage 不是 32 字节 hex。
    L402InvalidPreimage,
    /// SHA256(preimage) 与预期 payment_hash 不符（未支付/伪造原像）。
    L402PreimageHashMismatch,
    /// 实付金额与应付金额不守恒（多付/少付都拒绝，不静默接受差额）。
    L402SettlementMismatch,
}

impl std::fmt::Display for L402Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let m = match self {
            L402Error::L402MalformedChallenge => "L402_MALFORMED_CHALLENGE",
            L402Error::L402MissingParam => "L402_MISSING_PARAM",
            L402Error::L402MacaroonMismatch => "L402_MACAROON_MISMATCH",
            L402Error::L402UnsupportedInvoiceNetwork => "L402_UNSUPPORTED_INVOICE_NETWORK",
            L402Error::L402InvalidAmount => "L402_INVALID_AMOUNT",
            L402Error::L402SubMsatAmount => "L402_SUB_MSAT_AMOUNT",
            L402Error::L402AmountOverflow => "L402_AMOUNT_OVERFLOW",
            L402Error::L402InvalidPreimage => "L402_INVALID_PREIMAGE",
            L402Error::L402PreimageHashMismatch => "L402_PREIMAGE_HASH_MISMATCH",
            L402Error::L402SettlementMismatch => "L402_SETTLEMENT_MISMATCH",
        };
        f.write_str(m)
    }
}

impl std::error::Error for L402Error {}

/// 从 402 挑战头解析出的 L402 票据。
///
/// `macaroon` 为**原样不透明字符串**（通常 base64url），本内核不解释其 caveat、
/// 不校验其 HMAC 签名；`invoice` 为 BOLT11 字符串，只解析其人类可读金额前缀。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct L402Challenge {
    pub macaroon: String,
    pub invoice: String,
}

impl L402Challenge {
    /// 解析 `WWW-Authenticate` 头值，形如：
    /// `L402 macaroon="AG...", invoice="lnbc100n1p..."`。
    ///
    /// 参数顺序允许互换；scheme 大小写不敏感；值必须用双引号包裹（RFC 7235 token68/
    /// quoted-string，L402 规范要求 quoted）。多余空白容忍，未知参数忽略。
    pub fn parse_www_authenticate(header: &str) -> Result<Self, L402Error> {
        let mut rest = header.trim();
        // scheme
        let scheme_end = rest
            .find(|c: char| c.is_whitespace())
            .ok_or(L402Error::L402MalformedChallenge)?;
        let scheme = rest[..scheme_end].trim();
        if !scheme.eq_ignore_ascii_case(L402_SCHEME) {
            return Err(L402Error::L402MalformedChallenge);
        }
        rest = rest[scheme_end..].trim_start();

        let mut macaroon: Option<String> = None;
        let mut invoice: Option<String> = None;
        // 逐个解析 key="value"，以逗号分隔但尊重引号内逗号。
        while !rest.is_empty() {
            let eq = rest.find('=').ok_or(L402Error::L402MissingParam)?;
            let key = rest[..eq].trim().to_ascii_lowercase();
            let after = &rest[eq + 1..];
            if !after.starts_with('"') {
                return Err(L402Error::L402MissingParam);
            }
            let body = &after[1..];
            let close = body.find('"').ok_or(L402Error::L402MissingParam)?;
            let value = body[..close].to_string();
            match key.as_str() {
                "macaroon" => macaroon = Some(value),
                "invoice" => invoice = Some(value),
                _ => {}
            }
            let mut nxt = body[close + 1..].trim_start();
            if let Some(stripped) = nxt.strip_prefix(',') {
                nxt = stripped.trim_start();
            }
            rest = nxt;
        }

        let macaroon = macaroon.ok_or(L402Error::L402MissingParam)?;
        let invoice = invoice.ok_or(L402Error::L402MissingParam)?;
        if macaroon.is_empty() || invoice.is_empty() {
            return Err(L402Error::L402MissingParam);
        }
        Ok(L402Challenge { macaroon, invoice })
    }

    /// 解析发票人类可读金额前缀，返回整数 msats。
    ///
    /// 仅解析 `ln(bc|tb|bcrt)<amount><multiplier?>` 的前缀，**不**触碰 bech32 数据段。
    /// multiplier（相对 BTC）：空=`BTC`、`m`=milli、`u`=micro、`n`=nano、`p`=pico。
    pub fn invoice_amount_msat(&self) -> Result<u64, L402Error> {
        parse_bolt11_amount_msat(&self.invoice)
    }
}

/// 解析 BOLT11 发票前缀金额为整数 msats（见 [`L402Challenge::invoice_amount_msat`]）。
pub fn parse_bolt11_amount_msat(invoice: &str) -> Result<u64, L402Error> {
    // 去掉可能的 "lightning:" URI 前缀，大小写不敏感。
    let trimmed = invoice.trim();
    let s = if trimmed.len() >= 10 && trimmed[..10].eq_ignore_ascii_case("lightning:") {
        &trimmed[10..]
    } else {
        trimmed
    };

    // 网络前缀。
    let body = if let Some(b) = s.strip_prefix("lnbcrt") {
        b
    } else if let Some(b) = s.strip_prefix("lnbc") {
        b
    } else if let Some(b) = s.strip_prefix("lntb") {
        b
    } else {
        return Err(L402Error::L402UnsupportedInvoiceNetwork);
    };

    // 取金额段：连续 ASCII 数字 + 可选单个 multiplier 字母；到下一个非金额字符止。
    let digits: String = body.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        // 无金额发票（BOLT11 允许 amount 省略）无法在内核侧做精确守恒，具名拒绝。
        return Err(L402Error::L402InvalidAmount);
    }
    let amount: u128 = digits
        .parse::<u128>()
        .map_err(|_| L402Error::L402InvalidAmount)?;

    // multiplier 是数字后第一个字符（若为 m/u/n/p）。
    let mult = body[digits.len()..].chars().next();
    // 金额（单位 msat）乘数：以 1 BTC = 1e11 msat 为基准。
    // m:1e8  u:1e5  n:1e2  p:1e-1（故 10p=1msat）。
    // BOLT11 语法中 multiplier 可省略；数字后若紧跟 m/u/n/p 之外的任意字符（典型为
    // bech32 分隔符 p/q 等数据段字符），即表示无 multiplier、按整 BTC 计，而非错误。
    let scaled: u128 = match mult {
        Some('m') => amount.checked_mul(100_000_000),
        Some('u') => amount.checked_mul(100_000),
        Some('n') => amount.checked_mul(100),
        Some('p') => {
            // 1 picoBTC = 1e-1 msat；amount 必须为 10 的整数倍才能得到整数 msat。
            if !amount.is_multiple_of(10) {
                return Err(L402Error::L402SubMsatAmount);
            }
            amount.checked_div(10)
        }
        _ => amount.checked_mul(100_000_000_000),
    }
    .ok_or(L402Error::L402AmountOverflow)?;

    u64::try_from(scaled).map_err(|_| L402Error::L402AmountOverflow)
}

/// 客户端支付后构造并重放的 L402 凭证。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct L402Credential {
    pub macaroon: String,
    pub preimage: Preimage,
}

impl L402Credential {
    /// 用挑战中的 macaroon 与支付所得 preimage 构造凭证。
    pub fn new(challenge: &L402Challenge, preimage_hex: &str) -> Result<Self, L402Error> {
        let preimage = Preimage::from_hex(preimage_hex).ok_or(L402Error::L402InvalidPreimage)?;
        Ok(L402Credential {
            macaroon: challenge.macaroon.clone(),
            preimage,
        })
    }

    /// 生成 `Authorization` 头值：`L402 <macaroon>:<preimage_hex>`。
    /// （macaroon 原样透传；生产实现常见为 base64url，本内核不重新编码以免改变字节。）
    pub fn to_authorization_header(&self) -> String {
        format!(
            "{} {}:{}",
            L402_SCHEME,
            self.macaroon,
            self.preimage.to_hex()
        )
    }

    /// 服务端/校验方校验凭证：
    ///
    /// 1. macaroon 必须与挑战一致（防票据替换）；
    /// 2. `SHA256(preimage)` 必须等于调用方从受信任闪电节点取得的 `expected_hash`；
    /// 3. 发票金额（`required_msat`）与实付声明（`paid_msat`）精确守恒。
    ///
    /// 仅本地确定性校验，不代表 HTLC 已在链/路由层最终确认。
    pub fn verify(
        &self,
        challenge: &L402Challenge,
        expected_hash: &PaymentHash,
        paid_msat: u64,
    ) -> Result<(), L402Error> {
        if self.macaroon != challenge.macaroon {
            return Err(L402Error::L402MacaroonMismatch);
        }
        if self.preimage.payment_hash() != *expected_hash {
            return Err(L402Error::L402PreimageHashMismatch);
        }
        let required = challenge.invoice_amount_msat()?;
        verify_settlement(required, paid_msat)
    }
}

/// 精确金额守恒：实付必须恰好等于应付；多付/少付一律具名拒绝。
pub fn verify_settlement(required_msat: u64, paid_msat: u64) -> Result<(), L402Error> {
    if required_msat == paid_msat {
        Ok(())
    } else {
        Err(L402Error::L402SettlementMismatch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash_hex(h: &PaymentHash) -> String {
        h.to_hex()
    }

    #[test]
    fn parses_challenge_both_param_orders() {
        let h1 = r#"L402 macaroon="AGIA9234", invoice="lnbc100n1pabc""#;
        let c1 = L402Challenge::parse_www_authenticate(h1).unwrap();
        assert_eq!(c1.macaroon, "AGIA9234");
        assert_eq!(c1.invoice, "lnbc100n1pabc");

        // 参数顺序互换 + 大小写 scheme + 多余空白。
        let h2 = r#"l402   invoice="lntb2500m1q" , macaroon="XYZ""#;
        let c2 = L402Challenge::parse_www_authenticate(h2).unwrap();
        assert_eq!(c2.macaroon, "XYZ");
        assert_eq!(c2.invoice, "lntb2500m1q");
    }

    #[test]
    fn rejects_malformed_challenge() {
        assert_eq!(
            L402Challenge::parse_www_authenticate("Bearer abc").err(),
            Some(L402Error::L402MalformedChallenge)
        );
        // 缺 invoice。
        assert_eq!(
            L402Challenge::parse_www_authenticate(r#"L402 macaroon="x""#).err(),
            Some(L402Error::L402MissingParam)
        );
        // 值未加引号。
        assert_eq!(
            L402Challenge::parse_www_authenticate(r#"L402 macaroon=x invoice="y""#).err(),
            Some(L402Error::L402MissingParam)
        );
        // 空 macaroon。
        assert_eq!(
            L402Challenge::parse_www_authenticate(r#"L402 macaroon="", invoice="lnbc1""#).err(),
            Some(L402Error::L402MissingParam)
        );
    }

    #[test]
    fn bolt11_amount_multipliers_to_msat() {
        // lnbc1（无 multiplier）= 1 BTC = 1e11 msat；数字后紧跟 bech32 数据字符 q 同样按整 BTC。
        assert_eq!(parse_bolt11_amount_msat("lnbc1"), Ok(100_000_000_000));
        assert_eq!(parse_bolt11_amount_msat("lnbc1qdata"), Ok(100_000_000_000));
        // 1n = 1 nanoBTC = 100 msat。
        assert_eq!(parse_bolt11_amount_msat("lnbc1n1p"), Ok(100));
        // 2500m = 2500 mBTC = 2.5 BTC = 250_000_000_000 msat。
        assert_eq!(parse_bolt11_amount_msat("lntb2500m1q"), Ok(250_000_000_000));
        // 100u = 100 * 1e5 = 10_000_000 msat。
        assert_eq!(parse_bolt11_amount_msat("lnbc100u1p"), Ok(10_000_000));
        // 100n = 100 * 100 = 10_000 msat。
        assert_eq!(parse_bolt11_amount_msat("lnbc100n1p"), Ok(10_000));
        // 10p = 1 msat。
        assert_eq!(parse_bolt11_amount_msat("lnbc10p1p"), Ok(1));
        // regtest 网络。
        assert_eq!(parse_bolt11_amount_msat("lnbcrt500n1p"), Ok(50_000));
        // lightning: URI 前缀。
        assert_eq!(parse_bolt11_amount_msat("LIGHTNING:lnbc10n1p"), Ok(1_000));
        // m/u/n/p 之外的数字后字母是 bech32 数据段，按无 multiplier（100 BTC）计。
        assert_eq!(
            parse_bolt11_amount_msat("lnbc100x1p"),
            Ok(10_000_000_000_000)
        );
    }

    #[test]
    fn rejects_bad_amounts() {
        // 不支持的网络。
        assert_eq!(
            parse_bolt11_amount_msat("lnxx100n").err(),
            Some(L402Error::L402UnsupportedInvoiceNetwork)
        );
        // 省略金额的发票（lnbc 后直接是 bech32 数据，无数字）→ 无法精确守恒。
        assert_eq!(
            parse_bolt11_amount_msat("lnbcn1p").err(),
            Some(L402Error::L402InvalidAmount)
        );
        // pico 非 10 倍数 → 亚 msat。
        assert_eq!(
            parse_bolt11_amount_msat("lnbc15p1p").err(),
            Some(L402Error::L402SubMsatAmount)
        );
    }

    #[test]
    fn preimage_hash_relation_and_credential_verify() {
        // 任取 32 字节原像，payment_hash = SHA256(preimage)（标准 sha2，非 keccak）。
        let pre = [7u8; 32];
        let preimage = Preimage(pre);
        let expected = preimage.payment_hash();
        // 与 sha2 crate 直接计算一致。
        assert_eq!(expected.0, Sha256::digest(pre).as_slice());

        let ch = L402Challenge {
            macaroon: "MAC".to_string(),
            invoice: "lnbc100n1p".to_string(), // 10_000 msat
        };
        // 正确凭证 + 精确金额 → 通过。
        let cred = L402Credential::new(&ch, &hex::encode(pre)).unwrap();
        assert!(cred.verify(&ch, &expected, 10_000).is_ok());
        assert_eq!(
            cred.to_authorization_header(),
            format!("L402 MAC:{}", hex::encode(pre))
        );

        // 少付/多付 → 守恒拒绝。
        assert_eq!(
            cred.verify(&ch, &expected, 9_999).err(),
            Some(L402Error::L402SettlementMismatch)
        );
        assert_eq!(
            cred.verify(&ch, &expected, 10_001).err(),
            Some(L402Error::L402SettlementMismatch)
        );

        // 错误哈希（伪造/未支付原像）→ 拒绝。
        let wrong = PaymentHash([0u8; 32]);
        assert_eq!(
            cred.verify(&ch, &wrong, 10_000).err(),
            Some(L402Error::L402PreimageHashMismatch)
        );

        // macaroon 被替换 → 票据不一致。
        let ch2 = L402Challenge {
            macaroon: "OTHER".to_string(),
            invoice: ch.invoice.clone(),
        };
        assert_eq!(
            cred.verify(&ch2, &expected, 10_000).err(),
            Some(L402Error::L402MacaroonMismatch)
        );

        // 非法 preimage hex。
        assert_eq!(
            L402Credential::new(&ch, "abcd").err(),
            Some(L402Error::L402InvalidPreimage)
        );
    }

    #[test]
    fn payment_hash_hex_roundtrip() {
        let h = Preimage([1u8; 32]).payment_hash();
        let s = hash_hex(&h);
        assert_eq!(PaymentHash::from_hex(&s), Some(h));
        assert_eq!(PaymentHash::from_hex(&format!("0x{s}")), Some(h));
        assert_eq!(PaymentHash::from_hex("0x12"), None);
        assert_eq!(PaymentHash::from_hex(&format!("{}zz", &s[2..])), None);
    }
}
