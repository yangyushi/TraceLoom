use super::models::RecentWorkspace;
use crate::state::AppState;
use chrono::Utc;
use rusqlite::params;
use tauri::State;

#[tauri::command]
pub fn list_recent_workspaces(state: State<AppState>) -> Result<Vec<RecentWorkspace>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, file_path, name, last_opened FROM recent_workspaces ORDER BY last_opened DESC")
        .map_err(|e| e.to_string())?;

    let workspaces = stmt
        .query_map([], |row| {
            Ok(RecentWorkspace {
                id: row.get(0)?,
                file_path: row.get(1)?,
                name: row.get(2)?,
                last_opened: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(workspaces)
}

#[tauri::command]
pub fn add_recent_workspace(
    state: State<AppState>,
    file_path: String,
    name: String,
) -> Result<RecentWorkspace, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let now = Utc::now();

    conn.execute(
        "INSERT INTO recent_workspaces (file_path, name, last_opened) VALUES (?1, ?2, ?3)
         ON CONFLICT(file_path) DO UPDATE SET name = excluded.name, last_opened = excluded.last_opened",
        params![&file_path, &name, now.to_rfc3339()],
    )
    .map_err(|e| e.to_string())?;

    let id = conn.last_insert_rowid();
    Ok(RecentWorkspace {
        id,
        file_path,
        name,
        last_opened: now,
    })
}

#[tauri::command]
pub fn remove_recent_workspace(state: State<AppState>, id: i64) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM recent_workspaces WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}
