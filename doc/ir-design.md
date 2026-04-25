# IR Design — Messages and Blocks ARE the Graph

## Core Principle

One JSONL line = one `Message`.  
One content element inside a message = one `Block`.  
Both `Message` and `Block` are first-class vertices in the render graph.  
There is no separate `Node` / `Edge` layer — the graph is derived on demand from the domain model.

## Types

### Message

Represents a single JSONL line from the source file.

| Field | Meaning |
|-------|---------|
| `id` | Unique identifier (usually from the JSONL line) |
| `parent_id` | ID of the previous message in the conversation chain |
| `role` | `"user"`, `"assistant"`, `"system"`, `"tool"`, etc. |
| `timestamp` | Optional RFC-3339 timestamp |
| `blocks` | Ordered list of `Block`s parsed from the message's content array |
| `is_sidechain` | True for branched / non-mainline messages |
| `raw_json` | The original JSON string from the JSONL file (for debugging) |

Key invariant: every message gets its own envelope. There is no "absorption" optimization that collapses a single-block message into one node. The message envelope always exists so the parent chain is always intact.

### Block

Represents one element in the content collection of a message.

| Field | Meaning |
|-------|---------|
| `id` | Unique identifier (distinct from its parent message's id) |
| `kind` | `"user"`, `"text"`, `"think"`, `"tool_call"`, `"tool_result"`, `"snapshot"`, etc. |
| `content` | Typed payload (`Text`, `ToolUse`, `ToolResult`, `Thinking`, `Snapshot`, `Custom`) |
| `tool_call_id` | For `tool_result` blocks: the id of the `tool_call` block they respond to |

Key invariant: a block's id is never equal to its parent message's id. Parsers synthesize distinct block ids (e.g. `{msg-id}-block`, `{msg-id}-text-0`) to avoid collisions.

### Trajectory

Top-level container.

| Field | Meaning |
|-------|---------|
| `session_id` | Conversation / session identifier |
| `messages` | Ordered list of `Message`s (source of truth) |
| `orphans` | Message ids whose `parent_id` does not exist in the trajectory |
| `warnings` | Non-fatal issues found during validation |

There are no `nodes` or `edges` arrays. All graph relationships are derived from `messages`:

- Conversation chain: `message.parent_id → message.id`
- Containment: implicit — a block belongs to the message whose `blocks` array contains it
- Tool link: `block.tool_call_id → block.id` (cross-message reference)

## Parser Responsibilities

Each parser (Claude, OpenClaw, Codex) reads a `.jsonl` file and produces a `Trajectory` of `Message`s.

1. Preserve the raw line — every `Message` stores its original JSON string in `raw_json`
2. Create distinct block ids — never reuse the message id as a block id
3. Set `tool_call_id` only when present — do not fall back to empty string or message uuid
4. Do not call `flatten()` — there is no graph flattening step; the domain model is the graph

### Example: Claude parser

Input JSONL line (assistant with tool use):
```json
{"type":"assistant","uuid":"msg-1","parentUuid":"msg-0","message":{"role":"assistant","content":[{"type":"tool_use","id":"tool-a","name":"Read","input":{}}]}}
```

Produces:

```rust
Message {
  id: "msg-1",
  parent_id: Some("msg-0"),
  role: Role("assistant"),
  blocks: [
    Block {
      id: "tool-a",
      kind: "tool_call",
      content: ToolUse { name: "Read", input: {} },
      tool_call_id: None,
    }
  ],
  raw_json: Some("<original json string>"),
}
```

## Validation (`Trajectory::validate()`)

Run after parsing. Checks:

1. Duplicate ids — no two messages or blocks may share an id
2. Orphan parents — every `message.parent_id` must reference an existing message
3. Temporal consistency — a parent message's timestamp must not be after its child's
4. Cycles — the `parent_id` chain must not contain cycles
5. Invalid tool_call_id — every `block.tool_call_id` must reference an existing block id (warning, not error)


## Rendering

### Dots view  `DotsRenderer`

Messages form a vertical spine; blocks branch horizontally to the right.

- Message node at `x = 0`, `y = index  adaptive_spacing`
  - Colored by `role`
  - Label = capitalized initial of role ("U", "A", "S", "T")
- Block node at `x = 140 + i  45`, `y = message_y + (i - n/2)  50`
  - Colored by `kind`
  - Lower index = closer to the message spine
- Edges
  - `parent_id → message` (solid gray) — conversation chain
  - `message → block` (dashed gray) — containment
  - `tool_call_id → tool_result` (solid blue) — tool link

Cytoscape uses a `preset` layout: positions are computed from the `messages` / `blocks` data and assigned directly.

### Bricks view  `BricksRenderer`

Messages render as a vertical timeline of "bricks". Each message brick shows its role and block count. Blocks are indented sub-bricks underneath their parent message.

### Detail panel  `NodeDetail`

- Message selected: shows metadata (id, role, parent, timestamp, block count) + collapsible "Raw JSON" section displaying the original JSONL line
- Block selected: shows metadata (id, kind, tool_call_id) + parsed content with Raw / Markdown toggle


## File Map

| Rust | Purpose |
|------|---------|
| `src-tauri/src/ir/message.rs` | `Message`, `Block`, `Role` |
| `src-tauri/src/ir/trajectory.rs` | `Trajectory`, `validate()`, `topological_order()` |
| `src-tauri/src/ir/content.rs` | `Content` enum |
| `src-tauri/src/parser/{claude,openclaw,codex}.rs` | Format-specific parsers |

| TypeScript | Purpose |
|------------|---------|
| `src/types/ir.ts` | Frontend type mirrors |
| `src/components/DotsRenderer.svelte` | Cytoscape graph with preset layout |
| `src/components/BricksRenderer.svelte` | Vertical brick timeline |
| `src/components/NodeDetail.svelte` | Inspector panel for messages / blocks |
| `src/lib/colors.ts` | Color schema by `kind` / `role` |
