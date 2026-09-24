// Agent Universe v2.3.6 — Tauri 2 跨平台客户端
// 一套源码覆盖 macOS / Windows / Linux / iOS / Android

#[tauri::command]
fn get_platform() -> String {
    if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "windows") {
        "Windows"
    } else if cfg!(target_os = "ios") {
        "iOS"
    } else if cfg!(target_os = "android") {
        "Android"
    } else if cfg!(target_os = "linux") {
        "Linux"
    } else {
        "Unknown"
    }
    .to_string()
}

#[tauri::command]
fn get_sdk_version() -> String {
    "2.3.6".to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_platform, get_sdk_version])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
