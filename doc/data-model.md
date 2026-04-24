# Data Model Reference

## IR Types (Rust)

### Trajectory
```rust
pub struct Trajectory {
    pub session_id: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub orphans: Vec<NodeId>,
    pub warnings: Vec<String>,
}
```

- `orphans`: IDs referenced by edges but missing from `nodes`.
- `warnings`: Human-readable strings about broken edges or other non-fatal issues.

### Node
```rust
pub struct Node {
    pub id: NodeId,               // String
    pub parent_id: Option<NodeId>,
    pub kind: String,             // e.g. "user", "think", "tool_call"
    pub role: Role,               // wrapper around String
    pub content: Content,
    pub timestamp: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub is_sidechain: bool,
}
```

`kind` is free-form. Common values: `user`, `assistant`, `think`, `tool_call`, `tool_result`, `snapshot`, `system`, `respond`, `text`.

### Edge
```rust
pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
}
```

### Content
```rust
pub enum Content {
    Empty,
    Text(String),
    Thinking { text: String, encrypted: bool },
    ToolUse { name: String, input: serde_json::Value },
    ToolResult { output: String, is_error: bool },
    Snapshot { file_path: Option<String>, description: String },
    Custom { kind: String, payload: serde_json::Value },
}
```

Serialized with `#[serde(tag = "type", content = "data")]`.

## TypeScript Mirrors

Located in `src/types/ir.ts`. The TypeScript `Content` type is a discriminated union matching the Rust tagged serialization exactly.

## Validation Rules

`Trajectory::validate()` enforces:

1. **No duplicate IDs** — hard error (`InvalidTrajectory`).
2. **No cycles** — DFS detection; hard error.
3. **Temporal consistency** — for every edge `parent -> child`, `parent.timestamp <= child.timestamp`; hard error.
4. **Orphan tracking** — edges to missing nodes are logged as warnings and the missing IDs are stored in `orphans`.

## Topological Ordering

`Trajectory::topological_order()` returns nodes in Kahn-algorithm order with these tiebreakers:
- Roots (in-degree 0) sorted by timestamp ascending.
- Children sorted by timestamp ascending before being enqueued.
- Any unvisited nodes (should not happen in a valid DAG) appended at the end.

The frontend implements an equivalent `topoSort()` in `DotsRenderer.svelte` and `BricksRenderer.svelte`.
