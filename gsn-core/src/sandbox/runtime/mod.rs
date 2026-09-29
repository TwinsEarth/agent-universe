//! 沙箱运行时（v2.7.7）
//!
//! 运行时是 Sandbox trait 的具体实现，负责把"在沙箱内执行"落到真实进程/容器/VM。
//!
//! - [`process`]：进程级隔离运行时（无 Docker/KVM 时的 fallback，云电脑上真跑）；
//! - 容器 / microVM 运行时在具备对应环境的部署机上实现。
//!
//! 进程级隔离的边界（如实标注）：
//! - 独立临时目录（工作目录/文件系统隔离，不挂载宿主路径）；
//! - 不继承宿主环境变量，仅注入配置白名单；
//! - 经 `timeout` + `ulimit` 强制超时与进程/句柄上限；
//! - 非 root、根文件系统只读不被本进程改写（沙箱只能写自己的临时目录）。
//!
//! 进程级隔离**不**提供内核级隔离，不可用于运行完全不可信的代码。

pub mod process;

pub use process::ProcessSandbox;

/// 代码语言（Agent 生成代码时用）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeLanguage {
    Python,
    JavaScript,
}

impl CodeLanguage {
    pub fn label(&self) -> &'static str {
        match self {
            CodeLanguage::Python => "python",
            CodeLanguage::JavaScript => "javascript",
        }
    }

    /// 从标签解析，未知回 None
    pub fn from_label(s: &str) -> Option<Self> {
        Some(match s.to_lowercase().as_str() {
            "python" | "py" | "python3" => CodeLanguage::Python,
            "javascript" | "js" | "node" => CodeLanguage::JavaScript,
            _ => return None,
        })
    }
}
