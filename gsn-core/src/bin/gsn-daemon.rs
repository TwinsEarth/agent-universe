//! gsn-daemon - GSN 全节点 headless 守护进程
//!
//! 用法: gsn-daemon [--listen 0.0.0.0] [--port 4001] [--api-port 4002] [--data-dir ~/.gsn/data]

use gsn_core::NodeMode;
use std::path::PathBuf;

#[derive(Debug, Clone)]
struct Args {
    listen: String,
    port: u16,
    api_port: u16,
    data_dir: PathBuf,
    mode: String,
    bootstrap: Vec<String>,
}

impl Args {
    fn parse() -> Self {
        let mut listen = "0.0.0.0".to_string();
        let mut port = 4001u16;
        let mut api_port = 4002u16;
        let mut data_dir = PathBuf::from("~/.gsn/data");
        let mut mode = "full".to_string();
        let mut bootstrap = Vec::new();

        let args: Vec<String> = std::env::args().collect();
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--listen" => { listen = args[i+1].clone(); i += 2; }
                "--port" => { port = args[i+1].parse().unwrap_or(4001); i += 2; }
                "--api-port" => { api_port = args[i+1].parse().unwrap_or(4002); i += 2; }
                "--data-dir" => { data_dir = PathBuf::from(&args[i+1]); i += 2; }
                "--mode" => { mode = args[i+1].clone(); i += 2; }
                "--bootstrap" => {
                    while i + 1 < args.len() && !args[i+1].starts_with("--") {
                        bootstrap.push(args[i+1].clone());
                        i += 1;
                    }
                    i += 1;
                }
                "--help" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => i += 1,
            }
        }

        Self { listen, port, api_port, data_dir, mode, bootstrap }
    }
}

fn print_help() {
    println!("gsn-daemon - GSN 全节点守护进程");
    println!();
    println!("用法: gsn-daemon [选项]");
    println!();
    println!("选项:");
    println!("  --listen <addr>     监听地址 (默认: 0.0.0.0)");
    println!("  --port <port>       P2P 端口 (默认: 4001)");
    println!("  --api-port <port>   HTTP API 端口 (默认: 4002)");
    println!("  --data-dir <path>   数据目录 (默认: ~/.gsn/data)");
    println!("  --mode <mode>       节点模式: archive|full|light|edge|browser (默认: full)");
    println!("  --bootstrap <addr>  引导节点 multiaddr (可多个)");
    println!("  --help              显示帮助");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    println!("=== GSN Daemon v{} ===", env!("CARGO_PKG_VERSION"));
    println!("监听: {}:{}", args.listen, args.port);
    println!("API:  :{}", args.api_port);
    println!("数据: {:?}", args.data_dir);
    println!("模式: {}", args.mode);
    if args.bootstrap.is_empty() {
        println!("引导: 无（根种子模式）");
    } else {
        println!("引导: {} 个节点", args.bootstrap.len());
        for addr in &args.bootstrap {
            println!("  → {}", addr);
        }
    }

    let node_mode = match args.mode.as_str() {
        "archive" => NodeMode::Archive,
        "full" => NodeMode::Full,
        "light" => NodeMode::Light,
        "edge" => NodeMode::Edge,
        "browser" => NodeMode::Browser,
        _ => NodeMode::Full,
    };

    println!("节点模式: {:?}", node_mode);

    // 创建数据目录
    std::fs::create_dir_all(&args.data_dir)?;
    std::fs::create_dir_all(args.data_dir.join("logs"))?;

    println!("✅ gsn-daemon 启动完成");
    println!("   P2P 端口: {}", args.port);
    println!("   API 端口: {}", args.api_port);
    println!("   数据目录: {:?}", args.data_dir);

    // 保持运行
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}
