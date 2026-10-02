// Agent Universe v3.6.0 — 桌面工作台 Tauri 壳
// 架构：Tauri 主进程（Rust）管理 gsn-daemon（本地后端，HTTP :4002），
// 前端为工作台 SPA。提供单实例、系统托盘、关闭隐藏、默认工作区、
// 模型提供商配置、本地终端、文件浏览与预览数据等能力。

mod daemon;
mod workspace;

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use workspace::AppConfig;

struct AppState {
    daemon: Mutex<Option<daemon::DaemonHandle>>,
    config: Mutex<AppConfig>,
    workspace: String,
}

#[derive(Serialize)]
struct AppInfo {
    version: String,
    platform: String,
    workspace: String,
    daemon_data_dir: String,
    config_path: String,
}

#[derive(Serialize, Clone)]
struct DaemonStatusOut {
    found: bool,
    bin: String,
    running: bool,
    port: u16,
    healthy: bool,
}

#[derive(Serialize)]
struct ShellResult {
    stdout: String,
    stderr: String,
    code: Option<i32>,
}

#[derive(Serialize)]
struct FileEntry {
    name: String,
    path: String,
    is_dir: bool,
    size: u64,
}

fn platform_name() -> String {
    if cfg!(windows) {
        "Windows".to_string()
    } else if cfg!(target_os = "macos") {
        "macOS".to_string()
    } else {
        "Linux".to_string()
    }
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[tauri::command]
fn get_app_info(state: tauri::State<AppState>) -> AppInfo {
    AppInfo {
        version: "3.6.0".to_string(),
        platform: platform_name(),
        workspace: state.workspace.clone(),
        daemon_data_dir: workspace::daemon_data_dir().display().to_string(),
        config_path: workspace::config_file().display().to_string(),
    }
}

#[tauri::command]
fn get_daemon_status(state: tauri::State<AppState>) -> DaemonStatusOut {
    let exe_dir = daemon::current_exe_dir();
    let found_bin = daemon::find_daemon_binary(&exe_dir);
    let default_port = state.config.lock().expect("config lock").api_port;

    let mut guard = state.daemon.lock().expect("daemon lock");
    if let Some(h) = guard.as_mut() {
        let healthy = daemon::health_check(h.api_port);
        return DaemonStatusOut {
            found: true,
            bin: h.bin.display().to_string(),
            running: healthy,
            port: h.api_port,
            healthy,
        };
    }
    // 无托管实例：检查默认端口是否已有外部 daemon
    if daemon::is_port_open(default_port) {
        return DaemonStatusOut {
            found: found_bin.is_some(),
            bin: found_bin.map(|b| b.display().to_string()).unwrap_or_default(),
            running: true,
            port: default_port,
            healthy: daemon::health_check(default_port),
        };
    }
    DaemonStatusOut {
        found: found_bin.is_some(),
        bin: found_bin.map(|b| b.display().to_string()).unwrap_or_default(),
        running: false,
        port: default_port,
        healthy: false,
    }
}

#[tauri::command]
async fn start_daemon(state: tauri::State<'_, AppState>) -> Result<DaemonStatusOut, String> {
    let exe_dir = daemon::current_exe_dir();
    let bin = daemon::find_daemon_binary(&exe_dir).ok_or_else(|| {
        "找不到 gsn-daemon：请设置 GSN_DAEMON_BIN 环境变量，或把 gsn-daemon 放入 bin/ 目录。"
            .to_string()
    })?;
    let port = {
        let cfg = state.config.lock().expect("config lock").clone();
        daemon::find_free_port(cfg.api_port)
    };
    let data_dir = workspace::daemon_data_dir();
    let bin_c = bin.clone();

    let result = tokio::task::spawn_blocking(move || {
        let mut handle = daemon::start_daemon(&bin_c, &data_dir, port).map_err(|e| e.to_string())?;
        let mut healthy = false;
        for _ in 0..60 {
            if daemon::health_check(port) {
                healthy = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        if !healthy {
            daemon::stop_daemon(&mut handle);
            return Err("daemon 已启动但健康检查超时（约 12 秒）。".to_string());
        }
        Ok(handle)
    })
    .await
    .map_err(|e| e.to_string())?;

    let handle = result?;
    let port = handle.api_port;
    let bin_disp = handle.bin.display().to_string();
    *state.daemon.lock().expect("daemon lock") = Some(handle);
    Ok(DaemonStatusOut {
        found: true,
        bin: bin_disp,
        running: true,
        port,
        healthy: true,
    })
}

#[tauri::command]
fn stop_daemon(state: tauri::State<AppState>) -> Result<(), String> {
    let mut guard = state.daemon.lock().expect("daemon lock");
    if let Some(mut h) = guard.take() {
        daemon::stop_daemon(&mut h);
    }
    Ok(())
}

#[tauri::command]
async fn restart_daemon(state: tauri::State<'_, AppState>) -> Result<DaemonStatusOut, String> {
    {
        let mut guard = state.daemon.lock().expect("daemon lock");
        if let Some(mut h) = guard.take() {
            daemon::stop_daemon(&mut h);
        }
    }
    start_daemon(state).await
}

#[tauri::command]
fn get_config(state: tauri::State<AppState>) -> AppConfig {
    state.config.lock().expect("config lock").clone()
}

#[tauri::command]
fn save_config(cfg: AppConfig, state: tauri::State<AppState>) -> Result<(), String> {
    workspace::save_config(&cfg).map_err(|e| e.to_string())?;
    *state.config.lock().expect("config lock") = cfg;
    Ok(())
}

#[tauri::command]
fn run_shell(cmd: String, cwd: Option<String>) -> ShellResult {
    let cwd_path = cwd
        .map(PathBuf::from)
        .unwrap_or_else(workspace::default_workspace_dir);

    let mut command;
    #[cfg(windows)]
    {
        command = Command::new("cmd");
        command.arg("/C").arg(&cmd);
    }
    #[cfg(not(windows))]
    {
        command = Command::new("sh");
        command.arg("-c").arg(&cmd);
    }
    command.current_dir(&cwd_path);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }

    match command.output() {
        Ok(o) => ShellResult {
            stdout: String::from_utf8_lossy(&o.stdout).to_string(),
            stderr: String::from_utf8_lossy(&o.stderr).to_string(),
            code: o.status.code(),
        },
        Err(e) => ShellResult {
            stdout: String::new(),
            stderr: e.to_string(),
            code: Some(-1),
        },
    }
}

#[tauri::command]
fn list_dir(path: String) -> Result<Vec<FileEntry>, String> {
    let mut entries = Vec::new();
    for item in fs::read_dir(&path).map_err(|e| e.to_string())? {
        let item = item.map_err(|e| e.to_string())?;
        let meta = item.metadata();
        let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        entries.push(FileEntry {
            name: item.file_name().to_string_lossy().to_string(),
            path: item.path().display().to_string(),
            is_dir,
            size,
        });
    }
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
    Ok(entries)
}

#[tauri::command]
fn read_text(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn read_binary_base64(path: String) -> Result<String, String> {
    use base64::Engine;
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

#[tauri::command]
async fn pick_folder() -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(|| {
        rfd::FileDialog::new()
            .pick_folder()
            .map(|p| p.display().to_string())
    })
    .await
    .map_err(|e| e.to_string())
}

fn stop_all_daemons(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut guard = state.daemon.lock().expect("daemon lock");
    if let Some(mut h) = guard.take() {
        daemon::stop_daemon(&mut h);
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let icon = app
        .default_window_icon()
        .expect("默认窗口图标")
        .clone();

    TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .menu(&menu)
        .tooltip("Agent Universe")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "quit" => {
                stop_all_daemons(app);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app);
        }))
        .setup(|app| {
            let workspace = workspace::ensure_default_workspace().map_err(|e| {
                eprintln!("⚠️ 默认工作区初始化失败: {e}");
                e
            })?;
            let config = workspace::load_config();
            app.manage(AppState {
                daemon: Mutex::new(None),
                config: Mutex::new(config),
                workspace: workspace.display().to_string(),
            });
            build_tray(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // 关闭窗口=隐藏到后台（托盘常驻），不退出应用。
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            get_daemon_status,
            start_daemon,
            stop_daemon,
            restart_daemon,
            get_config,
            save_config,
            run_shell,
            list_dir,
            read_text,
            read_binary_base64,
            pick_folder
        ])
        .build(tauri::generate_context!())
        .expect("构建 Tauri 应用失败");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            stop_all_daemons(app_handle);
        }
    });
}
