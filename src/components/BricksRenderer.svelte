<script lang="ts">
  import type { Trajectory, Node } from "../types/ir";
  import { getStrokeColor } from "../lib/colors";

  interface Props {
    trajectory: Trajectory;
    onSelect: (node: Node) => void;
    selectedNode: Node | null;
  }

  let { trajectory, onSelect, selectedNode }: Props = $props();

  function topoSort(nodes: Node[], edges: { from: string; to: string }[]): Node[] {
    const idToNode = new Map(nodes.map((n) => [n.id, n]));
    const inDegree = new Map<string, number>();
    const adj = new Map<string, string[]>();
    nodes.forEach((n) => {
      inDegree.set(n.id, 0);
      adj.set(n.id, []);
    });
    edges.forEach((e) => {
      if (adj.has(e.from) && idToNode.has(e.to)) {
        adj.get(e.from)!.push(e.to);
        inDegree.set(e.to, (inDegree.get(e.to) || 0) + 1);
      }
    });
    const queue = nodes
      .filter((n) => (inDegree.get(n.id) || 0) === 0)
      .sort((a, b) => {
        const ta = a.timestamp ? new Date(a.timestamp).getTime() : 0;
        const tb = b.timestamp ? new Date(b.timestamp).getTime() : 0;
        return ta - tb;
      });
    const result: Node[] = [];
    const visited = new Set<string>();
    while (queue.length) {
      const node = queue.shift()!;
      if (visited.has(node.id)) continue;
      visited.add(node.id);
      result.push(node);
      const children = (adj.get(node.id) || [])
        .map((id) => idToNode.get(id)!)
        .filter(Boolean)
        .sort((a, b) => {
          const ta = a.timestamp ? new Date(a.timestamp).getTime() : 0;
          const tb = b.timestamp ? new Date(b.timestamp).getTime() : 0;
          return ta - tb;
        });
      for (const child of children) {
        const deg = (inDegree.get(child.id) || 0) - 1;
        inDegree.set(child.id, deg);
        if (deg === 0 && !visited.has(child.id)) {
          queue.push(child);
        }
      }
    }
    nodes.forEach((n) => {
      if (!visited.has(n.id)) result.push(n);
    });
    return result;
  }

  const sortedNodes = $derived(topoSort(trajectory.nodes, trajectory.edges));

  function formatTimestamp(ts: string | null): string {
    if (!ts) return "";
    return new Date(ts).toLocaleTimeString();
  }
</script>

<div class="bricks-container">
  {#each sortedNodes as node}
    <div
      class="brick"
      class:selected={selectedNode?.id === node.id}
      class:sidechain={node.is_sidechain}
      style="border-left-color: {getStrokeColor(node.kind)}"
      onclick={() => onSelect(node)}
      role="button"
      tabindex="0"
      onkeydown={(e) => e.key === "Enter" && onSelect(node)}
    >
      <div class="brick-header">
        <span class="kind">{node.kind}</span>
        <span class="role">{node.role[0]}</span>
        {#if node.timestamp}
          <span class="time">{formatTimestamp(node.timestamp)}</span>
        {/if}
      </div>
      <div class="brick-preview">
        {#if node.content.type === "Text"}
          {node.content.data.slice(0, 120)}{node.content.data.length > 120 ? "..." : ""}
        {:else if node.content.type === "ToolUse"}
          Tool: {node.content.data.name}
        {:else if node.content.type === "ToolResult"}
          Result: {node.content.data.output.slice(0, 120)}
        {:else if node.content.type === "Thinking"}
          {node.content.data.encrypted ? "[encrypted reasoning]" : node.content.data.text.slice(0, 120)}
        {:else}
          [{node.content.type}]
        {/if}
      </div>
    </div>
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

  .brick-header {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 6px;
    font-size: 12px;
  }

  .kind {
    font-weight: 600;
    color: #343a40;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .role {
    color: #868e96;
    background: #f1f3f5;
    padding: 2px 6px;
    border-radius: 3px;
    font-size: 11px;
  }

  .time {
    margin-left: auto;
    color: #adb5bd;
  }

  .brick-preview {
    color: #495057;
    font-size: 13px;
    line-height: 1.4;
    white-space: pre-wrap;
    word-break: break-word;
  }
</style>
