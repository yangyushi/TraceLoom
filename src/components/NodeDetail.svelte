<script lang="ts">
  import { marked } from "marked";
  import Prism from "prismjs";
  import "prismjs/components/prism-json";
  import type { Node } from "../types/ir";

  interface Props {
    node: Node;
    onClose: () => void;
  }

  let { node, onClose }: Props = $props();
  let showMarkdown = $state(false);

  function getRawText(): string {
    const c = node.content;
    switch (c.type) {
      case "Text":
        return c.data;
      case "Thinking":
        return c.data.text;
      case "ToolUse":
        return `${c.data.name}\n${JSON.stringify(c.data.input, null, 2)}`;
      case "ToolResult":
        return c.data.output;
      case "Snapshot":
        return c.data.description;
      case "Custom":
        return JSON.stringify(c.data.payload, null, 2);
      default:
        return "";
    }
  }

  /**
   * Try to extract a JSON object or array from anywhere inside the text.
   * Handles cases where JSON is embedded after a prefix (e.g. ToolUse names).
   */
  function findJsonInText(text: string): string | null {
    // Look for the first '{' or '[' that could start JSON
    let braceIdx = text.indexOf("{");
    let bracketIdx = text.indexOf("[");
    let start = -1;
    if (braceIdx >= 0 && bracketIdx >= 0) {
      start = Math.min(braceIdx, bracketIdx);
    } else if (braceIdx >= 0) {
      start = braceIdx;
    } else if (bracketIdx >= 0) {
      start = bracketIdx;
    }
    if (start < 0) return null;

    // Try expanding the substring until it parses as valid JSON
    for (let end = start + 2; end <= text.length; end++) {
      const candidate = text.slice(start, end);
      try {
        JSON.parse(candidate);
        return candidate;
      } catch {
        // continue expanding
      }
    }
    return null;
  }

  function highlightJson(text: string): string {
    // First try: the whole text is JSON
    try {
      JSON.parse(text);
      return Prism.highlight(text, Prism.languages.json, "json");
    } catch {
      // Second try: JSON is embedded somewhere in the text
      const jsonBlock = findJsonInText(text);
      if (jsonBlock) {
        const before = escapeHtml(text.slice(0, text.indexOf(jsonBlock)));
        const highlighted = Prism.highlight(jsonBlock, Prism.languages.json, "json");
        const after = escapeHtml(text.slice(text.indexOf(jsonBlock) + jsonBlock.length));
        return `${before}${highlighted}${after}`;
      }
    }
    return escapeHtml(text);
  }

  function escapeHtml(text: string): string {
    return text
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
  }

  interface Section {
    key: string;
    value: string;
    isMarkdown: boolean;
  }

  function parseSections(raw: string): Section[] {
    // First try: the whole text is a JSON object with top-level keys
    try {
      const parsed = JSON.parse(raw);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        const sections: Section[] = [];
        for (const [key, val] of Object.entries(parsed)) {
          const value =
            typeof val === "string" ? val : JSON.stringify(val, null, 2);
          sections.push({ key, value, isMarkdown: typeof val === "string" });
        }
        return sections;
      }
    } catch {
      // Not pure JSON object
    }

    // Second try: JSON is embedded in the text
    const jsonBlock = findJsonInText(raw);
    if (jsonBlock) {
      try {
        const parsed = JSON.parse(jsonBlock);
        if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
          const sections: Section[] = [];
          for (const [key, val] of Object.entries(parsed)) {
            const value =
              typeof val === "string" ? val : JSON.stringify(val, null, 2);
            sections.push({ key, value, isMarkdown: typeof val === "string" });
          }
          return sections;
        }
      } catch {
        // Embedded JSON is not an object
      }
    }

    return [{ key: "Content", value: raw, isMarkdown: true }];
  }

  function renderMarkdown(value: string): string {
    try {
      return marked.parse(value, { async: false }) as string;
    } catch {
      return value;
    }
  }

  const sections = $derived(parseSections(getRawText()));

  function formatTimestamp(ts: string | null): string {
    if (!ts) return "N/A";
    return new Date(ts).toLocaleString();
  }
</script>

<div class="detail-panel">
  <div class="detail-header">
    <h3>{node.kind}</h3>
    <button class="close-btn" onclick={onClose}>&times;</button>
  </div>

  <div class="detail-meta">
    <div><strong>ID:</strong> {node.id}</div>
    <div><strong>Role:</strong> {node.role[0]}</div>
    {#if node.parent_id}
      <div><strong>Parent:</strong> {node.parent_id}</div>
    {/if}
    <div><strong>Time:</strong> {formatTimestamp(node.timestamp)}</div>
    {#if node.is_sidechain}
      <div class="badge">sidechain</div>
    {/if}
  </div>

  <div class="detail-content">
    <div class="content-toolbar">
      <button class:active={!showMarkdown} onclick={() => (showMarkdown = false)}>Raw</button>
      <button class:active={showMarkdown} onclick={() => (showMarkdown = true)}>Markdown</button>
    </div>

    {#if showMarkdown}
      <div class="markdown-sections">
        {#each sections as section}
          <details class="markdown-section" open={false}>
            <summary class="section-title">{section.key}</summary>
            <div class="markdown-body">
              {#if section.isMarkdown}
                {@html renderMarkdown(section.value)}
              {:else}
                <pre class="json-block">{@html highlightJson(section.value)}</pre>
              {/if}
            </div>
          </details>
        {/each}
      </div>
    {:else}
      <pre class="raw-body">{@html highlightJson(getRawText())}</pre>
    {/if}
  </div>

  {#if Object.keys(node.metadata).length > 0}
    <details class="metadata">
      <summary>Metadata</summary>
      <pre>{@html highlightJson(JSON.stringify(node.metadata, null, 2))}</pre>
    </details>
  {/if}
</div>

<style>
  .detail-panel {
    padding: 16px;
  }

  .detail-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }

  .detail-header h3 {
    margin: 0;
    font-size: 16px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: #343a40;
  }

  .close-btn {
    background: none;
    border: none;
    color: #868e96;
    font-size: 22px;
    cursor: pointer;
    line-height: 1;
  }

  .close-btn:hover {
    color: #212529;
  }

  .detail-meta {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: #868e96;
    margin-bottom: 16px;
    padding-bottom: 12px;
    border-bottom: 1px solid #e9ecef;
  }

  .badge {
    display: inline-block;
    background: #e9ecef;
    color: #495057;
    padding: 2px 6px;
    border-radius: 3px;
    font-size: 10px;
    text-transform: uppercase;
    width: fit-content;
    margin-top: 4px;
  }

  .content-toolbar {
    display: flex;
    gap: 4px;
    margin-bottom: 8px;
  }

  .content-toolbar button {
    background: #f1f3f5;
    color: #495057;
    border: 1px solid #dee2e6;
    padding: 4px 10px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 12px;
  }

  .content-toolbar button.active {
    background: #dee2e6;
    color: #212529;
    font-weight: 500;
  }

  .raw-body {
    background: #f8f9fa;
    border: 1px solid #e9ecef;
    border-radius: 6px;
    padding: 12px;
    font-size: 12px;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
    color: #212529;
    max-height: 60vh;
    overflow: auto;
    font-family: ui-monospace, "Cascadia Code", "Source Code Pro", monospace;
  }

  :global(.raw-body .token.property) {
    color: #005cc5;
  }
  :global(.raw-body .token.string) {
    color: #22863a;
  }
  :global(.raw-body .token.number) {
    color: #005cc5;
  }
  :global(.raw-body .token.boolean) {
    color: #d73a49;
  }
  :global(.raw-body .token.null) {
    color: #d73a49;
  }
  :global(.raw-body .token.punctuation) {
    color: #24292e;
  }

  .markdown-sections {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .markdown-section {
    background: #f8f9fa;
    border: 1px solid #e9ecef;
    border-radius: 6px;
    overflow: hidden;
  }

  .section-title {
    padding: 8px 12px;
    font-size: 12px;
    font-weight: 600;
    color: #495057;
    text-transform: uppercase;
    letter-spacing: 0.3px;
    cursor: pointer;
    background: #f1f3f5;
    user-select: none;
  }

  .markdown-body {
    padding: 12px;
    font-size: 13px;
    line-height: 1.6;
    color: #343a40;
    max-height: 50vh;
    overflow: auto;
  }

  .markdown-body :global(p) {
    margin: 0 0 10px;
  }

  .markdown-body :global(code) {
    background: #e9ecef;
    padding: 2px 4px;
    border-radius: 3px;
    font-family: ui-monospace, monospace;
    font-size: 12px;
  }

  .markdown-body :global(pre) {
    background: #f1f3f5;
    padding: 10px;
    border-radius: 4px;
    overflow: auto;
    font-size: 12px;
  }

  .json-block {
    background: #f1f3f5;
    padding: 10px;
    border-radius: 4px;
    overflow: auto;
    font-size: 12px;
    line-height: 1.5;
    margin: 0;
    font-family: ui-monospace, "Cascadia Code", "Source Code Pro", monospace;
  }

  :global(.json-block .token.property) {
    color: #005cc5;
  }
  :global(.json-block .token.string) {
    color: #22863a;
  }
  :global(.json-block .token.number) {
    color: #005cc5;
  }
  :global(.json-block .token.boolean) {
    color: #d73a49;
  }
  :global(.json-block .token.null) {
    color: #d73a49;
  }
  :global(.json-block .token.punctuation) {
    color: #24292e;
  }

  .metadata {
    margin-top: 16px;
    font-size: 12px;
    color: #868e96;
  }

  .metadata summary {
    cursor: pointer;
    color: #495057;
    font-weight: 500;
  }

  .metadata pre {
    background: #f8f9fa;
    border: 1px solid #e9ecef;
    border-radius: 4px;
    padding: 8px;
    overflow: auto;
    max-height: 200px;
    font-size: 11px;
    font-family: ui-monospace, monospace;
  }
</style>
