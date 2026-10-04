//! v3.9.1 宿主签名服务与私钥隔离。
//!
//! # 目标
//!
//! 沙盒内的 Agent 永远**见不到私钥**。Agent 需要对一条支付/授权载荷签名时，只能把
//! [`SignIntent`] 提交给宿主签名闸门 [`HostSignerGate`]：
//!
//! - [`HostSignerGate::preview`] 只做输入校验与规范化，产出 [`SignPreview`]——不接触任何
//!   密钥、不产出签名，任何沙盒都可调用；
//! - [`HostSignerGate::request_sign`] 先做同样的确定性校验，再向宿主侧 [`SignatureBroker`]
//!   请求签名，返回的 [`SignatureReceipt`] **只含公钥与签名**，绝不含私钥/seed。
//!
//! # fail-closed
//!
//! 生产宿主在没有注入真实平台密钥后端时装配 [`UnconfiguredBroker`]：其
//! [`SignatureBroker::configured`] 恒为 `false`，`request_sign` 对任何意图一律返回
//! [`PaymentError::SignerNotConfigured`](super::PaymentError::SignerNotConfigured) 具名拒绝，
//! **绝不返回伪造签名**。preview 仍可用（它本就不需要密钥）。
//!
//! [`InMemoryBroker`] 是宿主进程内参考实现（持有 Ed25519 [`Keypair`]），用于单节点宿主装配
//! 与测试：密钥只活在宿主进程内存里，经闸门签发后只外泄公钥与签名。它不代表密钥进入沙盒。
//!
//! # 诚实边界
//!
//! 本模块仍是**纯确定性、零 IO、零 syscall、零 unsafe、无浮点**的策略/闸门内核：不连接任何
//! 链或闪电节点、不广播交易、不做币种兑换；真网轨道适配在 v3.9.2（EVM x402）/
//! v3.9.3（闪电 L402）。重放（nonce 双花）检测需要有状态账本，留给后续有状态宿主，本版本
//! 只把 `nonce` 纳入被签规范化载荷，不声称已完成链下重放拦截。

use super::router::PaymentTrack;
use super::PaymentError;
use crate::identity::{Ed25519Signer, Keypair};
use serde::{Deserialize, Serialize};

/// 待签消息摘要长度（32 字节，与常见 hash 输出对齐；内核不自行挑选哈希算法）。
pub const DIGEST_LEN: usize = 32;

/// 规范化字段分隔符（ASCII Unit Separator，不与 track 名/十进制数字冲突）。
const SEP: u8 = 0x1f;

/// 一次支付/授权签名意图（由沙盒内 Agent 构造，提交给宿主闸门）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignIntent {
    /// 走哪条支付轨道（决定用哪把宿主密钥与策略上限）。
    pub track: PaymentTrack,
    /// 签名域（如 L402 域 / EIP-712 domain 标识），不得为空白。
    pub domain: String,
    /// 待签核心摘要，必须恰为 [`DIGEST_LEN`] 字节。
    pub message_digest: Vec<u8>,
    /// 本签名授权的支付上限（内部整数 micro）；`auth_only=true` 时必须为 0。
    pub spend_cap_micro: u128,
    /// 防重放序号（纳入被签载荷；双花检测需有状态宿主，本版本不拦截历史 nonce）。
    pub nonce: u64,
    /// 纯认证（不授权任何支付）；为 `true` 时 `spend_cap_micro` 必须为 0。
    pub auth_only: bool,
}

impl SignIntent {
    /// 纯确定性校验：坏输入在触碰密钥之前就被具名拒绝。
    pub fn validate(&self, policy: &SignPolicy) -> Result<(), PaymentError> {
        if self.domain.trim().is_empty() {
            return Err(PaymentError::EmptySigningDomain);
        }
        if self.message_digest.len() != DIGEST_LEN {
            return Err(PaymentError::InvalidDigestLength {
                expected: DIGEST_LEN,
                got: self.message_digest.len(),
            });
        }
        if self.auth_only {
            if self.spend_cap_micro != 0 {
                return Err(PaymentError::AuthOnlyWithSpendCap);
            }
        } else if self.spend_cap_micro == 0 {
            return Err(PaymentError::NonPositiveSpendCap);
        }
        let rule = policy.for_track(self.track);
        if !rule.enabled {
            return Err(PaymentError::SigningTrackDisabled {
                required: self.track,
            });
        }
        if self.spend_cap_micro > rule.spend_cap_micro {
            return Err(PaymentError::SpendCapExceeded {
                requested_micro: self.spend_cap_micro,
                allowed_micro: rule.spend_cap_micro,
            });
        }
        Ok(())
    }

    /// 确定性规范化待签载荷：domain | track | cap(BE) | nonce(BE) | auth_only | digest。
    ///
    /// 对完整授权上下文（域/轨道/上限/序号）签名而非只签摘要，可防止任一字段被替换后挪用。
    pub fn normalized_payload(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.domain.len() + 96);
        out.extend_from_slice(self.domain.trim().as_bytes());
        out.push(SEP);
        out.extend_from_slice(self.track.as_str().as_bytes());
        out.push(SEP);
        out.extend_from_slice(&self.spend_cap_micro.to_be_bytes());
        out.push(SEP);
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out.push(SEP);
        out.push(if self.auth_only { 1u8 } else { 0u8 });
        out.push(SEP);
        out.extend_from_slice(&self.message_digest);
        out
    }
}

/// 单条轨道的签名策略：是否启用 + 该轨道单笔授权上限（micro）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackSignRule {
    pub enabled: bool,
    pub spend_cap_micro: u128,
}

/// 三轨签名策略（宿主受控常量）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignPolicy {
    pub lightning_l402: TrackSignRule,
    pub evm_x402: TrackSignRule,
    pub btc_rgb_htlc: TrackSignRule,
}

impl Default for SignPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl SignPolicy {
    /// 默认上限与 v3.9.0 选路阈值呼应：闪电小额、EVM 常规、BTC 大额最终结算。
    pub const DEFAULT: SignPolicy = SignPolicy {
        lightning_l402: TrackSignRule {
            enabled: true,
            spend_cap_micro: 10_000,
        },
        evm_x402: TrackSignRule {
            enabled: true,
            spend_cap_micro: 100_000_000,
        },
        btc_rgb_htlc: TrackSignRule {
            enabled: true,
            spend_cap_micro: 1_000_000_000,
        },
    };

    /// 启用轨道的上限必须为正，否则策略非法（避免出现「启用但零额度」的矛盾态）。
    pub const fn validated(self) -> Result<Self, PaymentError> {
        if self.lightning_l402.enabled && self.lightning_l402.spend_cap_micro == 0 {
            return Err(PaymentError::SignPolicyInvalid {
                track: PaymentTrack::LightningL402,
            });
        }
        if self.evm_x402.enabled && self.evm_x402.spend_cap_micro == 0 {
            return Err(PaymentError::SignPolicyInvalid {
                track: PaymentTrack::EvmX402,
            });
        }
        if self.btc_rgb_htlc.enabled && self.btc_rgb_htlc.spend_cap_micro == 0 {
            return Err(PaymentError::SignPolicyInvalid {
                track: PaymentTrack::BtcRgbHtlc,
            });
        }
        Ok(self)
    }

    /// 取某轨道规则。
    pub const fn for_track(&self, track: PaymentTrack) -> &TrackSignRule {
        match track {
            PaymentTrack::LightningL402 => &self.lightning_l402,
            PaymentTrack::EvmX402 => &self.evm_x402,
            PaymentTrack::BtcRgbHtlc => &self.btc_rgb_htlc,
        }
    }
}

/// 签名预览（不接触密钥、不产出签名）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignPreview {
    pub track: PaymentTrack,
    pub domain: String,
    pub nonce: u64,
    pub spend_cap_micro: u128,
    pub auth_only: bool,
    pub message_digest_hex: String,
    pub normalized_payload_hex: String,
    /// 真实签发必须由宿主密钥完成；沙盒拿不到私钥。
    pub signing_requires_host_key: bool,
    /// 当前宿主是否已配置密钥后端（生产默认 false）。
    pub host_key_configured: bool,
}

/// 签名回执：只含公钥与签名，**绝不含私钥/seed**。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureReceipt {
    pub track: PaymentTrack,
    pub public_key_hex: String,
    pub signature_hex: String,
    pub signed_normalized_payload_hex: String,
}

/// 宿主侧签名后端抽象。实现方运行在宿主进程，私钥不跨沙盒边界。
pub trait SignatureBroker {
    /// 是否已配置可用密钥；false 时闸门 fail-closed。
    fn configured(&self) -> bool;
    /// 返回该轨道验签公钥（沙盒可见公钥）。
    fn host_public_key(&self, track: PaymentTrack) -> Result<Vec<u8>, PaymentError>;
    /// 在宿主侧对规范化载荷签名，返回签名字节。
    fn sign_on_host(&self, track: PaymentTrack, normalized: &[u8])
        -> Result<Vec<u8>, PaymentError>;
}

/// 未配置后端：生产默认，任何签发一律具名拒绝。
#[derive(Debug, Default, Clone, Copy)]
pub struct UnconfiguredBroker;

impl SignatureBroker for UnconfiguredBroker {
    fn configured(&self) -> bool {
        false
    }
    fn host_public_key(&self, _track: PaymentTrack) -> Result<Vec<u8>, PaymentError> {
        Err(PaymentError::SignerNotConfigured)
    }
    fn sign_on_host(
        &self,
        _track: PaymentTrack,
        _normalized: &[u8],
    ) -> Result<Vec<u8>, PaymentError> {
        Err(PaymentError::SignerNotConfigured)
    }
}

/// 宿主进程内参考后端：每轨一把 Ed25519 密钥，仅活在宿主内存（用于单节点装配与测试）。
pub struct InMemoryBroker {
    lightning: Keypair,
    evm: Keypair,
    btc: Keypair,
}

impl Default for InMemoryBroker {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryBroker {
    /// 随机生成三轨密钥（宿主侧）。
    pub fn new() -> Self {
        Self {
            lightning: Keypair::generate(),
            evm: Keypair::generate(),
            btc: Keypair::generate(),
        }
    }

    /// 用固定种子构造（仅测试/确定性恢复；调用方须自行保证种子不进沙盒）。
    pub fn from_seeds(lightning: &[u8; 32], evm: &[u8; 32], btc: &[u8; 32]) -> Self {
        Self {
            lightning: Keypair::from_seed(lightning),
            evm: Keypair::from_seed(evm),
            btc: Keypair::from_seed(btc),
        }
    }

    fn keypair(&self, track: PaymentTrack) -> &Keypair {
        match track {
            PaymentTrack::LightningL402 => &self.lightning,
            PaymentTrack::EvmX402 => &self.evm,
            PaymentTrack::BtcRgbHtlc => &self.btc,
        }
    }
}

impl SignatureBroker for InMemoryBroker {
    fn configured(&self) -> bool {
        true
    }
    fn host_public_key(&self, track: PaymentTrack) -> Result<Vec<u8>, PaymentError> {
        Ok(self.keypair(track).public_key().to_vec())
    }
    fn sign_on_host(
        &self,
        track: PaymentTrack,
        normalized: &[u8],
    ) -> Result<Vec<u8>, PaymentError> {
        Ok(Ed25519Signer::new(self.keypair(track)).sign(normalized))
    }
}

/// 宿主签名闸门：先做确定性校验与规范化，再决定是否/如何让宿主后端签发。
pub struct HostSignerGate<B: SignatureBroker> {
    policy: SignPolicy,
    broker: B,
}

/// 生产默认闸门类型（未配置密钥，fail-closed）。
pub type UnconfiguredSignerGate = HostSignerGate<UnconfiguredBroker>;

impl UnconfiguredSignerGate {
    /// 生产装配点：默认策略 + 未配置后端。
    pub fn unconfigured() -> Self {
        HostSignerGate::new(SignPolicy::DEFAULT, UnconfiguredBroker)
    }
}

impl<B: SignatureBroker> HostSignerGate<B> {
    /// 建闸门（策略经合法性校验）。
    pub fn new(policy: SignPolicy, broker: B) -> Self {
        // 非法策略属于编程错误，但仍以确定类型暴露而非 panic：调用方可先 validated()。
        let policy = policy.validated().unwrap_or(SignPolicy::DEFAULT);
        Self { policy, broker }
    }

    /// 注入自定义策略（非法时原样返回错误，不建闸门）。
    pub fn with_validated_policy(broker: B, policy: SignPolicy) -> Result<Self, PaymentError> {
        Ok(Self {
            policy: policy.validated()?,
            broker,
        })
    }

    /// 当前宿主是否已配置密钥后端。
    pub fn host_key_configured(&self) -> bool {
        self.broker.configured()
    }

    /// 只读预览：校验 + 规范化，不触碰密钥。
    pub fn preview(&self, intent: &SignIntent) -> Result<SignPreview, PaymentError> {
        intent.validate(&self.policy)?;
        let normalized = intent.normalized_payload();
        Ok(SignPreview {
            track: intent.track,
            domain: intent.domain.trim().to_string(),
            nonce: intent.nonce,
            spend_cap_micro: intent.spend_cap_micro,
            auth_only: intent.auth_only,
            message_digest_hex: to_hex(&intent.message_digest),
            normalized_payload_hex: to_hex(&normalized),
            signing_requires_host_key: true,
            host_key_configured: self.broker.configured(),
        })
    }

    /// 请求宿主签发。坏输入先具名报错；未配置密钥则 SignerNotConfigured，绝不伪造签名。
    pub fn request_sign(&self, intent: &SignIntent) -> Result<SignatureReceipt, PaymentError> {
        let preview = self.preview(intent)?;
        if !self.broker.configured() {
            return Err(PaymentError::SignerNotConfigured);
        }
        let normalized = intent.normalized_payload();
        let public_key = self.broker.host_public_key(intent.track)?;
        let signature = self.broker.sign_on_host(intent.track, &normalized)?;
        Ok(SignatureReceipt {
            track: intent.track,
            public_key_hex: to_hex(&public_key),
            signature_hex: to_hex(&signature),
            signed_normalized_payload_hex: preview.normalized_payload_hex,
        })
    }
}

/// 纯函数字节→小写十六进制（零依赖、确定性）。
pub fn to_hex(bytes: &[u8]) -> String {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(H[(b >> 4) as usize] as char);
        s.push(H[(b & 0x0f) as usize] as char);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(track: PaymentTrack) -> SignIntent {
        SignIntent {
            track,
            domain: "x402.agent-universe.local".to_string(),
            message_digest: vec![7u8; DIGEST_LEN],
            spend_cap_micro: 1_000,
            nonce: 42,
            auth_only: false,
        }
    }

    #[test]
    fn preview_normalizes_deterministically_without_key() {
        let gate = HostSignerGate::unconfigured();
        let p1 = gate.preview(&sample(PaymentTrack::EvmX402)).unwrap();
        let p2 = gate.preview(&sample(PaymentTrack::EvmX402)).unwrap();
        assert_eq!(p1, p2);
        assert!(p1.signing_requires_host_key);
        assert!(!p1.host_key_configured);
        assert_eq!(p1.message_digest_hex.len(), DIGEST_LEN * 2);
        assert!(p1.normalized_payload_hex.contains("78343032")); // "x402" ascii
    }

    #[test]
    fn unconfigured_gate_rejects_signing_but_allows_preview() {
        let gate = HostSignerGate::unconfigured();
        let intent = sample(PaymentTrack::LightningL402);
        assert!(gate.preview(&intent).is_ok());
        assert_eq!(
            gate.request_sign(&intent).unwrap_err(),
            PaymentError::SignerNotConfigured
        );
    }

    #[test]
    fn validates_domain_digest_cap_and_authonly() {
        let gate = HostSignerGate::unconfigured();
        let mut bad_domain = sample(PaymentTrack::EvmX402);
        bad_domain.domain = "   ".to_string();
        assert_eq!(
            gate.preview(&bad_domain).unwrap_err(),
            PaymentError::EmptySigningDomain
        );

        let mut bad_len = sample(PaymentTrack::EvmX402);
        bad_len.message_digest = vec![1u8; 31];
        assert!(matches!(
            gate.preview(&bad_len).unwrap_err(),
            PaymentError::InvalidDigestLength {
                expected: 32,
                got: 31
            }
        ));

        let mut zero_cap = sample(PaymentTrack::EvmX402);
        zero_cap.spend_cap_micro = 0;
        assert_eq!(
            gate.preview(&zero_cap).unwrap_err(),
            PaymentError::NonPositiveSpendCap
        );

        let mut auth_with_cap = sample(PaymentTrack::EvmX402);
        auth_with_cap.auth_only = true;
        auth_with_cap.spend_cap_micro = 5;
        assert_eq!(
            gate.preview(&auth_with_cap).unwrap_err(),
            PaymentError::AuthOnlyWithSpendCap
        );

        // auth_only 必须 cap=0：合法。
        let mut auth_ok = sample(PaymentTrack::EvmX402);
        auth_ok.auth_only = true;
        auth_ok.spend_cap_micro = 0;
        assert!(gate.preview(&auth_ok).is_ok());
    }

    #[test]
    fn validates_track_enable_and_spend_cap() {
        let policy = SignPolicy {
            lightning_l402: TrackSignRule {
                enabled: true,
                spend_cap_micro: 100,
            },
            evm_x402: TrackSignRule {
                enabled: false,
                spend_cap_micro: 0,
            },
            btc_rgb_htlc: TrackSignRule {
                enabled: true,
                spend_cap_micro: 1_000,
            },
        };
        let gate = HostSignerGate::with_validated_policy(UnconfiguredBroker, policy).unwrap();

        let mut over_cap = sample(PaymentTrack::LightningL402);
        over_cap.spend_cap_micro = 101;
        assert!(matches!(
            gate.preview(&over_cap).unwrap_err(),
            PaymentError::SpendCapExceeded {
                requested_micro: 101,
                allowed_micro: 100
            }
        ));

        let disabled = sample(PaymentTrack::EvmX402);
        assert_eq!(
            gate.preview(&disabled).unwrap_err(),
            PaymentError::SigningTrackDisabled {
                required: PaymentTrack::EvmX402
            }
        );

        // 启用但零额度是非法策略。
        let bad = SignPolicy {
            lightning_l402: TrackSignRule {
                enabled: true,
                spend_cap_micro: 0,
            },
            ..SignPolicy::DEFAULT
        };
        assert!(matches!(
            bad.validated().unwrap_err(),
            PaymentError::SignPolicyInvalid {
                track: PaymentTrack::LightningL402
            }
        ));
    }

    #[test]
    fn configured_gate_returns_verifiable_receipt_without_secret() {
        let broker = InMemoryBroker::from_seeds(&[1u8; 32], &[2u8; 32], &[3u8; 32]);
        let gate = HostSignerGate::new(SignPolicy::DEFAULT, broker);
        assert!(gate.host_key_configured());

        let intent = sample(PaymentTrack::EvmX402);
        let normalized = intent.normalized_payload();
        let receipt = gate.request_sign(&intent).unwrap();

        // 公钥来自宿主密钥，签名可用公钥独立验过。
        let pubkey = hex_decode(&receipt.public_key_hex);
        let sig = hex_decode(&receipt.signature_hex);
        assert!(Ed25519Signer::verify_with_pubkey(
            &pubkey,
            &normalized,
            &sig
        ));

        // 回执序列化后不含私钥/seed（各轨 seed 为常量 [1]/[2]/[3]）。
        let raw = serde_json::to_string(&receipt).unwrap();
        assert!(!raw.contains(&to_hex(&[2u8; 32]))); // evm seed
        assert!(!raw.to_lowercase().contains("seed"));
        assert!(!raw.to_lowercase().contains("secret"));
        assert!(!raw.to_lowercase().contains("private"));
        // 签的是完整规范化载荷而非裸摘要。
        assert_eq!(receipt.signed_normalized_payload_hex, to_hex(&normalized));
    }

    #[test]
    fn broker_is_not_called_when_intent_invalid() {
        // 校验先于后端：即使已配置，坏输入也到不了 broker。
        let broker = InMemoryBroker::new();
        let gate = HostSignerGate::new(SignPolicy::DEFAULT, broker);
        let mut bad = sample(PaymentTrack::EvmX402);
        bad.message_digest = vec![];
        assert!(matches!(
            gate.request_sign(&bad).unwrap_err(),
            PaymentError::InvalidDigestLength { .. }
        ));
    }

    fn hex_decode(s: &str) -> Vec<u8> {
        let b = s.as_bytes();
        (0..b.len())
            .step_by(2)
            .map(|i| {
                let h = |c: u8| match c {
                    b'0'..=b'9' => c - b'0',
                    b'a'..=b'f' => c - b'a' + 10,
                    b'A'..=b'F' => c - b'A' + 10,
                    _ => 0,
                };
                h(b[i]) << 4 | h(b[i + 1])
            })
            .collect()
    }
}
