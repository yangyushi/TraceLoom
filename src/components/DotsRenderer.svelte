<script lang="ts">
  import type { Trajectory, Node } from "../types/ir";
  import { getFillColor } from "../lib/colors";

  interface Props {
    trajectory: Trajectory;
    onSelect: (node: Node) => void;
    selectedNode: Node | null;
  }

  let { trajectory, onSelect, selectedNode }: Props = $props();

  function getLabel(node: Node): string {
    return node.kind;
  }

  function getTitle(node: Node): string {
    const ts = node.timestamp ? new Date(node.timestamp).toLocaleString() : "no time";
    return `${node.kind} | ${node.role[0]} | ${ts}`;
  }

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
  const spacing = 40;
  const radius = 10;
  const svgWidth = 200;
  const svgHeight = $derived(sortedNodes.length * spacing + spacing);
</script>

<div class="dots-container">
  <svg width={svgWidth} height={svgHeight}>
    <line x1={svgWidth / 2} y1={spacing} x2={svgWidth / 2} y2={svgHeight - spacing} stroke="#dee2e6" stroke-width="2" />
    {#each sortedNodes as node, i}
      <g
        class="dot-group"
        class:selected={selectedNode?.id === node.id}
        class:sidechain={node.is_sidechain}
        onclick={() => onSelect(node)}
        onkeydown={(e) => e.key === "Enter" && onSelect(node)}
        role="button"
        tabindex="0"
        style="cursor: pointer;"
        transform="translate({svgWidth / 2}, {spacing + i * spacing})"
      >
        <circle
          r={radius}
          fill={getFillColor(node.kind)}
          stroke={selectedNode?.id === node.id ? "#212529" : "transparent"}
          stroke-width="2"
        />
        <text x={radius + 14} y={4} text-anchor="start" fill="#495057" font-size="11">{getLabel(node)}</text>
        <title>{getTitle(node)}</title>
      </g>
    {/each}
  </svg>
</div>

<style>
  .dots-container {
    padding: 24px;
    overflow: auto;
    display: flex;
    justify-content: center;
  }

  .dot-group:hover circle {
    filter: brightness(1.15);
  }

  .dot-group.selected circle {
    stroke: #212529;
    stroke-width: 2;
  }

  .dot-group.sidechain {
    opacity: 0.5;
  }

  .dot-group.sidechain circle {
    stroke-dasharray: 3 2;
  }
</style>
