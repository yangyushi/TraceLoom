#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::Path;
use tauri::Manager;
use traceloom_lib::db::commands::*;
use traceloom_lib::db::init_db;
use traceloom_lib::ir::Trajectory;
use traceloom_lib::parser;
use traceloom_lib::state::AppState;
use traceloom_lib::workspace_file::*;

#[tauri::command]
async fn load_trajectory(path: String) -> Result<Trajectory, String> {
    tauri::async_runtime::spawn_blocking(move || {
        parser::parse_file(&path).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn read_file_text(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        std::fs::read_to_string(&path).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn list_jsonl_files(folder: String) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || list_jsonl_files_blocking(folder))
        .await
        .map_err(|e| e.to_string())?
}

fn list_jsonl_files_blocking(folder: String) -> Result<Vec<String>, String> {
    let path = Path::new(&folder);
    if !path.is_dir() {
        return Err("Not a directory".to_string());
    }

    let mut files = Vec::new();
    let entries = std::fs::read_dir(path).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "jsonl" {
                    files.push(path.to_string_lossy().to_string());
                }
            }
        }
    }

    files.sort();
    Ok(files)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let conn = init_db(app.handle())?;
            let db_path = traceloom_lib::db::db_path(app.handle())?;
            app.manage(AppState::new(db_path, conn));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_trajectory,
            list_jsonl_files,
            read_file_text,
            export_workspace,
            import_workspace,
            list_recent_workspaces,
            add_recent_workspace,
            remove_recent_workspace,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn main() {
    run();
}
