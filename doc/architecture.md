# Architecture Reference

## Stack

| Layer | Technology |
|-------|------------|
| Backend | Rust (Tauri v2) |
| Frontend | Svelte 5 + TypeScript |
| Build tool | Vite |
| Markdown | `marked` (npm) |
| Syntax highlighting | PrismJS |

## Module Structure

```
src-tauri/
├── src/
│   ├── main.rs              # Tauri entry point; defines commands
│   ├── lib.rs               # Re-exports ir + parser for tests
│   ├── error.rs             # ParseError enum
│   ├── ir/
│   │   ├── mod.rs           # Public re-exports
│   │   ├── node.rs          # Node, NodeId, Role
│   │   ├── edge.rs          # Edge
│   │   ├── trajectory.rs    # Trajectory, validation, topo sort
│   │   └── content.rs       # Content enum
│   └── parser/
│       ├── mod.rs           # parse_file() facade
│       ├── detector.rs      # SourceFormat detection
│       ├── claude.rs        # Claude JSONL parser
│       ├── codex.rs         # Codex JSONL parser
│       └── openclaw.rs      # OpenClaw JSONL parser
├── capabilities/default.json
├── permissions/default.toml
└── tauri.conf.json

src/
├── App.svelte               # Main layout, panels, resize handles
├── main.ts                  # Vite entry
├── components/
│   ├── DotsRenderer.svelte  # SVG dot stream
│   ├── BricksRenderer.svelte# HTML card stack
│   └── NodeDetail.svelte    # Inspector panel
├── lib/
│   ├── api.ts               # Tauri invoke wrappers
│   └── colors.ts            # Color schema by kind
└── types/
    └── ir.ts                # TypeScript IR mirrors
```

## Data Flow

```
.jsonl file
    |
    v
+---------------+
|   detector    |  <-- inspects first 5 JSON lines
+---------------+
    |
    v
+---------------+
| parser module |  <-- claude | codex | openclaw
+---------------+
    |
    v
+---------------+
|  Trajectory   |  <-- IR: nodes + edges
|  validate()   |  <-- DAG checks, temporal checks
+---------------+
    |
    v
+---------------+
| Tauri command |  <-- load_trajectory(path)
+---------------+
    |
    v
+---------------+
|   Svelte UI   |  <-- topo sort, render dots/bricks
+---------------+
    |
    v
+---------------+
|  NodeDetail   |  <-- inspect, markdown toggle
+---------------+
```

## Security Model (Tauri v2)

Commands are not callable from the frontend unless explicitly allowed:

1. Define permission in `permissions/default.toml`:
   ```toml
   [[permission]]
   identifier = "allow-my-command"
   commands.allow = ["my_command"]
   ```

2. Reference it in `capabilities/default.json`:
   ```json
   {
     "permissions": [
       "core:default",
       "allow-my-command"
     ]
   }
   ```

3. Register the command in `main.rs` via `tauri::generate_handler!`.

Missing any step results in an ACL denial at runtime.
