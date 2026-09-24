// Agent Universe v2.3.4 — Tauri 2 最小壳
// 前端: Vite + HTML/JS，调用 @twinsearth/agent-universe@2.3.4

#[tauri::command]
fn get_sdk_version() -> String {
    "2.3.4".to_string()
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_sdk_version])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
