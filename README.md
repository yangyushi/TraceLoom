# LLM Tracer

A cross-platform desktop application that visualises LLM agent message histories as DAGs.

![screenshot-placeholder](doc/screenshot.png)

## What it does

LLM Tracer reads `.jsonl` log files from various sources (Claude, Codex, OpenClaw), parses them into a source-agnostic intermediate representation, and renders the conversation flow as an interactive graph.

**Features**
- **Two visual themes**: `dots` (stream of circles) and `bricks` (vertical stack of cards)
- **Source-agnostic IR**: extensible enough for future LLM agent message kinds
- **Node inspection**: click any node to view raw content or rendered markdown
- **DAG validation**: temporal checks, cycle detection, orphan handling

## Quick start

```bash
# Install dependencies
npm install

# Run in dev mode
make dev            # Linux / macOS
.\build.ps1 -Dev    # Windows

# Build for release
make                # Linux / macOS
.\build.ps1         # Windows
```

See [doc/building.md](doc/building.md) for detailed build instructions, platform-specific prerequisites, and troubleshooting.

## Project structure

```
llm-tracer/
├── src/                    # Svelte + TypeScript frontend
│   ├── components/         # DotsRenderer, BricksRenderer, NodeDetail
│   ├── lib/                # API wrappers, layout helpers
│   └── types/              # TypeScript IR mirrors
├── src-tauri/              # Rust backend
│   ├── src/ir/             # Universal IR (Trajectory, Node, Edge, Content)
│   ├── src/parser/         # Claude, Codex, OpenClaw parsers
│   ├── capabilities/       # Tauri v2 capability definitions
│   ├── permissions/        # Tauri v2 permission definitions
│   └── icons/              # Application icon
├── test/
│   └── samples/            # Fixture files for testing
│       ├── claude/
│       ├── codex/
│       └── openclaw/
└── doc/                    # Detailed documentation
```

## Tech stack

- **Backend**: Rust + Tauri v2
- **Frontend**: Svelte 5 + TypeScript + Vite
- **Markdown rendering**: `marked`
- **Bundling**: Tauri CLI (`.deb`, `.rpm`, `.AppImage`, `.dmg`, `.msi`)

## License

MIT
