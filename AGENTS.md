# AGENTS.md — LLM Tracer

## Purpose

LLM Tracer is a cross-platform desktop app that visualises LLM agent message histories as DAGs. It reads `.jsonl` files from diverse sources, parses them into a single universal intermediate representation (IR), and renders them in multiple visual themes.

---

## Core Principles

### 1. Source-Agnostic IR
All input formats collapse into one `Trajectory` structure. The IR must not contain any source-format-specific fields. If a new source is added, only the parser changes — never the IR.

### 2. Heuristic Over Configuration
We detect source formats by inspecting the first few JSON lines and looking for tell-tale fields. There is no manifest file or user-selected format. If detection fails, we return `UnknownFormat` — never guess silently.

### 3. Temporal Causality is Law
A parent node must never have a timestamp after its child. This is a hard invariant enforced by `Trajectory::validate()`. Any parser that produces backward-time edges is buggy.

### 4. Fail-Soft, Not Fail-Silent
If an edge points to a missing node, the graph still renders. The broken linkage is collected into `Trajectory.orphans` with a warning. We never drop data silently.

### 5. Extensibility by Addition, Not Modification
Adding a new node kind, color, parser, or renderer should require creating new entries/maps — not refactoring existing code paths.

- `node.kind` is a free-form `String`, not a closed enum.
- Colors are keyed by `kind`, not by source format or role.
- Content variants are additive; existing renderers handle unknown variants with sensible fallbacks.

### 6. Two Renderers, One Ordering
Both visual themes (`dots` and `bricks`) consume the same topological ordering. The renderers are views over the same sorted data; they do not each implement their own sort or layout engine.

### 7. No General Graph-Layout Engine
We do not use a force-directed or hierarchical graph-layout library. The renderers are bespoke and lightweight:
- Dots: a vertical SVG stream.
- Bricks: a vertical HTML card stack.
This keeps bundle size small and rendering predictable.

---

## Invariants (Never Violate)

1. **DAG validation is mandatory.** Every `Trajectory` returned by `parse_file()` has passed `validate()`, which checks for duplicate IDs, cycles, and temporal violations.
2. **Colors are keyed by `kind`.** Never map colors by `role`, `source_format`, or `content.type`. The color schema in `src/lib/colors.ts` is the single source of truth.
3. **Topology sort is timestamp-tiebroken.** When two nodes have equal graph depth, the earlier timestamp comes first. Both Rust and TypeScript sort implementations must agree on this.
4. **Encrypted content is stored, not decrypted.** Codex encrypted reasoning is stored as `Content::Thinking { encrypted: true, text: "" }`. We never attempt decryption.
5. **Tauri commands are explicitly ACL-whitelisted.** Every new command must be added to both `permissions/default.toml` and `capabilities/default.json`.

---

## Guidelines for Common Tasks

### Adding a New Source Parser

1. Create `src-tauri/src/parser/<name>.rs`.
2. Implement `parse(contents: &str) -> Result<Trajectory, ParseError>`.
3. Add detection heuristics to `parser/detector.rs`. Prefer field-name fingerprints over regex.
4. Wire the new variant into `parser/mod.rs`'s `parse_file()` match.
5. Add a sample file to `test/samples/<name>/`.
6. Add the sample path to the integration test in `parser/mod.rs`.
7. Ensure the parser populates `parent_id` and produces edges so the DAG validates.

### Adding a New `Content` Variant

1. Add the variant to `src-tauri/src/ir/content.rs` with `Serialize`/`Deserialize`.
2. Mirror it in `src/types/ir.ts`.
3. Update `NodeDetail.svelte` to render the new variant in both Raw and Markdown modes.
4. Update `BricksRenderer.svelte` preview logic to show a compact label for the new variant.
5. Add a unit test for serde round-trip in `content.rs`.

### Adding a New Node Kind / Color

1. Append the kind and color schema to `KIND_COLORS` in `src/lib/colors.ts`.
2. If the kind originates from a new source parser, map the raw source type to the canonical kind string in the parser — do not create source-specific kinds.
3. No other files need changing; renderers fall back to `DEFAULT_COLOR` automatically.

### Adding a New Renderer Theme

1. Create `src/components/<Name>Renderer.svelte`.
2. Accept props: `trajectory: Trajectory`, `onSelect: (node: Node) => void`, `selectedNode: Node | null`.
3. Use the same `topoSort()` ordering that `DotsRenderer` and `BricksRenderer` use.
4. Wire the theme toggle into `App.svelte`.
5. Respect `node.is_sidechain` (visually de-emphasize sidechain nodes).

---

## Philosophy & Trade-Offs

**Why Rust backend + Svelte frontend?**
Rust gives us strong correctness guarantees for parsing and DAG validation. Svelte 5 gives us reactive UI with minimal boilerplate. Tauri bridges them with a small native binary instead of bundling a full browser engine.

**Why is `kind` a free-form `String`?**
Closed enums force a central edit for every new concept. A free-form string lets parsers evolve independently. The cost — no exhaustiveness checking — is offset by the color map and renderer fallbacks.

**Why no general graph-layout engine?**
Agent trajectories are predominantly linear with occasional branching (parallel tool calls). A full graph-layout engine adds complexity and bundle size for a problem that is 90% vertical stacking. If we later need complex branching layouts, we can revisit this.

**Why two independent `topoSort` implementations?**
One in Rust (`Trajectory::topological_order()`) for backend tests and potential server use; one in TypeScript for frontend rendering. They must produce identical orderings. We accept this duplication to keep the frontend decoupled from Rust calls for sorting.
