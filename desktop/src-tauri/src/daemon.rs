// Agent Universe v2.9.0 — gsn-daemon 生命周期管理
// 跨平台：Windows / macOS / Linux。daemon 作为本地后端（HTTP API），
// 桌面客户端负责查找、启动、健康检查、停止与重启。

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// 守护进程句柄
pub struct DaemonHandle {
    pub child: Child,
    pub api_port: u16,
    pub data_dir: PathBuf,
    pub bin: PathBuf,
}

fn exe_name() -> &'static str {
    if cfg!(windows) {
        "gsn-daemon.exe"
    } else {
        "gsn-daemon"
    }
}

fn which(name: &str) -> Option<PathBuf> {
    let path_env = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_env) {
        let full = dir.join(name);
        if full.is_file() {
            return Some(full);
        }
    }
    None
}

/// 当前可执行文件所在目录
pub fn current_exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// 查找 daemon 二进制。
/// 优先级：环境变量 GSN_DAEMON_BIN → exe 同目录 → bin/ → sidecar/ → 当前目录 → PATH
pub fn find_daemon_binary(exe_dir: &Path) -> Option<PathBuf> {
    let name = exe_name();
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(p) = std::env::var("GSN_DAEMON_BIN") {
        if !p.trim().is_empty() {
            candidates.push(PathBuf::from(p));
        }
    }
    candidates.push(exe_dir.join(name));
    candidates.push(exe_dir.join("bin").join(name));
    candidates.push(exe_dir.join("sidecar").join(name));
    candidates.push(PathBuf::from("bin").join(name));
    candidates.push(PathBuf::from(name));

    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
    }
    which(name)
}

/// 从 preferred 开始寻找可用端口（默认 4002），避免与已有实例冲突。
pub fn find_free_port(preferred: u16) -> u16 {
    for p in preferred..preferred.saturating_add(200) {
        if TcpListener::bind(("127.0.0.1", p)).is_ok() {
            return p;
        }
    }
    TcpListener::bind(("127.0.0.1", 0))
        .ok()
        .and_then(|l| l.local_addr().ok())
        .map(|a| a.port())
        .unwrap_or(preferred)
}

/// 给 Command 附加 Windows 静默标志（不弹控制台）。
#[cfg(windows)]
fn no_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    // CREATE_NO_WINDOW
    cmd.creation_flags(0x0800_0000);
}

#[cfg(not(windows))]
fn no_window(_cmd: &mut Command) {}

/// 启动 daemon：仅绑定 loopback，本地免认证，数据目录隔离。
pub fn start_daemon(bin: &Path, data_dir: &Path, api_port: u16) -> std::io::Result<DaemonHandle> {
    std::fs::create_dir_all(data_dir)?;

    let mut cmd = Command::new(bin);
    cmd.arg("--api-port")
        .arg(api_port.to_string())
        .arg("--data-dir")
        .arg(data_dir)
        .arg("--listen")
        .arg("127.0.0.1")
        .arg("--mode")
        .arg("full")
        // 本地 loopback：允许前端直接访问；不暴露到局域网。
        .env("REST_ALLOW_UNAUTHENTICATED", "1")
        .env("REST_ALLOWED_ORIGINS", "http://localhost:1420,tauri://localhost,http://tauri.localhost")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    no_window(&mut cmd);

    let child = cmd.spawn()?;
    Ok(DaemonHandle {
        child,
        api_port,
        data_dir: data_dir.to_path_buf(),
        bin: bin.to_path_buf(),
    })
}

fn parse_addr(port: u16) -> Option<SocketAddr> {
    format!("127.0.0.1:{}", port).parse().ok()
}

/// 端口是否在监听
pub fn is_port_open(port: u16) -> bool {
    if let Some(addr) = parse_addr(port) {
        return TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok();
    }
    false
}

/// 健康检查：HTTP GET /health，要求 200 且 status=ok。
pub fn health_check(api_port: u16) -> bool {
    let addr = match parse_addr(api_port) {
        Some(a) => a,
        None => return false,
    };
    let mut stream = match TcpStream::connect_timeout(&addr, Duration::from_millis(400)) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(800)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(400)));

    let req = "GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
    if stream.write_all(req.as_bytes()).is_err() {
        return false;
    }
    let mut buf = String::new();
    if stream.read_to_string(&mut buf).is_err() {
        return false;
    }
    buf.contains("200 OK") && buf.contains("\"status\":\"ok\"")
}

/// 停止 daemon（跨平台：Unix SIGKILL / Windows TerminateProcess）。
pub fn stop_daemon(handle: &mut DaemonHandle) {
    let _ = handle.child.kill();
    let _ = handle.child.wait();
}
