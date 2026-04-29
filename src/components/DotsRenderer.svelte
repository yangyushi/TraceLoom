<script lang="ts">
  import type { TrajectorySource, RenderId } from "../types/workspace";
  import { namespaceId } from "../lib/workspace";
  import { getFillColor, getStrokeColor, getSourceColor } from "../lib/colors";
  import { topologicalMessages } from "../lib/order";
  import type cytoscape from "cytoscape";
  import { onDestroy, untrack } from "svelte";

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
  let resizeObserver: ResizeObserver | null = null;
  let previousSelectedRenderId: RenderId | null = null;
  let nodeContextMenu = $state<{ x: number; y: number; renderId: string } | null>(null);

  const MSG_X = 0;
  const BLOCK_X_OFFSET = 140;
  const BLOCK_H_SPACING = 45;
  const MIN_MSG_SPACING = 100;
  const PADDING = 40;
  const LANE_WIDTH = 400;
  const INITIAL_ZOOM = 1;

  interface NavigationRow {
    sourceIndex: number;
    msgIndex: number;
    messageId: string;
    blockIds: string[];
  }

  const renderSignature = $derived.by(() =>
    sources
      .map((source) => {
        const trajectory = source.trajectory;
        return [
          source.id,
          source.visibility_state,
          source.display_name,
          trajectory?.messages.length ?? 0,
          trajectory?.messages.map((msg) => `${msg.id}:${msg.blocks.length}`).join(",") ?? "",
        ].join("|");
      })
      .join(";")
  );

  const navigationRows = $derived.by(() => {
    const rows: NavigationRow[] = [];
    for (let si = 0; si < sources.length; si++) {
      const traj = sources[si].trajectory;
      if (!traj) continue;
      const msgOrder = topologicalMessages(traj.messages);
      for (let mi = 0; mi < msgOrder.length; mi++) {
        const msg = msgOrder[mi];
        rows.push({
          sourceIndex: si,
          msgIndex: mi,
          messageId: namespaceId(sources[si].id, msg.id),
          blockIds: msg.blocks.map((block) => namespaceId(sources[si].id, block.id)),
        });
      }
    }
    return rows;
  });

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
    previousSelectedRenderId = null;

    const allElements: cytoscape.ElementDefinition[] = [];

    for (let i = 0; i < sources.length; i++) {
      const { elements } = buildSourceElements(sources[i], i);
      allElements.push(...elements);
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

    // Keep message nodes at a fixed rendered size on import/load.
    if (allElements.length > 0) {
      cy.zoom(INITIAL_ZOOM);
      cy.pan({ x: Math.max(PADDING, rect.width / 2 - BLOCK_X_OFFSET), y: PADDING + 40 });
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

    updateSelection();
  }

  function updateSelection() {
    if (!cy) return;
    const id = selectedRenderId;
    if (id === previousSelectedRenderId) return;

    const previousId = previousSelectedRenderId;
    previousSelectedRenderId = id;

    if (previousId) {
      const previous = cy.getElementById(previousId);
      if (previous.length > 0) {
        previous.unselect();
      }
    }

    if (id) {
      const ele = cy.getElementById(id);
      if (ele.length > 0) {
        ele.select();
        cy.stop(false, true);
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
    const signature = renderSignature;
    void signature;
    if (c) {
      void untrack(createCy);
    }
  });

  $effect(() => {
    const c = container;
    if (!c) return;

    resizeObserver?.disconnect();
    resizeObserver = new ResizeObserver(() => {
      cy?.resize();
    });
    resizeObserver.observe(c);

    return () => {
      resizeObserver?.disconnect();
      resizeObserver = null;
    };
  });

  $effect(() => {
    const id = selectedRenderId;
    void id;
    updateSelection();
  });

  onDestroy(() => {
    resizeObserver?.disconnect();
    if (cy) {
      cy.destroy();
      cy = null;
    }
    previousSelectedRenderId = null;
  });

  function getNavigationTarget(key: string): string | null {
    if (!selectedRenderId) return null;

    const rows = navigationRows;
    const rowIndex = rows.findIndex((row) =>
      row.messageId === selectedRenderId || row.blockIds.includes(selectedRenderId)
    );
    if (rowIndex < 0) return null;

    const row = rows[rowIndex];
    const isMessage = row.messageId === selectedRenderId;
    const isBlock = row.blockIds.includes(selectedRenderId);

    switch (key) {
      case "ArrowLeft":
        return isBlock ? row.messageId : null;
      case "ArrowRight":
        return isMessage ? row.blockIds[0] ?? null : null;
      case "ArrowUp":
        return rows[rowIndex - 1]?.messageId ?? null;
      case "ArrowDown":
        return rows[rowIndex + 1]?.messageId ?? null;
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
