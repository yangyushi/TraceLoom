import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { Trajectory } from "../types/ir";
import type { WorkspaceFile, RecentWorkspace } from "../types/workspace";

export async function pickFolder(): Promise<string | null> {
  const folder = await open({
    multiple: false,
    directory: true,
  });
  if (!folder) return null;
  return Array.isArray(folder) ? folder[0] : folder;
}

export async function pickJsonlFile(): Promise<string | null> {
  const file = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "JSONL", extensions: ["jsonl"] }],
  });
  if (!file) return null;
  return Array.isArray(file) ? file[0] : file;
}

export async function listJsonlFiles(folder: string): Promise<string[]> {
  return invoke<string[]>("list_jsonl_files", { folder });
}

export async function loadTrajectory(path: string): Promise<Trajectory> {
  return invoke<Trajectory>("load_trajectory", { path });
}

export async function readFileText(path: string): Promise<string> {
  return invoke<string>("read_file_text", { path });
}

// Workspace file I/O

export async function exportWorkspace(path: string, workspaceJson: string): Promise<void> {
  return invoke<void>("export_workspace", { path, workspaceJson });
}

export async function importWorkspace(path: string): Promise<WorkspaceFile> {
  return invoke<WorkspaceFile>("import_workspace", { path });
}

export async function pickWorkspaceFile(): Promise<string | null> {
  const file = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "TraceLoom Workspace", extensions: ["json"] }],
  });
  if (!file) return null;
  return Array.isArray(file) ? file[0] : file;
}

export async function pickSavePath(): Promise<string | null> {
  const path = await save({
    filters: [{ name: "TraceLoom Workspace", extensions: ["json"] }],
  });
  return path ?? null;
}

// Recent workspaces cache (SQLite)

export async function listRecentWorkspaces(): Promise<RecentWorkspace[]> {
  return invoke<RecentWorkspace[]>("list_recent_workspaces");
}

export async function addRecentWorkspace(filePath: string, name: string): Promise<RecentWorkspace> {
  return invoke<RecentWorkspace>("add_recent_workspace", { filePath, name });
}

export async function removeRecentWorkspace(id: number): Promise<void> {
  return invoke<void>("remove_recent_workspace", { id });
}
