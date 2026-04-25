<script lang="ts">
  import type { Trajectory, Message, Block } from "../types/ir";
  import { getFillColor, getStrokeColor } from "../lib/colors";
  import cytoscape from "cytoscape";
  import { onDestroy } from "svelte";

  interface Props {
    trajectory: Trajectory;
    onSelect: (id: string) => void;
    selectedId: string | null;
  }

  let { trajectory, onSelect, selectedId }: Props = $props();

  let container: HTMLDivElement | null = $state(null);
  let cy: cytoscape.Core | null = null;

  const MSG_X = 0;
  const BLOCK_X_OFFSET = 140;
  const BLOCK_H_SPACING = 45;
  const BLOCK_SPACING = 50;
  const MIN_MSG_SPACING = 100;
  const PADDING = 40;

  function topologicalSort(messages: Message[]): Message[] {
    const msgMap = new Map(messages.map((m) => [m.id, m]));
    const inDegree = new Map<string, number>();
    const adj = new Map<string, string[]>();

    for (const m of messages) {
      inDegree.set(m.id, 0);
    }
    for (const m of messages) {
      if (m.parent_id) {
        adj.set(m.parent_id, [...(adj.get(m.parent_id) || []), m.id]);
        inDegree.set(m.id, (inDegree.get(m.id) || 0) + 1);
      }
    }

    const queue = messages.filter((m) => (inDegree.get(m.id) || 0) === 0);
    const result: Message[] = [];
    const visited = new Set<string>();

    while (queue.length > 0) {
      const msg = queue.shift()!;
      if (visited.has(msg.id)) continue;
      visited.add(msg.id);
      result.push(msg);

      for (const childId of adj.get(msg.id) || []) {
        const child = msgMap.get(childId);
        if (!child) continue;
        const deg = (inDegree.get(childId) || 0) - 1;
        inDegree.set(childId, deg);
        if (deg === 0 && !visited.has(childId)) {
          queue.push(child);
        }
      }
    }

    for (const m of messages) {
      if (!visited.has(m.id)) result.push(m);
    }

    return result;
  }

  function buildElements(): cytoscape.ElementDefinition[] {
    const elements: cytoscape.ElementDefinition[] = [];
    const msgOrder = topologicalSort(trajectory.messages);

    // Compute message Y positions (adaptive spacing)
    let currentY = 0;
    const msgY = new Map<string, number>();
    for (const msg of msgOrder) {
      msgY.set(msg.id, currentY);
      const blockClusterHeight = msg.blocks.length * BLOCK_SPACING;
      const spacing = Math.max(MIN_MSG_SPACING, blockClusterHeight + PADDING);
      currentY += spacing;
    }

    // Create message nodes
    for (const msg of msgOrder) {
      const y = msgY.get(msg.id)!;
      elements.push({
        data: {
          id: msg.id,
          label: msg.role.charAt(0).toUpperCase(),
          color: getFillColor(msg.role),
          stroke: getStrokeColor(msg.role),
          isMessage: true,
        },
        position: { x: MSG_X, y },
      });
    }

    // Create block nodes
    for (const msg of msgOrder) {
      const my = msgY.get(msg.id)!;
      const n = msg.blocks.length;
      for (let i = 0; i < n; i++) {
        const block = msg.blocks[i];
        const yOffset = (i - (n - 1) / 2) * BLOCK_SPACING;
        elements.push({
          data: {
            id: block.id,
            label: block.kind,
            color: getFillColor(block.kind),
            stroke: getStrokeColor(block.kind),
            isMessage: false,
          },
          position: { x: MSG_X + BLOCK_X_OFFSET + i * BLOCK_H_SPACING, y: my + yOffset },
        });
      }
    }

    // Create edges
    for (const msg of msgOrder) {
      if (msg.parent_id) {
        elements.push({
          data: {
            id: `${msg.parent_id}-${msg.id}`,
            source: msg.parent_id,
            target: msg.id,
            edgeType: "chain",
          },
        });
      }
      for (const block of msg.blocks) {
        elements.push({
          data: {
            id: `${msg.id}-${block.id}`,
            source: msg.id,
            target: block.id,
            edgeType: "contain",
          },
        });
        if (block.tool_call_id) {
          elements.push({
            data: {
              id: `${block.tool_call_id}-${block.id}`,
              source: block.tool_call_id,
              target: block.id,
              edgeType: "tool",
            },
          });
        }
      }
    }

    return elements;
  }

  function createCy() {
    if (!container) return;

    const rect = container.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) {
      requestAnimationFrame(createCy);
      return;
    }

    if (cy) {
      cy.destroy();
      cy = null;
    }

    cy = cytoscape({
      container,
      elements: buildElements(),
      style: [
        {
          selector: "node",
          style: {
            "background-color": "data(color)",
            "border-color": "data(stroke)",
            "border-width": 2,
            width: (ele: any) => (ele.data("isMessage") ? "28px" : "16px"),
            height: (ele: any) => (ele.data("isMessage") ? "28px" : "16px"),
            label: "data(label)",
            "font-size": (ele: any) =>
              ele.data("isMessage") ? "13px" : "10px",
            "text-valign": "bottom",
            "text-halign": "center",
            "text-margin-y": 5,
            color: "#495057",
            "font-weight": (ele: any) =>
              ele.data("isMessage") ? "bold" : "normal",
            "text-background-color": "#f8f9fa",
            "text-background-opacity": 0.8,
            "text-background-padding": 2,
          } as any,
        },
        {
          selector: "edge[edgeType = 'chain']",
          style: {
            width: 1.5,
            "line-color": "#adb5bd",
            "target-arrow-color": "#adb5bd",
            "target-arrow-shape": "triangle",
            "curve-style": "bezier",
            "arrow-scale": 0.7,
          } as any,
        },
        {
          selector: "edge[edgeType = 'contain']",
          style: {
            width: 1,
            "line-color": "#ced4da",
            "line-style": "dashed",
            "target-arrow-color": "#ced4da",
            "target-arrow-shape": "triangle",
            "curve-style": "bezier",
            "arrow-scale": 0.6,
          } as any,
        },
        {
          selector: "edge[edgeType = 'tool']",
          style: {
            width: 1.5,
            "line-color": "#339af0",
            "target-arrow-color": "#339af0",
            "target-arrow-shape": "triangle",
            "curve-style": "unbundled-bezier",
            "arrow-scale": 0.7,
            "control-point-distances": [40],
            "control-point-weights": [0.5],
          } as any,
        },
        {
          selector: ":selected",
          style: {
            "border-color": "#212529",
            "border-width": 3,
            "background-color": "#e7f5ff",
          } as any,
        },
      ] as any,
      layout: {
        name: "preset",
        fit: true,
        padding: 40,
      } as any,
      minZoom: 0.1,
      maxZoom: 3,
      wheelSensitivity: 0.3,
    });

    cy.on("tap", "node", (evt: cytoscape.EventObjectNode) => {
      const id = evt.target.id();
      if (id) onSelect(id);
    });
  }

  function updateSelection() {
    if (!cy) return;
    cy.nodes().unselect();
    const id = selectedId;
    if (id) {
      const ele = cy.getElementById(id);
      if (ele.length > 0) ele.select();
    }
  }

  // Initialize when container is bound or trajectory changes
  $effect(() => {
    const c = container;
    const t = trajectory;
    void t;
    if (c) {
      createCy();
    }
  });

  // Update selection highlight when selectedId changes
  $effect(() => {
    const id = selectedId;
    void id;
    updateSelection();
  });

  onDestroy(() => {
    if (cy) {
      cy.destroy();
      cy = null;
    }
  });

  function getNavigationTarget(key: string): string | null {
    if (!selectedId) return null;

    const msgOrder = topologicalSort(trajectory.messages);
    const msgIndex = msgOrder.findIndex((m) => m.id === selectedId);

    if (msgIndex >= 0) {
      const msg = msgOrder[msgIndex];
      switch (key) {
        case "ArrowRight":
          return msg.blocks[0]?.id ?? null;
        case "ArrowUp":
          return msgOrder[msgIndex - 1]?.id ?? null;
        case "ArrowDown":
          return msgOrder[msgIndex + 1]?.id ?? null;
      }
    } else {
      for (let i = 0; i < msgOrder.length; i++) {
        const msg = msgOrder[i];
        const blockIndex = msg.blocks.findIndex((b) => b.id === selectedId);
        if (blockIndex >= 0) {
          switch (key) {
            case "ArrowRight":
              return msg.blocks[blockIndex + 1]?.id ?? null;
            case "ArrowLeft":
              if (blockIndex === 0) return msg.id;
              return msg.blocks[blockIndex - 1]?.id ?? null;
            case "ArrowUp":
              return msgOrder[i - 1]?.id ?? null;
            case "ArrowDown":
              return msgOrder[i + 1]?.id ?? null;
          }
        }
      }
    }
    return null;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!["ArrowRight", "ArrowLeft", "ArrowUp", "ArrowDown"].includes(e.key)) return;
    const target = e.target as HTMLElement;
    if (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable) {
      return;
    }
    e.preventDefault();
    const nextId = getNavigationTarget(e.key);
    if (nextId) {
      onSelect(nextId);
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="dots-container" bind:this={container}></div>

<style>
  .dots-container {
    width: 100%;
    height: 100%;
    background: #f8f9fa;
  }
</style>
