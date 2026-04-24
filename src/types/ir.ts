export interface Trajectory {
  session_id: string;
  nodes: Node[];
  edges: Edge[];
  orphans: string[];
  warnings: string[];
}

export interface Node {
  id: string;
  parent_id: string | null;
  kind: string;
  role: Role;
  content: Content;
  timestamp: string | null;
  metadata: Record<string, unknown>;
  is_sidechain: boolean;
}

export interface Role {
  0: string;
}

export interface Edge {
  from: string;
  to: string;
}

export type Content =
  | { type: "Empty" }
  | { type: "Text"; data: string }
  | { type: "Thinking"; data: { text: string; encrypted: boolean } }
  | { type: "ToolUse"; data: { name: string; input: unknown } }
  | { type: "ToolResult"; data: { output: string; is_error: boolean } }
  | { type: "Snapshot"; data: { file_path: string | null; description: string } }
  | { type: "Custom"; data: { kind: string; payload: unknown } };
