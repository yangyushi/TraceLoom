# Parser Reference

## Detection Heuristics

`parser/detector.rs` inspects the first 5 non-empty JSON lines. It tries these checks in order:

| Format | Fingerprint |
|--------|-------------|
| Claude | `uuid` + `parentUuid` + `sessionId` |
| Codex | `payload` with `type` field, or top-level `type == "session_meta"` |
| OpenClaw | `id` + `parentId` + `type`, or `type == "session"` + `version` |

If none match, returns `ParseError::UnknownFormat`.

## Claude Parser (`parser/claude.rs`)

Handles Claude agent `.jsonl` exports.

**Record types consumed:**
- `"user"` → `Node { kind: "user", role: Role::user(), content: Text(...) }`
- `"assistant"` → multiple blocks per message:
  - `thinking` → `kind: "think"`
  - `text` → `kind: "text"` (or `"respond"` if `stop_reason == "end_turn"`)
  - `tool_use` → `kind: "tool_call"`
- `"file-history-snapshot"` → `kind: "snapshot"`
- `"last-prompt"` → skipped

**Sidechains:** All Claude nodes are currently marked `is_sidechain: true` because the original data model treated every message as a potential branch.

**Tool results:** Parsed from `toolUseResult` field on user-type records.

## Codex Parser (`parser/codex.rs`)

Handles Codex rollout `.jsonl` exports.

**Record types consumed:**
- `"response_item"` / `"event_msg"` → parsed via `payload`
- `"session_meta"` → extracts `session_id`
- `"turn_context"` → skipped

**Content blocks inside `response_item`:**
- `input_text` / `output_text` → `Text`
- `reasoning` → `Thinking { encrypted: true, text: "" }` (we do not decrypt)
- `input_image` → skipped

**Events (`event_msg`):**
Mapped to `Content::Custom` variants: `task_started`, `task_complete`, `token_count`, `agent_message`, `agent_reasoning`, `user_message`.

**IDs:** Synthetic IDs generated from `turn_id` + an internal counter because Codex records do not always have stable node IDs.

## OpenClaw Parser (`parser/openclaw.rs`)

Handles OpenClaw session `.jsonl` exports.

**Record types consumed:**
- `"session"` → skipped (extracts `session_id` only)
- `"model_change"` / `"thinking_level_change"` → `Content::Custom`
- `"custom"` → `Content::Custom`
- `"message"` → parsed via `message` sub-object:
  - `text` blocks → `Text`
  - `toolCall` blocks → `ToolUse`
  - `toolResult` role → `ToolResult`

**Parent linkage:** Uses `parentId` directly from the record.

## Public API

```rust
// parser/mod.rs
pub fn parse_file(path: &str) -> Result<Trajectory, ParseError> {
    let contents = std::fs::read_to_string(path)?;
    let format = detector::detect_format(&contents)?;
    let mut trajectory = match format { ... };
    trajectory.validate()?;
    Ok(trajectory)
}
```

Every parser must:
1. Set `trajectory.session_id` if found in the file.
2. Create `Node` objects with `parent_id` populated.
3. Create `Edge { from: parent_id, to: node.id }` for every node that has a parent.
4. Return a `Trajectory` that will pass `validate()`.
