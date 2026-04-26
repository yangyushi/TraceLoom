<script lang="ts">
  import type { Trajectory, Message, Block } from "../types/ir";
  import { getStrokeColor } from "../lib/colors";
  import { getBlockPreview } from "../lib/blockPreview";
  import { orderedItems, topologicalMessages } from "../lib/order";

  interface Props {
    trajectory: Trajectory;
    onSelect: (id: string) => void;
    selectedId: string | null;
  }

  let { trajectory, onSelect, selectedId }: Props = $props();
  const orderedMessages = $derived(topologicalMessages(trajectory.messages));

  function isMessageSelected(msg: Message): boolean {
    return selectedId === msg.id;
  }

  function isBlockSelected(block: Block): boolean {
    return selectedId === block.id;
  }

  function formatTimestamp(ts: string | null): string {
    if (!ts) return "";
    return new Date(ts).toLocaleTimeString();
  }

  function getNavigationTarget(key: string): string | null {
    if (!selectedId || key === "ArrowLeft" || key === "ArrowRight") return null;

    const ids = orderedItems(trajectory.messages).map((item) =>
      item.type === "message" ? item.message.id : item.block.id
    );

    const idx = ids.indexOf(selectedId);
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
        const el = document.querySelector(`[data-id="${nextId}"]`);
        if (el) {
          el.scrollIntoView({ block: "center", behavior: "smooth" });
        }
      });
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="bricks-container">
  {#each orderedMessages as msg}
    <div
      class="brick message-brick"
      data-id={msg.id}
      class:selected={isMessageSelected(msg)}
      class:sidechain={msg.is_sidechain}
      style="border-left-color: {getStrokeColor(msg.role)};"
      onclick={() => onSelect(msg.id)}
      role="button"
      tabindex="0"
      onkeydown={(e) => e.key === "Enter" && onSelect(msg.id)}
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
        data-id={block.id}
        class:selected={isBlockSelected(block)}
        class:sidechain={msg.is_sidechain}
        style="border-left-color: {getStrokeColor(block.kind)}; margin-left: 24px;"
        onclick={() => onSelect(block.id)}
        role="button"
        tabindex="0"
        onkeydown={(e) => e.key === "Enter" && onSelect(block.id)}
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

<style>
  .bricks-container {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 16px;
    max-width: 800px;
    margin: 0 auto;
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
</style>
