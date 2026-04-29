<script lang="ts">
  import type { TrajectorySource, RenderId } from "../types/workspace";
  import { namespaceId } from "../lib/workspace";
  import { getStrokeColor, getSourceColor } from "../lib/colors";
  import { getBlockPreview } from "../lib/blockPreview";
  import { topologicalMessages } from "../lib/order";

  interface Props {
    source: TrajectorySource;
    onSelect: (renderId: RenderId) => void;
    selectedRenderId: RenderId | null;
    onQuickAddBookmark: (renderId: RenderId) => void;
  }

  let { source, onSelect, selectedRenderId, onQuickAddBookmark }: Props = $props();

  let container: HTMLDivElement | null = $state(null);
  let nodeContextMenu = $state<{ x: number; y: number; renderId: string } | null>(null);

  interface BrickBlock {
    id: string;
    kind: string;
    renderId: string;
    borderColor: string;
    preview: string;
  }

  interface BrickRow {
    id: string;
    role: string;
    renderId: string;
    timestamp: string;
    blockCount: number;
    isSidechain: boolean;
    borderColor: string;
    blocks: BrickBlock[];
  }

  const rows = $derived.by((): BrickRow[] => {
    const trajectory = source.trajectory;
    if (!trajectory) return [];

    return topologicalMessages(trajectory.messages).map((msg) => ({
      id: msg.id,
      role: msg.role,
      renderId: namespaceId(source.id, msg.id),
      timestamp: formatTimestamp(msg.timestamp),
      blockCount: msg.blocks.length,
      isSidechain: msg.is_sidechain,
      borderColor: getStrokeColor(msg.role),
      blocks: msg.blocks.map((block) => ({
        id: block.id,
        kind: block.kind,
        renderId: namespaceId(source.id, block.id),
        borderColor: getStrokeColor(block.kind),
        preview: getBlockPreview(block),
      })),
    }));
  });

  const navigationIds = $derived.by(() =>
    rows.flatMap((row) => [row.renderId, ...row.blocks.map((block) => block.renderId)])
  );

  function handleContextMenu(e: MouseEvent, renderId: string) {
    e.preventDefault();
    nodeContextMenu = { x: e.clientX, y: e.clientY, renderId };
  }

  function closeContextMenu() {
    nodeContextMenu = null;
  }

  function formatTimestamp(ts: string | null): string {
    if (!ts) return "";
    return new Date(ts).toLocaleTimeString();
  }

  function getNavigationTarget(key: string): string | null {
    if (!selectedRenderId || key === "ArrowLeft" || key === "ArrowRight") return null;

    const ids = navigationIds;
    const idx = ids.indexOf(selectedRenderId);
    if (idx < 0) return null;

    if (key === "ArrowUp") return ids[idx - 1] ?? null;
    if (key === "ArrowDown") return ids[idx + 1] ?? null;
    return null;
  }

  function isEditableTarget(target: HTMLElement): boolean {
    return target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable;
  }

  function isBricksKeyTarget(target: HTMLElement): boolean {
    if (isEditableTarget(target)) return false;
    if (target === document.body || target === document.documentElement) return true;
    return container?.contains(target) ?? false;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!["ArrowUp", "ArrowDown"].includes(e.key)) return;
    const target = e.target as HTMLElement;
    if (!isBricksKeyTarget(target)) return;
    const nextId = getNavigationTarget(e.key);
    if (nextId) {
      e.preventDefault();
      onSelect(nextId);
      requestAnimationFrame(() => {
        const el = document.querySelector(`[data-render-id="${nextId}"]`);
        if (el) {
          el.scrollIntoView({ block: "center", behavior: "smooth" });
        }
      });
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="bricks-container" bind:this={container}>
  {#if source.trajectory}
    <div class="source-section">
      <div class="source-header" style="border-left-color: {getSourceColor(source.color_key)};">
        <span class="source-name">{source.display_name}</span>
        <span class="source-meta">{source.trajectory.messages.length} messages</span>
      </div>

      {#each rows as row (row.renderId)}
        <div
          class="brick message-brick"
          data-render-id={row.renderId}
          class:selected={selectedRenderId === row.renderId}
          class:sidechain={row.isSidechain}
          style="border-left-color: {row.borderColor};"
          onclick={() => onSelect(row.renderId)}
          role="button"
          tabindex="0"
          onkeydown={(e) => e.key === "Enter" && onSelect(row.renderId)}
          oncontextmenu={(e) => handleContextMenu(e, row.renderId)}
        >
          <div class="brick-header">
            <span class="kind">{row.role}</span>
            {#if row.timestamp}
              <span class="time">{row.timestamp}</span>
            {/if}
          </div>
          {#if row.blockCount > 0}
            <div class="brick-meta">{row.blockCount} block{row.blockCount > 1 ? 's' : ''}</div>
          {/if}
        </div>

        {#each row.blocks as block (block.renderId)}
          <div
            class="brick block-brick"
            data-render-id={block.renderId}
            class:selected={selectedRenderId === block.renderId}
            class:sidechain={row.isSidechain}
            style="border-left-color: {block.borderColor}; margin-left: 24px;"
            onclick={() => onSelect(block.renderId)}
            role="button"
            tabindex="0"
            onkeydown={(e) => e.key === "Enter" && onSelect(block.renderId)}
            oncontextmenu={(e) => handleContextMenu(e, block.renderId)}
          >
            <div class="brick-header">
              <span class="kind">{block.kind}</span>
            </div>
            <div class="brick-preview">
              {block.preview}
            </div>
          </div>
        {/each}
      {/each}
    </div>
  {:else}
    <div class="placeholder">Source is not loaded</div>
  {/if}
</div>

{#if nodeContextMenu}
  <div
    class="node-context-menu-overlay"
    onclick={closeContextMenu}
    oncontextmenu={(e) => { e.preventDefault(); closeContextMenu(); }}
    role="presentation"
  ></div>
  <div
    class="node-context-menu"
    style="left: {nodeContextMenu.x}px; top: {nodeContextMenu.y}px;"
  >
    <button onclick={() => { onQuickAddBookmark(nodeContextMenu!.renderId); closeContextMenu(); }}>
      Add Bookmark
    </button>
  </div>
{/if}

<style>
  .bricks-container {
    display: flex;
    flex-direction: column;
    gap: 24px;
    padding: 16px;
    max-width: 800px;
    margin: 0 auto;
  }

  .placeholder {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #adb5bd;
    font-size: 18px;
  }

  .source-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .source-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 14px;
    background: #ffffff;
    border: 1px solid #e9ecef;
    border-left: 4px solid #adb5bd;
    border-radius: 6px;
    font-size: 13px;
  }

  .source-name {
    font-weight: 600;
    color: #343a40;
  }

  .source-meta {
    color: #868e96;
    font-size: 12px;
  }

  .brick {
    background: #ffffff;
    border: 1px solid #e9ecef;
    border-left: 4px solid #adb5bd;
    border-radius: 6px;
    padding: 10px 14px;
    cursor: pointer;
    transition: background 0.15s, box-shadow 0.15s;
  }

  .brick:hover {
    background: #f8f9fa;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.05);
  }

  .brick.selected {
    background: #e7f5ff;
    border-color: #74c0fc;
    box-shadow: 0 0 0 2px #a5d8ff;
  }

  .brick.sidechain {
    opacity: 0.6;
  }

  .message-brick {
    background: #ffffff;
  }

  .block-brick {
    background: #f8f9fa;
  }

  .brick-header {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12px;
  }

  .kind {
    font-weight: 600;
    color: #343a40;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .time {
    margin-left: auto;
    color: #adb5bd;
  }

  .brick-meta {
    color: #868e96;
    font-size: 11px;
    margin-top: 4px;
  }

  .brick-preview {
    color: #495057;
    font-size: 13px;
    line-height: 1.4;
    white-space: pre-wrap;
    word-break: break-word;
    margin-top: 6px;
  }

  .node-context-menu-overlay {
    position: fixed;
    inset: 0;
    z-index: 9998;
  }

  .node-context-menu {
    position: fixed;
    z-index: 9999;
    background: #ffffff;
    border: 1px solid #dee2e6;
    border-radius: 6px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
    display: flex;
    flex-direction: column;
    min-width: 140px;
    padding: 4px;
  }

  .node-context-menu button {
    background: none;
    border: none;
    padding: 8px 12px;
    text-align: left;
    font-size: 13px;
    color: #212529;
    cursor: pointer;
    border-radius: 4px;
  }

  .node-context-menu button:hover {
    background: #f1f3f5;
  }
</style>
