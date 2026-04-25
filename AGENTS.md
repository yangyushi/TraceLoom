# AGENTS.md — LLM Tracer

## Purpose

LLM Tracer is a cross-platform desktop app that visualises LLM agent message histories as DAGs. It reads `.jsonl` files from diverse sources, parses them into a single universal intermediate representation (IR), and renders them in multiple visual themes.

## Core Principles

- Source-Agnostic IR:
    - All input formats collapse into one `Trajectory` structure.
    - The IR must not contain any source-format-specific fields.
    - If a new source is added, only the parser changes — never the IR.
- Heuristic Over Configuration:
    - to detect source formats by inspecting the first JSON lines and looking for tell-tale fields.
    - There is no manifest file or user-selected format.
    - If detection fails, we return `UnknownFormat` — never guess silently.
- Temporal Causality is Law
    - A parent node must never have a timestamp after its child.
    - This is a hard invariant enforced by `Trajectory::validate()`.
    - Any parser that produces backward-time edges is buggy.
- Fail-Soft, Not Fail-Silent
    - If an edge points to a missing node, the graph still renders.
    - The broken linkage is collected into `Trajectory.orphans` with a warning.
    - We never drop data silently.
- Extensibility by Addition, Not Modification:
    - Adding a new node kind, color, parser, or renderer should require creating new entries/maps — not refactoring existing code paths.
- Two Renderers, One Ordering
    - Both visual themes (`dots` and `bricks`) consume the same topological ordering.
    - The renderers are views over the same graph data structure.

## Invariants (Never Violate)

- Every `Trajectory` returned by `parse_file()` shuold pass `validate()`, which checks for duplicate IDs, cycles, and temporal violations.
- The color schema in `src/lib/colors.ts` is the single source of truth.
- When two nodes have equal graph depth, the earlier timestamp comes first.
- Encrypted content is stored, not decrypted. Codex encrypted reasoning is stored as `Content::Thinking { encrypted: true, text: "" }`. We never attempt decryption.
- Tauri commands are explicitly ACL-whitelisted. Every new command must be added to both `permissions/default.toml` and `capabilities/default.json`.
