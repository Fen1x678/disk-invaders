// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod brain_cmds;
mod commands;
mod system;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(commands::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::system_info,
            commands::start_scan,
            commands::cancel_scan,
            commands::list_dir,
            commands::delete_items,
            brain_cmds::brain_status,
            brain_cmds::brain_score,
            brain_cmds::brain_teach,
            brain_cmds::brain_export,
            brain_cmds::brain_forget,
            brain_cmds::voice_status,
            brain_cmds::voice_pull,
            brain_cmds::voice_chat,
            brain_cmds::voice_stop,
        ])
        .run(tauri::generate_context!())
        .expect("error while starting Disk Invaders");
}
