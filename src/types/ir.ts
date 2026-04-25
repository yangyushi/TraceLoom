export interface Trajectory {
  session_id: string;
  messages: Message[];
  orphans: string[];
  warnings: string[];
}

export interface Message {
  id: string;
  parent_id: string | null;
  role: Role;
  timestamp: string | null;
  blocks: Block[];
  is_sidechain: boolean;
  raw_json: string | null;
}

export interface Block {
  id: string;
  kind: string;
  content: Content;
  tool_call_id: string | null;
}

export type Role = string;

export type Content =
  | { type: "Empty" }
  | { type: "Text"; data: string }
  | { type: "Thinking"; data: { text: string; encrypted: boolean } }
  | { type: "ToolUse"; data: { name: string; input: unknown } }
  | { type: "ToolResult"; data: { output: string; is_error: boolean } }
  | { type: "Snapshot"; data: { file_path: string | null; description: string } }
  | { type: "Custom"; data: { kind: string; payload: unknown } };
