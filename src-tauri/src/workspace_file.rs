use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkspaceFile {
    pub version: u32,
    pub workspace: WorkspaceMeta,
    pub sources: Vec<WorkspaceSource>,
    pub annotations: Vec<WorkspaceAnnotation>,
    pub bookmarks: Vec<WorkspaceBookmark>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkspaceMeta {
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkspaceSource {
    pub file_path: String,
    pub display_name: String,
    pub color_key: String,
    pub sort_order: i32,
    pub visibility_state: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkspaceAnnotation {
    pub name: String,
    pub trajectory_source_index: usize,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkspaceBookmark {
    pub annotation_index: usize,
    pub node_id: String,
    pub comment: Option<String>,
    pub created_at: String,
}

#[tauri::command]
pub fn export_workspace(path: String, workspace_json: String) -> Result<(), String> {
    std::fs::write(&path, workspace_json).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn import_workspace(path: String) -> Result<WorkspaceFile, String> {
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let file: WorkspaceFile = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    Ok(file)
}
