//! gsn-daemon - GSN 全节点 headless 守护进程
//!
//! 启动逻辑在 gsn_core::node::run_daemon（与 `gsn daemon` 子命令共用）。
//!
//! 用法: gsn-daemon [选项]，--help 查看帮助

use gsn_core::node;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    // Docker 健康探针：--healthcheck 只做一次 HTTP GET，绝不启动节点。
    if node::wants_healthcheck(&argv) {
        std::process::exit(node::run_healthcheck_now().await);
    }
    let args = node::parse_daemon_args(&argv);
    node::run_daemon(args).await
}
