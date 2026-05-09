import type { Trajectory } from "./ir";

// Runtime types (in-memory, used by UI components)

export interface TrajectorySource {
  id: number;
  display_name: string;
  source_type: "imported_file" | "saved";
  file_path: string | null;
  trajectory: Trajectory | null;
  color_key: string;
  sort_order: number;
  visibility_state: "unloaded" | "loaded" | "hidden";
}

export interface Annotation {
  id: number;
  name: string;
  trajectory_source_id: number;
  created_at: string;
  updated_at: string;
}

export interface Bookmark {
  id: number;
  annotation_id: number;
  node_id: string;
  comment: string | null;
  created_at: string;
}

export interface WorkspaceState {
  filePath: string | null;
  workspace: { name: string; created_at: string; updated_at: string };
  sources: TrajectorySource[];
  annotations: Annotation[];
  bookmarks: Bookmark[];
  activeAnnotation: Annotation | null;
}

// File format types (serialized to/from JSON file)

export interface WorkspaceFile {
  version: 1;
  workspace: { name: string; created_at: string; updated_at: string };
  sources: WorkspaceFileSource[];
  annotations: WorkspaceFileAnnotation[];
  bookmarks: WorkspaceFileBookmark[];
}

export interface WorkspaceFileSource {
  file_path: string;
  display_name: string;
  color_key: string;
  sort_order: number;
  visibility_state: "unloaded" | "loaded" | "hidden";
}

export interface WorkspaceFileAnnotation {
  name: string;
  trajectory_source_index: number;
  created_at: string;
  updated_at: string;
}

export interface WorkspaceFileBookmark {
  annotation_index: number;
  node_id: string;
  comment: string | null;
  created_at: string;
}

export interface RecentWorkspace {
  id: number;
  file_path: string;
  name: string;
  last_opened: string;
}

// Namespaced ID for rendering: "source_id::node_id"
export type RenderId = string;
