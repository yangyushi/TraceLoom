# IR Design: Messages and Blocks Are the Graph

## Core Principle

One JSONL line becomes one `Message`. One content element inside that line becomes one `Block`. Both `Message` and `Block` are graph vertices; there is no separate persisted `Node` or `Edge` layer.

The graph is derived from:

- `message.parent_id -> message.id` for conversation edges
- `message.id -> block.id` for containment edges
- `block.tool_call_id -> block.id` for tool result edges

## Types

### `Trajectory`

| Field | Meaning |
| --- | --- |
| `session_id` | Conversation/session identifier |
| `messages` | Source-of-truth message envelopes |
| `orphans` | Message IDs whose `parent_id` is missing |
| `warnings` | Non-fatal parser or validation issues |

`raw_json` is intentionally preserved on each `Message` for debugging and inspector display. This keeps parsing lossless for current desktop workloads. If very large logs become a problem, the future path is streaming or external raw-line storage, not adding source-specific fields to the IR.

### `Message`

| Field | Meaning |
| --- | --- |
| `id` | Unique message ID |
| `parent_id` | Previous message in the conversation chain, when known |
| `role` | `user`, `assistant`, `system`, `tool`, or another source role |
| `timestamp` | Optional RFC 3339 timestamp |
| `blocks` | Ordered content blocks |
| `is_sidechain` | True for branched/non-mainline records |
| `raw_json` | Original JSONL line |

Every parsed record that represents user-visible content keeps its message envelope. Parsers do not collapse a single-block message into a block-only node.

### `Block`

| Field | Meaning |
| --- | --- |
| `id` | Unique block ID, distinct from the parent message ID |
| `kind` | Render kind such as `user`, `text`, `think`, `tool_call`, `tool_result`, `snapshot`, or `custom` |
| `content` | Typed payload: `Text`, `Thinking`, `ToolUse`, `ToolResult`, `Snapshot`, or `Custom` |
| `tool_call_id` | Referenced tool call block for tool results |

Unknown source blocks are preserved as `Content::Custom { kind, payload }` and a parser warning is added. Source-specific data belongs in `raw_json` or `Content::Custom.payload`, never in dedicated IR fields.

## Parser Responsibilities

Parsers are the only source-specific layer. They must:

- Preserve raw lines in `Message.raw_json`
- Preserve supported and unknown user-visible content blocks
- Represent encrypted reasoning without attempting decryption
- Preserve tool result error state when the source provides it
- Emit warnings for unknown or skipped meaningful records
- Return a trajectory that passes `Trajectory::validate()`

Pure setup/session records may be skipped only when they are metadata. If a skipped-looking record has parent links or visible payload, it must be preserved or warned about.

## Validation

`Trajectory::validate()` enforces hard invariants:

- Duplicate message or block IDs are errors
- Cycles in the parent chain are errors
- A parent timestamp more than one second after a child timestamp is an error
- Missing parents are fail-soft: the child message remains, its ID is added to `orphans`, and a warning is emitted
- Invalid `tool_call_id` references are warnings

Validation is idempotent. Re-running it does not duplicate generated orphans or warnings.

## Ordering

Rust and TypeScript use the same ordering rule:

1. Parent messages always appear before children when the parent exists.
2. Among messages available at the same graph depth, the earlier timestamp appears first.
3. Source order is the stable tie-breaker.
4. Cyclic or otherwise unvisited messages are appended in source order after validation fallback.

The frontend helper in `src/lib/order.ts` is shared by Dots, Bricks, keyboard navigation, and inspector selection lookup.

## Rendering

### Dots

Messages form a vertical spine. Blocks from the same message branch horizontally on the same row as their parent message:

- Message node: `x = 0`, `y = ordered_index * adaptive_spacing`
- Block node: `x = 140 + block_index * 45`, `y = message_y`
- Chain edges are solid gray
- Containment edges are dashed gray
- Tool result edges are solid blue

Cytoscape uses a preset layout; positions are computed from the shared ordered message list.

### Bricks

Bricks renders the same ordered message list as a vertical timeline. Each message brick is followed by its block bricks, indented below it. Keyboard navigation uses the shared ordered item list.

### Inspector

The inspector renders message metadata, raw JSON, block metadata, raw content, markdown, and JSON sections. Markdown input is escaped before rendering, rendered HTML is sanitized, and expensive embedded JSON probing is bounded.

## Tauri Commands

Every Tauri command must be whitelisted in both:

- `src-tauri/permissions/default.toml`
- `src-tauri/capabilities/default.json`

Current commands are `load_trajectory`, `list_jsonl_files`, and `read_file_text`.
