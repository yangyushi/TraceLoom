# Frontend Reference

## Component Structure

```
App.svelte
├── toolbar (Open Folder, Open File, theme toggle)
├── error banner
└── workspace
    ├── file-list sidebar (resizable)
    ├── resize handle
    ├── canvas
    │   ├── DotsRenderer  (theme === "dots")
    │   └── BricksRenderer (theme === "bricks")
    ├── resize handle
    └── inspector (NodeDetail)
```

## State Management

All state lives in `App.svelte` as Svelte 5 runes:

```typescript
let trajectory = $state<Trajectory | null>(null);
let selectedNode = $state<Node | null>(null);
let theme = $state<"dots" | "bricks">("dots");
let files = $state<string[]>([]);
let currentFile = $state<string | null>(null);
let folderPath = $state<string | null>(null);
let leftWidth = $state(220);
let rightWidth = $state(420);
```

Props are drilled down to renderers. There is no global store.

## Renderer Contract

Both renderers accept the same props:

```typescript
interface Props {
  trajectory: Trajectory;
  onSelect: (node: Node) => void;
  selectedNode: Node | null;
}
```

They both call an internal `topoSort()` that must produce identical ordering to `Trajectory::topological_order()` in Rust.

### DotsRenderer

- SVG-based.
- One `<circle>` per node, arranged vertically.
- A faint connecting line runs down the center.
- Labels shown as SVG `<text>` to the right of each dot.
- Selected node gets a dark stroke.
- Sidechain nodes are faded (`opacity: 0.5`) with a dashed stroke.

### BricksRenderer

- HTML/CSS-based.
- One `div.brick` per node, stacked vertically in a flex column.
- Brick header shows `kind`, `role` initial, and timestamp.
- Brick preview shows a compact snippet based on `content.type`:
  - `Text` → first 120 chars
  - `ToolUse` → tool name
  - `ToolResult` → first 120 chars of output
  - `Thinking` → `[encrypted reasoning]` or first 120 chars
  - default → `[type]`
- Selected brick gets a blue background and border shadow.
- Sidechain bricks are faded.

## NodeDetail Inspector

### Raw Mode
Displays `getRawText()` with JSON syntax highlighting via PrismJS. If the raw text contains an embedded JSON block (e.g., `ToolUse` prefixed with a name), `findJsonInText()` extracts the JSON and highlights only that portion.

### Markdown Mode
Parses the raw text into sections:
1. If the text is a pure JSON object, each top-level key becomes a collapsible section.
2. If JSON is embedded inside text, the same extraction applies.
3. Otherwise, the entire text is treated as one `Content` section.

Sections are rendered as `<details>` elements. String values are passed through `marked.parse()`. Non-string values are JSON-highlighted.

### Metadata
Displayed as a collapsible `<details>` block with full JSON highlighting.

## Color Theming

Colors are defined in `src/lib/colors.ts` as a `Record<string, ColorSchema>`.

```typescript
interface ColorSchema {
  fill: string;   // dots circle fill
  stroke: string; // dots selected stroke / bricks left border
  text: string;
  bg: string;
  border: string;
}
```

Lookup is case-insensitive: `kind.toLowerCase().trim()`. Unknown kinds fall back to `DEFAULT_COLOR`.

## Resizable Panels

Mouse-down on a `.resize-handle` sets a flag and attaches `mousemove`/`mouseup` listeners to `window`. `leftWidth` is clamped `[140, 400]`; `rightWidth` is computed from `appWidth - clientX` and clamped `[280, 600]`.

The file-list sidebar must have `flex-shrink: 0` on its container and on each `.file-item` to prevent flexbox collapse when overflowing.
