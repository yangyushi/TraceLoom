<script lang="ts">
  import type { TrajectorySource, RenderId } from "../types/workspace";
  import { namespaceId, parseRenderId } from "../lib/workspace";
  import { getFillColor, getStrokeColor, getSourceColor } from "../lib/colors";
  import { topologicalMessages } from "../lib/order";
  import type cytoscape from "cytoscape";
  import { onDestroy } from "svelte";

  interface Props {
    sources: TrajectorySource[];
    onSelect: (renderId: RenderId) => void;
    selectedRenderId: RenderId | null;
    onQuickAddBookmark: (renderId: RenderId) => void;
  }

  let { sources, onSelect, selectedRenderId, onQuickAddBookmark }: Props = $props();

  let container: HTMLDivElement | null = $state(null);
  let cy: cytoscape.Core | null = null;
  let cytoscapeFactory: typeof cytoscape | null = null;
  let nodeContextMenu = $state<{ x: number; y: number; renderId: string } | null>(null);

  const MSG_X = 0;
  const BLOCK_X_OFFSET = 140;
  const BLOCK_H_SPACING = 45;
  const MIN_MSG_SPACING = 100;
  const PADDING = 40;
  const LANE_WIDTH = 400;

  async function loadCytoscape(): Promise<typeof cytoscape> {
    if (!cytoscapeFactory) {
      cytoscapeFactory = (await import("cytoscape")).default;
    }
    return cytoscapeFactory;
  }

  function sheet(selector: string, style: Record<string, unknown>): cytoscape.StylesheetJsonBlock {
    return { selector, style } as cytoscape.StylesheetJsonBlock;
  }

  interface SourceElements {
    elements: cytoscape.ElementDefinition[];
    msgOrder: { msgId: string; y: number }[];
  }

  function buildSourceElements(source: TrajectorySource, laneIndex: number): SourceElements {
    const elements: cytoscape.ElementDefinition[] = [];
    const laneOffset = laneIndex * LANE_WIDTH;
    if (!source.trajectory) return { elements: [], msgOrder: [] };
    const msgOrder = topologicalMessages(source.trajectory.messages);

    let currentY = 0;
    const msgY = new Map<string, number>();
    for (const msg of msgOrder) {
      msgY.set(msg.id, currentY);
      const spacing = Math.max(MIN_MSG_SPACING, PADDING);
      currentY += spacing;
    }

    // Source label
    elements.push({
      data: {
        id: `label-${source.id}`,
        label: source.display_name,
        color: getSourceColor(source.color_key),
        isLabel: true,
      },
      position: { x: laneOffset + MSG_X, y: -40 },
    });

    for (const msg of msgOrder) {
      const y = msgY.get(msg.id)!;
      const renderId = namespaceId(source.id, msg.id);
      elements.push({
        data: {
          id: renderId,
          label: msg.role.charAt(0).toUpperCase(),
          color: getFillColor(msg.role),
          stroke: getStrokeColor(msg.role),
          isMessage: true,
          sourceId: source.id,
        },
        position: { x: laneOffset + MSG_X, y },
      });
    }

    for (const msg of msgOrder) {
      const my = msgY.get(msg.id)!;
      const n = msg.blocks.length;
      for (let i = 0; i < n; i++) {
        const block = msg.blocks[i];
        const renderId = namespaceId(source.id, block.id);
        elements.push({
          data: {
            id: renderId,
            label: block.kind,
            color: getFillColor(block.kind),
            stroke: getStrokeColor(block.kind),
            isMessage: false,
            sourceId: source.id,
          },
          position: { x: laneOffset + BLOCK_X_OFFSET + i * BLOCK_H_SPACING, y: my },
        });
      }
    }

    const nodeIds = new Set(elements.map((el) => el.data!.id as string));

    for (const msg of msgOrder) {
      const msgRenderId = namespaceId(source.id, msg.id);
      if (msg.parent_id && nodeIds.has(namespaceId(source.id, msg.parent_id))) {
        elements.push({
          data: {
            id: `edge-${source.id}-${msg.parent_id}-${msg.id}`,
            source: namespaceId(source.id, msg.parent_id),
            target: msgRenderId,
            edgeType: "chain",
          },
        });
      }
      for (const block of msg.blocks) {
        const blockRenderId = namespaceId(source.id, block.id);
        elements.push({
          data: {
            id: `edge-${source.id}-${msg.id}-${block.id}`,
            source: msgRenderId,
            target: blockRenderId,
            edgeType: "contain",
          },
        });
        if (block.tool_call_id && nodeIds.has(namespaceId(source.id, block.tool_call_id))) {
          elements.push({
            data: {
              id: `edge-${source.id}-${block.tool_call_id}-${block.id}`,
              source: namespaceId(source.id, block.tool_call_id),
              target: blockRenderId,
              edgeType: "tool",
            },
          });
        }
      }
    }

    return {
      elements,
      msgOrder: msgOrder.map((m) => ({ msgId: m.id, y: msgY.get(m.id)! })),
    };
  }

  async function createCy() {
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

    const allElements: cytoscape.ElementDefinition[] = [];
    let maxY = 0;

    for (let i = 0; i < sources.length; i++) {
      const { elements, msgOrder } = buildSourceElements(sources[i], i);
      allElements.push(...elements);
      for (const m of msgOrder) {
        maxY = Math.max(maxY, m.y);
      }
    }

    const cytoscape = await loadCytoscape();
    if (!container) return;

    cy = cytoscape({
      container,
      elements: allElements,
      style: [
        sheet("node", {
          "background-color": "data(color)",
          "border-color": "data(stroke)",
          "border-width": 2,
          width: (ele: cytoscape.NodeSingular) => (ele.data("isMessage") ? "28px" : "16px"),
          height: (ele: cytoscape.NodeSingular) => (ele.data("isMessage") ? "28px" : "16px"),
          label: "data(label)",
          "font-size": (ele: cytoscape.NodeSingular) =>
            ele.data("isMessage") ? "13px" : "10px",
          "text-valign": "bottom",
          "text-halign": "center",
          "text-margin-y": 5,
          color: "#495057",
          "font-weight": (ele: cytoscape.NodeSingular) =>
            ele.data("isMessage") ? "bold" : "normal",
          "text-background-color": "#f8f9fa",
          "text-background-opacity": 0.8,
          "text-background-padding": 2,
        }),
        sheet("node[isLabel]", {
          width: "1px",
          height: "1px",
          "background-opacity": 0,
          "border-width": 0,
          "font-size": "12px",
          "font-weight": "bold",
          color: "data(color)",
          "text-valign": "center",
          "text-halign": "center",
        }),
        sheet("edge[edgeType = 'chain']", {
          width: 1.5,
          "line-color": "#adb5bd",
          "target-arrow-color": "#adb5bd",
          "target-arrow-shape": "triangle",
          "curve-style": "bezier",
          "arrow-scale": 0.7,
        }),
        sheet("edge[edgeType = 'contain']", {
          width: 1,
          "line-color": "#ced4da",
          "line-style": "dashed",
          "target-arrow-color": "#ced4da",
          "target-arrow-shape": "triangle",
          "curve-style": "bezier",
          "arrow-scale": 0.6,
        }),
        sheet("edge[edgeType = 'tool']", {
          width: 1.5,
          "line-color": "#339af0",
          "target-arrow-color": "#339af0",
          "target-arrow-shape": "triangle",
          "curve-style": "unbundled-bezier",
          "arrow-scale": 0.7,
          "control-point-distances": [40],
          "control-point-weights": [0.5],
        }),
        sheet(":selected", {
          "border-color": "#212529",
          "border-width": 3,
          "background-color": "#e7f5ff",
        }),
      ],
      layout: {
        name: "preset",
        fit: false,
        padding: 40,
      },
      minZoom: 0.05,
      maxZoom: 3,
      wheelSensitivity: 0.3,
      autoungrabify: true,
    });

    // Fit to content
    if (allElements.length > 0) {
      const zoom = Math.min(3, Math.max(0.05, rect.height / (maxY + MIN_MSG_SPACING)));
      cy.zoom(zoom);
      cy.center();
    }

    cy.on("tap", "node", (evt: cytoscape.EventObjectNode) => {
      const id = evt.target.id();
      if (id && !evt.target.data("isLabel")) {
        onSelect(id);
      }
    });

    cy.on("dbltap", "node", (evt: cytoscape.EventObjectNode) => {
      const id = evt.target.id();
      if (!id || !cy || evt.target.data("isLabel")) return;
      const ele = cy.getElementById(id);
      if (ele.length > 0) {
        cy.animate(
          {
            center: { eles: ele },
            zoom: cy.zoom(),
          },
          { duration: 300 }
        );
      }
    });

    cy.on("cxttap", "node", (evt: cytoscape.EventObjectNode) => {
      const id = evt.target.id();
      if (!id || evt.target.data("isLabel")) return;
      const pos = evt.target.renderedPosition();
      if (pos) {
        nodeContextMenu = { x: pos.x, y: pos.y, renderId: id };
      }
    });

    cy.on("tap", (evt: cytoscape.EventObject) => {
      if (evt.target === cy || evt.target.isNode?.()) {
        nodeContextMenu = null;
      }
    });
  }

  function updateSelection() {
    if (!cy) return;
    cy.nodes().unselect();
    const id = selectedRenderId;
    if (id) {
      const ele = cy.getElementById(id);
      if (ele.length > 0) {
        ele.select();
        cy.animate(
          {
            center: { eles: ele },
            zoom: cy.zoom(),
          },
          { duration: 150 }
        );
      }
    }
  }

  $effect(() => {
    const c = container;
    const s = sources;
    void s;
    if (c) {
      void createCy();
    }
  });

  $effect(() => {
    const id = selectedRenderId;
    void id;
    updateSelection();
  });

  onDestroy(() => {
    if (cy) {
      cy.destroy();
      cy = null;
    }
  });

  function getAllItems(): { renderId: string; sourceIndex: number; msgIndex: number; isBlock: boolean; blockIndex: number }[] {
    const items: { renderId: string; sourceIndex: number; msgIndex: number; isBlock: boolean; blockIndex: number }[] = [];
    for (let si = 0; si < sources.length; si++) {
      const traj = sources[si].trajectory;
      if (!traj) continue;
      const msgOrder = topologicalMessages(traj.messages);
      for (let mi = 0; mi < msgOrder.length; mi++) {
        const msg = msgOrder[mi];
        items.push({ renderId: namespaceId(sources[si].id, msg.id), sourceIndex: si, msgIndex: mi, isBlock: false, blockIndex: -1 });
        for (let bi = 0; bi < msg.blocks.length; bi++) {
          items.push({ renderId: namespaceId(sources[si].id, msg.blocks[bi].id), sourceIndex: si, msgIndex: mi, isBlock: true, blockIndex: bi });
        }
      }
    }
    return items;
  }

  function getNavigationTarget(key: string): string | null {
    if (!selectedRenderId) return null;

    const allItems = getAllItems();
    const idx = allItems.findIndex((item) => item.renderId === selectedRenderId);
    if (idx < 0) return null;

    const current = allItems[idx];

    switch (key) {
      case "ArrowRight": {
        const trajR = sources[current.sourceIndex].trajectory;
        if (!trajR) return null;
        if (!current.isBlock) {
          const msg = topologicalMessages(trajR.messages)[current.msgIndex];
          return msg.blocks[0] ? namespaceId(sources[current.sourceIndex].id, msg.blocks[0].id) : null;
        }
        const msg = topologicalMessages(trajR.messages)[current.msgIndex];
        return msg.blocks[current.blockIndex + 1] ? namespaceId(sources[current.sourceIndex].id, msg.blocks[current.blockIndex + 1].id) : null;
      }
      case "ArrowLeft": {
        if (!current.isBlock) return null;
        const trajL = sources[current.sourceIndex].trajectory;
        if (!trajL) return null;
        if (current.blockIndex === 0) {
          const msg = topologicalMessages(trajL.messages)[current.msgIndex];
          return namespaceId(sources[current.sourceIndex].id, msg.id);
        }
        const msg = topologicalMessages(trajL.messages)[current.msgIndex];
        return namespaceId(sources[current.sourceIndex].id, msg.blocks[current.blockIndex - 1].id);
      }
      case "ArrowUp":
        return allItems[idx - 1]?.renderId ?? null;
      case "ArrowDown":
        return allItems[idx + 1]?.renderId ?? null;
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

<div class="dots-container" bind:this={container}>
  {#if nodeContextMenu}
    <div
      class="node-context-menu"
      style="left: {nodeContextMenu.x}px; top: {nodeContextMenu.y}px;"
    >
      <button onclick={() => { onQuickAddBookmark(nodeContextMenu!.renderId); nodeContextMenu = null; }}>
        Add Bookmark
      </button>
    </div>
  {/if}
</div>

<style>
  .dots-container {
    width: 100%;
    height: 100%;
    background: #f8f9fa;
    position: relative;
  }

  .node-context-menu {
    position: absolute;
    z-index: 100;
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
