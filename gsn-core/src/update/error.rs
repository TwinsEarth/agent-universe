//! 自动更新子系统的类型化错误。生产代码一律返回 `Result`，不使用
//! `unwrap/expect/panic`（遵循 `scripts/check-no-panics.mjs`）。

use std::io::ErrorKind;

/// 更新流程中所有可预期的失败。
#[derive(Debug)]
pub enum UpdateError {
    /// 版本字符串无法解析为 X.Y.Z。
    BadVersion(String),
    /// Rust 形态版本不符合 0.X.NN 约定。
    BadRustVersion(String),
    /// 补丁号达到 10，npm↔Rust 编码会碰撞。
    PatchOutOfRange(u64),
    /// 版本算术溢出。
    VersionOverflow,
    /// 无法识别的更新通道名（GSN_UPDATE_TRACK typo）。
    BadTrack(String),
    /// 通用 HTTP 错误。
    Http(String),
    /// TLS 握手/证书错误。
    Tls(String),
    /// 连接或读取超时。
    Timeout,
    /// 响应体超过配置上限。
    BodyTooLarge,
    /// npm registry 返回内容异常。
    Registry(String),
    /// 当前平台不在发布矩阵内。
    UnsupportedTarget(String),
    /// sha256 校验失败或清单非法（fail-closed，拒绝安装）。
    Checksum(String),
    /// 写盘/替换失败，附带路径与 io::ErrorKind。
    IoPath(String, ErrorKind),
    /// 校验和不匹配（期望 vs 实际）。
    ChecksumMismatch { expected: String, actual: String },
    /// 外部命令（npm）不可用或执行失败。
    Command(String),
    /// 守护进程正在运行且当前平台无法原地替换（Windows 文件占用）。
    LockedBinary(String),
    /// 加载本机 libp2p 身份失败（无法判定白名单通道）。
    Identity(String),
}

impl std::fmt::Display for UpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateError::BadVersion(v) => write!(f, "非法版本号: {v}"),
            UpdateError::BadRustVersion(v) => write!(f, "非法 Rust 版本（应为 0.X.NN）: {v}"),
            UpdateError::PatchOutOfRange(p) => {
                write!(f, "补丁号 {p} 超出 npm↔Rust 编码范围（必须 < 10）")
            }
            UpdateError::VersionOverflow => write!(f, "版本号算术溢出"),
            UpdateError::BadTrack(t) => {
                write!(
                    f,
                    "无法识别的更新通道 GSN_UPDATE_TRACK={t}（有效值: minor/patch）"
                )
            }
            UpdateError::Http(m) => write!(f, "HTTP 错误: {m}"),
            UpdateError::Tls(m) => write!(f, "TLS 错误: {m}"),
            UpdateError::Timeout => write!(f, "网络超时"),
            UpdateError::BodyTooLarge => write!(f, "响应体超过大小上限"),
            UpdateError::Registry(m) => write!(f, "npm registry 数据异常: {m}"),
            UpdateError::UnsupportedTarget(t) => write!(f, "当前平台无发布资产: {t}"),
            UpdateError::Checksum(m) => write!(f, "校验和错误: {m}"),
            UpdateError::IoPath(p, k) => write!(f, "文件操作失败 [{p}]: {k}"),
            UpdateError::ChecksumMismatch { expected, actual } => write!(
                f,
                "下载二进制 sha256 不匹配，拒绝安装（期望 {expected}，实际 {actual}）"
            ),
            UpdateError::Command(m) => write!(f, "外部命令失败: {m}"),
            UpdateError::LockedBinary(p) => write!(
                f,
                "目标二进制被占用，无法原地替换: {p}（请先停止正在运行的 daemon 再更新）"
            ),
            UpdateError::Identity(m) => write!(f, "本机身份加载失败: {m}"),
        }
    }
}

impl std::error::Error for UpdateError {}
