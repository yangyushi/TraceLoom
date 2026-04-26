<script lang="ts">
  import type { Message, Block } from "../types/ir";
  import { getBlockRawText } from "../lib/blockPreview";

  interface MessageItem {
    type: "message";
    message: Message;
  }

  interface BlockItem {
    type: "block";
    message: Message;
    block: Block;
  }

  interface Props {
    item: MessageItem | BlockItem | null;
    onClose: () => void;
  }

  let { item, onClose }: Props = $props();
  let showMarkdown = $state(false);
  let showItems = $state(false);
  let rendererVersion = $state(0);
  let markedParser: typeof import("marked").marked | null = null;
  let prism: typeof import("prismjs") | null = null;
  let loadingMarked = false;
  let loadingPrism = false;

  const MAX_JSON_EXTRACTION_BYTES = 200_000;

  function ensureMarked() {
    if (markedParser || loadingMarked) return;
    loadingMarked = true;
    void import("marked").then((module) => {
      markedParser = module.marked;
      loadingMarked = false;
      rendererVersion += 1;
    });
  }

  function ensurePrism() {
    if (prism || loadingPrism) return;
    loadingPrism = true;
    void import("prismjs").then(async (module) => {
      await import("prismjs/components/prism-json");
      prism = module;
      loadingPrism = false;
      rendererVersion += 1;
    });
  }

  /**
   * Try to extract a JSON object or array from anywhere inside the text.
   */
  function findJsonInText(text: string): string | null {
    if (text.length > MAX_JSON_EXTRACTION_BYTES) return null;

    for (let start = 0; start < text.length; start++) {
      const opener = text[start];
      if (opener !== "{" && opener !== "[") continue;
      const closer = opener === "{" ? "}" : "]";
      const stack = [closer];
      let inString = false;
      let escaped = false;

      for (let idx = start + 1; idx < text.length; idx++) {
        const ch = text[idx];
        if (escaped) {
          escaped = false;
          continue;
        }
        if (ch === "\\") {
          escaped = inString;
          continue;
        }
        if (ch === "\"") {
          inString = !inString;
          continue;
        }
        if (inString) continue;
        if (ch === "{" || ch === "[") {
          stack.push(ch === "{" ? "}" : "]");
          continue;
        }
        if (ch === "}" || ch === "]") {
          if (stack.pop() !== ch) break;
          if (stack.length === 0 && ch === closer) {
            const candidate = text.slice(start, idx + 1);
            try {
              JSON.parse(candidate);
              return candidate;
            } catch {
              break;
            }
          }
        }
      }
    }
    return null;
  }

  function highlightJson(text: string): string {
    rendererVersion;
    ensurePrism();
    try {
      JSON.parse(text);
      if (prism?.languages.json) {
        return sanitizeHtml(prism.highlight(text, prism.languages.json, "json"));
      }
      return escapeHtml(text);
    } catch {
      const jsonBlock = findJsonInText(text);
      if (jsonBlock) {
        const before = escapeHtml(text.slice(0, text.indexOf(jsonBlock)));
        const highlighted = prism?.languages.json
          ? sanitizeHtml(prism.highlight(jsonBlock, prism.languages.json, "json"))
          : escapeHtml(jsonBlock);
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

  function sanitizeHtml(html: string): string {
    const allowedTags = new Set([
      "A",
      "BLOCKQUOTE",
      "BR",
      "CODE",
      "DEL",
      "EM",
      "H1",
      "H2",
      "H3",
      "H4",
      "H5",
      "H6",
      "HR",
      "LI",
      "OL",
      "P",
      "PRE",
      "SPAN",
      "STRONG",
      "TABLE",
      "TBODY",
      "TD",
      "TH",
      "THEAD",
      "TR",
      "UL",
    ]);
    const template = document.createElement("template");
    template.innerHTML = html;

    const walk = (node: Node) => {
      for (const child of Array.from(node.childNodes)) {
        if (child.nodeType === Node.ELEMENT_NODE) {
          const element = child as HTMLElement;
          if (!allowedTags.has(element.tagName)) {
            element.replaceWith(document.createTextNode(element.textContent ?? ""));
            continue;
          }
          for (const attr of Array.from(element.attributes)) {
            const name = attr.name.toLowerCase();
            const value = attr.value;
            const isSafeHref =
              element.tagName === "A"
              && name === "href"
              && /^(https?:|mailto:|#)/.test(value);
            const isSafeClass =
              (element.tagName === "SPAN" || element.tagName === "CODE")
              && name === "class"
              && /^[a-z0-9_\- ]+$/i.test(value);
            if (!isSafeHref && !isSafeClass) {
              element.removeAttribute(attr.name);
            }
          }
        }
        walk(child);
      }
    };

    walk(template.content);
    return template.innerHTML;
  }

  interface Section {
    key: string;
    value: string;
    isMarkdown: boolean;
  }

  function parseSections(raw: string): Section[] {
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
    rendererVersion;
    ensureMarked();
    try {
      if (!markedParser) return escapeHtml(value);
      return sanitizeHtml(markedParser.parse(escapeHtml(value), { async: false }) as string);
    } catch {
      return escapeHtml(value);
    }
  }

  function formatTimestamp(ts: string | null): string {
    if (!ts) return "N/A";
    return new Date(ts).toLocaleString();
  }

  function prettyPrintJson(raw: string): string {
    try {
      const parsed = JSON.parse(raw);
      return JSON.stringify(parsed, null, 2);
    } catch {
      return raw;
    }
  }

  interface KvItem {
    key: string;
    value: string;
    isJson: boolean;
  }

  function parseObjectItems(raw: string): KvItem[] {
    try {
      const parsed = JSON.parse(raw);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        const items: KvItem[] = [];
        for (const [key, val] of Object.entries(parsed)) {
          if (typeof val === "string") {
            items.push({ key, value: val, isJson: false });
          } else {
            items.push({ key, value: JSON.stringify(val, null, 2), isJson: true });
          }
        }
        return items;
      }
    } catch {
      // fall through
    }
    return [{ key: "Content", value: raw, isJson: false }];
  }
</script>

<div class="detail-panel">
  {#if item && item.type === "message"}
    {@const msg = item.message}
    <div class="detail-header">
      <h3>{msg.role}</h3>
      <button class="close-btn" onclick={onClose}>&times;</button>
    </div>

    <div class="detail-meta">
      <div><strong>ID:</strong> {msg.id}</div>
      <div><strong>Role:</strong> {msg.role}</div>
      {#if msg.parent_id}
        <div><strong>Parent:</strong> {msg.parent_id}</div>
      {/if}
      <div><strong>Time:</strong> {formatTimestamp(msg.timestamp)}</div>
      <div><strong>Blocks:</strong> {msg.blocks.length}</div>
      {#if msg.is_sidechain}
        <div class="badge">sidechain</div>
      {/if}
    </div>

    {#if msg.raw_json}
      <div class="detail-content">
        <div class="content-toolbar">
          <button class:active={!showItems} onclick={() => (showItems = false)}>json</button>
          <button class:active={showItems} onclick={() => (showItems = true)}>items</button>
        </div>
        {#if showItems}
          <div class="item-sections">
            {#each parseObjectItems(msg.raw_json) as item}
              <details class="item-section" open>
                <summary class="item-title">{item.key}</summary>
                <div class="item-body">
                  {#if item.isJson}
                    <pre class="json-block">{@html highlightJson(item.value)}</pre>
                  {:else}
                    <div class="markdown-body">{@html renderMarkdown(item.value)}</div>
                  {/if}
                </div>
              </details>
            {/each}
          </div>
        {:else}
          <pre class="raw-body">{@html highlightJson(prettyPrintJson(msg.raw_json))}</pre>
        {/if}
      </div>
    {/if}
  {:else if item && item.type === "block"}
    {@const block = item.block}
    {@const msg = item.message}
    <div class="detail-header">
      <h3>{block.kind}</h3>
      <button class="close-btn" onclick={onClose}>&times;</button>
    </div>

    <div class="detail-meta">
      <div><strong>ID:</strong> {block.id}</div>
      <div><strong>Kind:</strong> {block.kind}</div>
      <div><strong>Message:</strong> {msg.role} ({msg.id})</div>
      {#if block.tool_call_id}
        <div><strong>Tool Call ID:</strong> {block.tool_call_id}</div>
      {/if}
    </div>

    <div class="detail-content">
      <div class="content-toolbar">
        <button class:active={!showMarkdown} onclick={() => (showMarkdown = false)}>Raw</button>
        <button class:active={showMarkdown} onclick={() => (showMarkdown = true)}>Markdown</button>
      </div>

      {#if showMarkdown}
        <div class="markdown-sections">
          {#each parseSections(getBlockRawText(block)) as section}
            <details class="markdown-section" open>
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
        <pre class="raw-body">{@html highlightJson(getBlockRawText(block))}</pre>
      {/if}
    </div>
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

  .item-sections {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .item-section {
    background: #f8f9fa;
    border: 1px solid #e9ecef;
    border-radius: 6px;
    overflow: hidden;
  }

  .item-title {
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

  .item-body {
    padding: 12px;
    font-size: 13px;
    line-height: 1.6;
    color: #343a40;
    max-height: 50vh;
    overflow: auto;
  }

  .item-body :global(p) {
    margin: 0 0 10px;
  }

  .item-body :global(code) {
    background: #e9ecef;
    padding: 2px 4px;
    border-radius: 3px;
    font-family: ui-monospace, monospace;
    font-size: 12px;
  }

  .item-body :global(pre) {
    background: #f1f3f5;
    padding: 10px;
    border-radius: 4px;
    overflow: auto;
    font-size: 12px;
  }
</style>
