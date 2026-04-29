<script lang="ts">
  import type { TrajectorySource, RenderId } from "../types/workspace";
  import { namespaceId, parseRenderId } from "../lib/workspace";
  import { getStrokeColor, getSourceColor } from "../lib/colors";
  import { getBlockPreview } from "../lib/blockPreview";
  import { orderedItems, topologicalMessages } from "../lib/order";

  interface Props {
    source: TrajectorySource;
    onSelect: (renderId: RenderId) => void;
    selectedRenderId: RenderId | null;
    onQuickAddBookmark: (renderId: RenderId) => void;
  }

  let { source, onSelect, selectedRenderId, onQuickAddBookmark }: Props = $props();

  let nodeContextMenu = $state<{ x: number; y: number; renderId: string } | null>(null);

  function handleContextMenu(e: MouseEvent, renderId: string) {
    e.preventDefault();
    nodeContextMenu = { x: e.clientX, y: e.clientY, renderId };
  }

  function closeContextMenu() {
    nodeContextMenu = null;
  }

  function isMessageSelected(msgId: string): boolean {
    return selectedRenderId === namespaceId(source.id, msgId);
  }

  function isBlockSelected(blockId: string): boolean {
    return selectedRenderId === namespaceId(source.id, blockId);
  }

  function formatTimestamp(ts: string | null): string {
    if (!ts) return "";
    return new Date(ts).toLocaleTimeString();
  }

  function getAllRenderIds(): string[] {
    if (!source.trajectory) return [];
    const ids: string[] = [];
    for (const item of orderedItems(source.trajectory.messages)) {
      ids.push(namespaceId(source.id, item.type === "message" ? item.message.id : item.block.id));
    }
    return ids;
  }

  function getNavigationTarget(key: string): string | null {
    if (!selectedRenderId || key === "ArrowLeft" || key === "ArrowRight") return null;

    const ids = getAllRenderIds();
    const idx = ids.indexOf(selectedRenderId);
    if (idx < 0) return null;

    if (key === "ArrowUp") return ids[idx - 1] ?? null;
    if (key === "ArrowDown") return ids[idx + 1] ?? null;
    return null;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!["ArrowUp", "ArrowDown"].includes(e.key)) return;
    const target = e.target as HTMLElement;
    if (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable) {
      return;
    }
    e.preventDefault();
    const nextId = getNavigationTarget(e.key);
    if (nextId) {
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

<div class="bricks-container">
  {#if source.trajectory}
    <div class="source-section">
      <div class="source-header" style="border-left-color: {getSourceColor(source.color_key)};">
        <span class="source-name">{source.display_name}</span>
        <span class="source-meta">{source.trajectory.messages.length} messages</span>
      </div>

      {#each topologicalMessages(source.trajectory.messages) as msg}
        <div
          class="brick message-brick"
          data-render-id={namespaceId(source.id, msg.id)}
          class:selected={isMessageSelected(msg.id)}
          class:sidechain={msg.is_sidechain}
          style="border-left-color: {getStrokeColor(msg.role)};"
          onclick={() => onSelect(namespaceId(source.id, msg.id))}
          role="button"
          tabindex="0"
          onkeydown={(e) => e.key === "Enter" && onSelect(namespaceId(source.id, msg.id))}
          oncontextmenu={(e) => handleContextMenu(e, namespaceId(source.id, msg.id))}
        >
          <div class="brick-header">
            <span class="kind">{msg.role}</span>
            {#if msg.timestamp}
              <span class="time">{formatTimestamp(msg.timestamp)}</span>
            {/if}
          </div>
          {#if msg.blocks.length > 0}
            <div class="brick-meta">{msg.blocks.length} block{msg.blocks.length > 1 ? 's' : ''}</div>
          {/if}
        </div>

        {#each msg.blocks as block}
          <div
            class="brick block-brick"
            data-render-id={namespaceId(source.id, block.id)}
            class:selected={isBlockSelected(block.id)}
            class:sidechain={msg.is_sidechain}
            style="border-left-color: {getStrokeColor(block.kind)}; margin-left: 24px;"
            onclick={() => onSelect(namespaceId(source.id, block.id))}
            role="button"
            tabindex="0"
            onkeydown={(e) => e.key === "Enter" && onSelect(namespaceId(source.id, block.id))}
            oncontextmenu={(e) => handleContextMenu(e, namespaceId(source.id, block.id))}
          >
            <div class="brick-header">
              <span class="kind">{block.kind}</span>
            </div>
            <div class="brick-preview">
              {getBlockPreview(block)}
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
