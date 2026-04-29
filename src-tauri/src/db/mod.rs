pub mod commands;
pub mod migrations;
pub mod models;

use rusqlite::Connection;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri::Manager;

pub fn db_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let app_dir = app_handle
        .path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&app_dir).map_err(|e| e.to_string())?;
    Ok(app_dir.join("traceloom.db"))
}

pub fn init_db(app_handle: &AppHandle) -> Result<Connection, String> {
    let path = db_path(app_handle)?;
    let mut conn = Connection::open(&path).map_err(|e| e.to_string())?;
    migrations::run_migrations(&mut conn)?;
    Ok(conn)
}
