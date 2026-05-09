use serde::{Deserialize, Serialize};
use std::path::Path;

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
pub async fn export_workspace(path: String, workspace_json: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || export_workspace_blocking(path, workspace_json))
        .await
        .map_err(|e| e.to_string())?
}

pub fn export_workspace_blocking(path: String, workspace_json: String) -> Result<(), String> {
    let output_path = Path::new(&path);
    let file_name = output_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();

    if file_name.eq_ignore_ascii_case("tauri.conf.json") {
        return Err("Refusing to overwrite Tauri app configuration with a workspace.".to_string());
    }

    serde_json::from_str::<WorkspaceFile>(&workspace_json).map_err(|e| e.to_string())?;

    if output_path.exists() {
        let existing = std::fs::read_to_string(output_path).map_err(|e| e.to_string())?;
        if serde_json::from_str::<WorkspaceFile>(&existing).is_err() {
            return Err(
                "Refusing to overwrite an existing JSON file that is not a TraceLoom workspace."
                    .to_string(),
            );
        }
    }

    std::fs::write(&path, workspace_json).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn import_workspace(path: String) -> Result<WorkspaceFile, String> {
    tauri::async_runtime::spawn_blocking(move || import_workspace_blocking(path))
        .await
        .map_err(|e| e.to_string())?
}

pub fn import_workspace_blocking(path: String) -> Result<WorkspaceFile, String> {
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let file: WorkspaceFile = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn workspace_json() -> String {
        serde_json::json!({
            "version": 1,
            "workspace": {
                "name": "Test",
                "created_at": "2026-04-29T00:00:00.000Z",
                "updated_at": "2026-04-29T00:00:00.000Z"
            },
            "sources": [],
            "annotations": [],
            "bookmarks": []
        })
        .to_string()
    }

    #[test]
    fn export_workspace_refuses_tauri_config() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("tauri.conf.json");
        let result =
            export_workspace_blocking(path.to_string_lossy().to_string(), workspace_json());

        assert!(result.is_err());
        assert!(!path.exists());
    }

    #[test]
    fn export_workspace_refuses_existing_non_workspace_json() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"identifier":"com.traceloom.app"}"#).unwrap();

        let result =
            export_workspace_blocking(path.to_string_lossy().to_string(), workspace_json());

        assert!(result.is_err());
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            r#"{"identifier":"com.traceloom.app"}"#
        );
    }

    #[test]
    fn export_workspace_allows_new_workspace_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("workspace.json");

        export_workspace_blocking(path.to_string_lossy().to_string(), workspace_json()).unwrap();

        let imported = import_workspace_blocking(path.to_string_lossy().to_string()).unwrap();
        assert_eq!(imported.workspace.name, "Test");
    }
}
