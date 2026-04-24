/**
 * Centralized color schema for node kinds.
 *
 * Colors are assigned by `node.kind` (e.g. "user", "assistant", "think",
 * "tool_call", "tool_result", "snapshot", "system") rather than by source
 * format, so the same logical kind always renders with the same color
 * regardless of whether it came from Claude, Codex, or OpenClaw.
 *
 * To add a color for a new kind, simply append an entry to the map.
 */

export interface ColorSchema {
  fill: string;
  stroke: string;
  text: string;
  bg: string;
  border: string;
}

const DEFAULT_COLOR: ColorSchema = {
  fill: "#9e9e9e",
  stroke: "#757575",
  text: "#616161",
  bg: "#f5f5f5",
  border: "#e0e0e0",
};

const KIND_COLORS: Record<string, ColorSchema> = {
  user: {
    fill: "#42a5f5",
    stroke: "#1e88e5",
    text: "#1565c0",
    bg: "#e3f2fd",
    border: "#bbdefb",
  },
  assistant: {
    fill: "#66bb6a",
    stroke: "#43a047",
    text: "#2e7d32",
    bg: "#e8f5e9",
    border: "#c8e6c9",
  },
  think: {
    fill: "#ab47bc",
    stroke: "#8e24aa",
    text: "#6a1b9a",
    bg: "#f3e5f5",
    border: "#e1bee7",
  },
  tool_call: {
    fill: "#ffa726",
    stroke: "#fb8c00",
    text: "#ef6c00",
    bg: "#fff3e0",
    border: "#ffe0b2",
  },
  tool_result: {
    fill: "#ff7043",
    stroke: "#f4511e",
    text: "#d84315",
    bg: "#fbe9e7",
    border: "#ffccbc",
  },
  snapshot: {
    fill: "#26c6da",
    stroke: "#00acc1",
    text: "#00838f",
    bg: "#e0f7fa",
    border: "#b2ebf2",
  },
  system: {
    fill: "#bdbdbd",
    stroke: "#9e9e9e",
    text: "#616161",
    bg: "#f5f5f5",
    border: "#e0e0e0",
  },
};

/**
 * Look up the color schema for a given node kind.
 * Falls back to DEFAULT_COLOR for unknown kinds.
 */
export function getColorSchema(kind: string): ColorSchema {
  const normalized = kind.toLowerCase().trim();
  return KIND_COLORS[normalized] ?? DEFAULT_COLOR;
}

/**
 * Get just the fill color (for dots / circles).
 */
export function getFillColor(kind: string): string {
  return getColorSchema(kind).fill;
}

/**
 * Get just the stroke/border-left color (for bricks).
 */
export function getStrokeColor(kind: string): string {
  return getColorSchema(kind).stroke;
}
