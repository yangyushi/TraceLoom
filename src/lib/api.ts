import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { Trajectory } from "../types/ir";

export async function pickAndLoadTrajectory(): Promise<Trajectory | null> {
  const file = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "JSONL", extensions: ["jsonl"] }],
  });
  if (!file) return null;

  const path = Array.isArray(file) ? file[0] : file;
  return invoke<Trajectory>("load_trajectory", { path });
}

export async function pickFolder(): Promise<string | null> {
  const folder = await open({
    multiple: false,
    directory: true,
  });
  if (!folder) return null;
  return Array.isArray(folder) ? folder[0] : folder;
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
