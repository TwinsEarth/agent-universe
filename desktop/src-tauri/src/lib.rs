// Agent Universe v2.5.5 — Tauri 2 桌面客户端壳
// 前端: Vite + HTML/JS，调用 @twinsearth/agent-universe

#[tauri::command]
fn get_sdk_version() -> String {
    "2.5.5".to_string()
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_sdk_version])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
