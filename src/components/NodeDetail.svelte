<script lang="ts">
  import type { Message, Block } from "../types/ir";
  import type { Bookmark } from "../types/workspace";
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
    nodeId: string | null;
    bookmarks: Bookmark[];
    onAddBookmark: (comment: string) => void;
    onRemoveBookmark: (id: number) => void;
  }

  let { item, onClose, nodeId, bookmarks, onAddBookmark, onRemoveBookmark }: Props = $props();

  let showBookmarkForm = $state(false);
  let bookmarkComment = $state("");

  const existingBookmark = $derived.by(() => {
    if (!nodeId) return null;
    return bookmarks.find((b) => b.node_id === nodeId) ?? null;
  });

  function handleAddBookmark() {
    onAddBookmark(bookmarkComment);
    bookmarkComment = "";
    showBookmarkForm = false;
  }
  let showMarkdown = $state(false);
  let showItems = $state(false);
  let rendererVersion = $state(0);
  let markedParser: typeof import("marked").marked | null = null;
  let prism: typeof import("prismjs") | null = null;
  let loadingMarked = false;
  let loadingPrism = false;

  const MAX_CACHE_ENTRIES = 24;
  const MAX_HIGHLIGHT_BYTES = 100_000;
  const MAX_MARKDOWN_BYTES = 160_000;
  const MAX_JSON_EXTRACTION_BYTES = 200_000;
  const highlightCache = new Map<string, string>();
  const markdownCache = new Map<string, string>();
  const sectionCache = new Map<string, Section[]>();
  const objectItemsCache = new Map<string, KvItem[]>();

  const blockRawText = $derived.by(() =>
    item?.type === "block" ? getBlockRawText(item.block) : ""
  );

  const messageRawJson = $derived.by(() =>
    item?.type === "message" ? item.message.raw_json : null
  );

  const prettyMessageJson = $derived.by(() =>
    messageRawJson ? prettyPrintJson(messageRawJson) : ""
  );

  const highlightedMessageJson = $derived.by(() =>
    prettyMessageJson ? highlightJson(prettyMessageJson) : ""
  );

  const highlightedBlockRaw = $derived.by(() =>
    item?.type === "block" ? highlightJson(blockRawText) : ""
  );

  const blockSections = $derived.by(() =>
    item?.type === "block" ? parseSections(blockRawText) : []
  );

  const messageObjectItems = $derived.by(() =>
    messageRawJson ? parseObjectItems(messageRawJson) : []
  );

  function remember<T>(cache: Map<string, T>, key: string, value: T): T {
    cache.set(key, value);
    if (cache.size > MAX_CACHE_ENTRIES) {
      const oldest = cache.keys().next().value;
      if (oldest !== undefined) cache.delete(oldest);
    }
    return value;
  }

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
    if (text.length > MAX_HIGHLIGHT_BYTES) {
      return escapeHtml(text);
    }

    const prismReady = Boolean(prism?.languages.json);
    const cacheKey = `${prismReady ? "prism" : "plain"}:${text}`;
    const cached = highlightCache.get(cacheKey);
    if (cached) return cached;

    ensurePrism();
    try {
      JSON.parse(text);
      if (prismReady && prism?.languages.json) {
        return remember(
          highlightCache,
          cacheKey,
          sanitizeHtml(prism.highlight(text, prism.languages.json, "json"))
        );
      }
      return remember(highlightCache, cacheKey, escapeHtml(text));
    } catch {
      const jsonBlock = findJsonInText(text);
      if (jsonBlock) {
        const before = escapeHtml(text.slice(0, text.indexOf(jsonBlock)));
        const highlighted = prismReady && prism?.languages.json
          ? sanitizeHtml(prism.highlight(jsonBlock, prism.languages.json, "json"))
          : escapeHtml(jsonBlock);
        const after = escapeHtml(text.slice(text.indexOf(jsonBlock) + jsonBlock.length));
        return remember(highlightCache, cacheKey, `${before}${highlighted}${after}`);
      }
    }
    return remember(highlightCache, cacheKey, escapeHtml(text));
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
    const cached = sectionCache.get(raw);
    if (cached) return cached;

    try {
      const parsed = JSON.parse(raw);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        const sections: Section[] = [];
        for (const [key, val] of Object.entries(parsed)) {
          const value =
            typeof val === "string" ? val : JSON.stringify(val, null, 2);
          sections.push({ key, value, isMarkdown: typeof val === "string" });
        }
        return remember(sectionCache, raw, sections);
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
          return remember(sectionCache, raw, sections);
        }
      } catch {
        // Embedded JSON is not an object
      }
    }

    return remember(sectionCache, raw, [{ key: "Content", value: raw, isMarkdown: true }]);
  }

  function renderMarkdown(value: string): string {
    rendererVersion;
    if (value.length > MAX_MARKDOWN_BYTES) {
      return escapeHtml(value);
    }

    const markedReady = Boolean(markedParser);
    const cacheKey = `${markedReady ? "marked" : "plain"}:${value}`;
    const cached = markdownCache.get(cacheKey);
    if (cached) return cached;

    ensureMarked();
    try {
      if (!markedParser) return remember(markdownCache, cacheKey, escapeHtml(value));
      return remember(
        markdownCache,
        cacheKey,
        sanitizeHtml(markedParser.parse(escapeHtml(value), { async: false }) as string)
      );
    } catch {
      return remember(markdownCache, cacheKey, escapeHtml(value));
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
    const cached = objectItemsCache.get(raw);
    if (cached) return cached;

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
        return remember(objectItemsCache, raw, items);
      }
    } catch {
      // fall through
    }
    return remember(objectItemsCache, raw, [{ key: "Content", value: raw, isJson: false }]);
  }
</script>

<div class="detail-panel">
  {#if item && item.type === "message"}
    {@const msg = item.message}
    <div class="detail-header">
      <h3>{msg.role}</h3>
      <div class="header-actions">
        {#if existingBookmark}
          <button class="bookmark-btn active" onclick={() => onRemoveBookmark(existingBookmark.id)} title="Remove bookmark">
            ★
          </button>
        {:else}
          <button class="bookmark-btn" onclick={() => showBookmarkForm = !showBookmarkForm} title="Add bookmark">
            ☆
          </button>
        {/if}
        <button class="close-btn" onclick={onClose}>&times;</button>
      </div>
    </div>

    {#if showBookmarkForm}
      <div class="bookmark-form">
        <textarea
          placeholder="Add a comment (optional)..."
          bind:value={bookmarkComment}
          rows={2}
        ></textarea>
        <div class="bookmark-form-actions">
          <button onclick={handleAddBookmark}>Add Bookmark</button>
          <button onclick={() => { showBookmarkForm = false; bookmarkComment = ""; }}>Cancel</button>
        </div>
      </div>
    {/if}

    {#if existingBookmark?.comment}
      <div class="bookmark-comment">
        <strong>Bookmark:</strong> {existingBookmark.comment}
      </div>
    {/if}

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
            {#each messageObjectItems as item}
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
          <pre class="raw-body">{@html highlightedMessageJson}</pre>
        {/if}
      </div>
    {/if}
  {:else if item && item.type === "block"}
    {@const block = item.block}
    {@const msg = item.message}
    <div class="detail-header">
      <h3>{block.kind}</h3>
      <div class="header-actions">
        {#if existingBookmark}
          <button class="bookmark-btn active" onclick={() => onRemoveBookmark(existingBookmark.id)} title="Remove bookmark">
            ★
          </button>
        {:else}
          <button class="bookmark-btn" onclick={() => showBookmarkForm = !showBookmarkForm} title="Add bookmark">
            ☆
          </button>
        {/if}
        <button class="close-btn" onclick={onClose}>&times;</button>
      </div>
    </div>

    {#if showBookmarkForm}
      <div class="bookmark-form">
        <textarea
          placeholder="Add a comment (optional)..."
          bind:value={bookmarkComment}
          rows={2}
        ></textarea>
        <div class="bookmark-form-actions">
          <button onclick={handleAddBookmark}>Add Bookmark</button>
          <button onclick={() => { showBookmarkForm = false; bookmarkComment = ""; }}>Cancel</button>
        </div>
      </div>
    {/if}

    {#if existingBookmark?.comment}
      <div class="bookmark-comment">
        <strong>Bookmark:</strong> {existingBookmark.comment}
      </div>
    {/if}

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
          {#each blockSections as section}
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
        <pre class="raw-body">{@html highlightedBlockRaw}</pre>
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

  .header-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .bookmark-btn {
    background: none;
    border: none;
    font-size: 18px;
    cursor: pointer;
    color: #adb5bd;
    line-height: 1;
    padding: 0;
  }

  .bookmark-btn:hover {
    color: #f59f00;
  }

  .bookmark-btn.active {
    color: #f59f00;
  }

  .bookmark-form {
    margin-bottom: 12px;
    padding: 8px;
    background: #fff9db;
    border: 1px solid #ffe066;
    border-radius: 6px;
  }

  .bookmark-form textarea {
    width: 100%;
    border: 1px solid #dee2e6;
    border-radius: 4px;
    padding: 6px;
    font-size: 12px;
    resize: vertical;
  }

  .bookmark-form-actions {
    display: flex;
    gap: 6px;
    margin-top: 6px;
  }

  .bookmark-form-actions button {
    background: #f1f3f5;
    border: 1px solid #ced4da;
    border-radius: 4px;
    padding: 4px 10px;
    cursor: pointer;
    font-size: 12px;
  }

  .bookmark-form-actions button:first-child {
    background: #1864ab;
    color: #fff;
    border-color: #1864ab;
  }

  .bookmark-comment {
    font-size: 12px;
    color: #5f3f00;
    background: #fff3bf;
    padding: 8px 10px;
    border-radius: 4px;
    margin-bottom: 12px;
    border: 1px solid #ffe066;
  }
</style>
