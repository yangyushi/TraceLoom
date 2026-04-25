<script lang="ts">
  import type { Trajectory, Node, Message, Block } from "../types/ir";
  import { getStrokeColor } from "../lib/colors";

  interface Props {
    trajectory: Trajectory;
    onSelect: (node: Node) => void;
    selectedNode: Node | null;
  }

  let { trajectory, onSelect, selectedNode }: Props = $props();

  const nodeById = $derived(
    new Map(trajectory.nodes.map((n) => [n.id, n]))
  );

  function handleSelectMessage(msg: Message) {
    const node = nodeById.get(msg.id);
    if (node) onSelect(node);
  }

  function handleSelectBlock(block: Block) {
    const node = nodeById.get(block.id);
    if (node) onSelect(node);
  }

  function isMessageSelected(msg: Message): boolean {
    return selectedNode?.id === msg.id;
  }

  function isBlockSelected(block: Block): boolean {
    return selectedNode?.id === block.id;
  }

  function formatTimestamp(ts: string | null): string {
    if (!ts) return "";
    return new Date(ts).toLocaleTimeString();
  }

  function getBlockPreview(content: Block["content"]): string {
    switch (content.type) {
      case "Text":
        return content.data.slice(0, 120) + (content.data.length > 120 ? "..." : "");
      case "ToolUse":
        return `Tool: ${content.data.name}`;
      case "ToolResult":
        return `Result: ${content.data.output.slice(0, 120)}${content.data.output.length > 120 ? "..." : ""}`;
      case "Thinking":
        return content.data.encrypted ? "[encrypted reasoning]" : content.data.text.slice(0, 120) + (content.data.text.length > 120 ? "..." : "");
      case "Snapshot":
        return content.data.description;
      case "Custom":
        return `[${content.data.kind}]`;
      default:
        return `[${(content as { type: string }).type}]`;
    }
  }
</script>

<div class="bricks-container">
  {#each trajectory.messages as msg}
    <div
      class="brick message-brick"
      class:selected={isMessageSelected(msg)}
      class:sidechain={msg.is_sidechain}
      style="border-left-color: {getStrokeColor(msg.role[0])};"
      onclick={() => handleSelectMessage(msg)}
      role="button"
      tabindex="0"
      onkeydown={(e) => e.key === "Enter" && handleSelectMessage(msg)}
    >
      <div class="brick-header">
        <span class="kind">{msg.role[0]}</span>
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
        class:selected={isBlockSelected(block)}
        class:sidechain={msg.is_sidechain}
        style="border-left-color: {getStrokeColor(block.kind)}; margin-left: 24px;"
        onclick={() => handleSelectBlock(block)}
        role="button"
        tabindex="0"
        onkeydown={(e) => e.key === "Enter" && handleSelectBlock(block)}
      >
        <div class="brick-header">
          <span class="kind">{block.kind}</span>
        </div>
        <div class="brick-preview">
          {getBlockPreview(block.content)}
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
