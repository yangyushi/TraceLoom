<script lang="ts">
  import type { Trajectory, Message } from "../types/ir";
  import { getFillColor, getStrokeColor } from "../lib/colors";
  import { topologicalMessages } from "../lib/order";
  import type cytoscape from "cytoscape";
  import { onDestroy } from "svelte";

  interface Props {
    trajectory: Trajectory;
    onSelect: (id: string) => void;
    selectedId: string | null;
  }

  let { trajectory, onSelect, selectedId }: Props = $props();

  let container: HTMLDivElement | null = $state(null);
  let cy: cytoscape.Core | null = null;
  let cytoscapeFactory: typeof cytoscape | null = null;

  const MSG_X = 0;
  const BLOCK_X_OFFSET = 140;
  const BLOCK_H_SPACING = 45;
  const MIN_MSG_SPACING = 100;
  const PADDING = 40;

  async function loadCytoscape(): Promise<typeof cytoscape> {
    if (!cytoscapeFactory) {
      cytoscapeFactory = (await import("cytoscape")).default;
    }
    return cytoscapeFactory;
  }

  function sheet(selector: string, style: Record<string, unknown>): cytoscape.StylesheetJsonBlock {
    return { selector, style } as cytoscape.StylesheetJsonBlock;
  }

  function buildElements(): {
    elements: cytoscape.ElementDefinition[];
    msgOrder: Message[];
    msgY: Map<string, number>;
  } {
    const elements: cytoscape.ElementDefinition[] = [];
    const msgOrder = topologicalMessages(trajectory.messages);

    // Compute message Y positions (adaptive spacing)
    let currentY = 0;
    const msgY = new Map<string, number>();
    for (const msg of msgOrder) {
      msgY.set(msg.id, currentY);
      // Blocks now share the message's Y, so they don't add vertical height
      const blockClusterHeight = 0;
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

    // Create block nodes (all blocks from same message share the message's Y)
    for (const msg of msgOrder) {
      const my = msgY.get(msg.id)!;
      const n = msg.blocks.length;
      for (let i = 0; i < n; i++) {
        const block = msg.blocks[i];
        elements.push({
          data: {
            id: block.id,
            label: block.kind,
            color: getFillColor(block.kind),
            stroke: getStrokeColor(block.kind),
            isMessage: false,
          },
          position: { x: MSG_X + BLOCK_X_OFFSET + i * BLOCK_H_SPACING, y: my },
        });
      }
    }

    // Collect all node IDs so we never create edges to missing nodes
    const nodeIds = new Set(elements.map((el) => el.data!.id as string));

    // Create edges
    for (const msg of msgOrder) {
      if (msg.parent_id && nodeIds.has(msg.parent_id)) {
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
        if (block.tool_call_id && nodeIds.has(block.tool_call_id)) {
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

    return { elements, msgOrder, msgY };
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

    const { elements, msgOrder, msgY } = buildElements();
    const cytoscape = await loadCytoscape();
    if (!container) return;

    cy = cytoscape({
      container,
      elements,
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
    });

    // Fix zoom so that ~10 message nodes fill the viewport vertically
    if (msgOrder.length > 0) {
      const targetCount = Math.min(10, msgOrder.length);
      const targetHeight = msgY.get(msgOrder[targetCount - 1].id)! + MIN_MSG_SPACING;
      const zoom = Math.min(3, Math.max(0.05, rect.height / targetHeight));
      cy.zoom(zoom);
      cy.center(cy.getElementById(msgOrder[0].id));
    }

    cy.on("tap", "node", (evt: cytoscape.EventObjectNode) => {
      const id = evt.target.id();
      if (id) onSelect(id);
    });

    cy.on("dbltap", "node", (evt: cytoscape.EventObjectNode) => {
      const id = evt.target.id();
      if (!id || !cy) return;
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
  }

  function updateSelection() {
    if (!cy) return;
    cy.nodes().unselect();
    const id = selectedId;
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

  // Initialize when container is bound or trajectory changes
  $effect(() => {
    const c = container;
    const t = trajectory;
      void t;
    if (c) {
      void createCy();
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

    const msgOrder = topologicalMessages(trajectory.messages);
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
