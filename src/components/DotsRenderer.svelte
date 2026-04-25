<script lang="ts">
  import type { Trajectory, Node } from "../types/ir";
  import { getFillColor, getStrokeColor } from "../lib/colors";
  import cytoscape from "cytoscape";
  import dagre from "cytoscape-dagre";
  import { onDestroy } from "svelte";

  cytoscape.use(dagre);

  interface Props {
    trajectory: Trajectory;
    onSelect: (node: Node) => void;
    selectedNode: Node | null;
  }

  let { trajectory, onSelect, selectedNode }: Props = $props();

  let container: HTMLDivElement | null = $state(null);
  let cy: cytoscape.Core | null = null;

  const msgById = $derived(new Map(trajectory.messages.map((m) => [m.id, m])));

  function isMessageNode(node: Node): boolean {
    return msgById.has(node.id);
  }

  function buildElements(): cytoscape.ElementDefinition[] {
    const elements: cytoscape.ElementDefinition[] = [];

    for (const node of trajectory.nodes) {
      const isMsg = isMessageNode(node);
      const label = isMsg ? node.role[0] : node.kind;
      const color = getFillColor(isMsg ? node.role[0] : node.kind);
      const stroke = getStrokeColor(isMsg ? node.role[0] : node.kind);

      elements.push({
        data: {
          id: node.id,
          label,
          color,
          stroke,
          isMessage: isMsg,
          _node: node,
        },
      });
    }

    for (const edge of trajectory.edges) {
      elements.push({
        data: {
          id: `${edge.from}-${edge.to}`,
          source: edge.from,
          target: edge.to,
        },
      });
    }

    return elements;
  }

  function createCy() {
    if (!container) return;

    // Make sure the container has non-zero dimensions before init
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
            width: (ele: any) => (ele.data("isMessage") ? "36px" : "20px"),
            height: (ele: any) => (ele.data("isMessage") ? "36px" : "20px"),
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
          selector: "edge",
          style: {
            width: 1.5,
            "line-color": "#ced4da",
            "target-arrow-color": "#ced4da",
            "target-arrow-shape": "triangle",
            "curve-style": "bezier",
            "arrow-scale": 0.7,
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
        name: "dagre",
        rankDir: "TB",
        nodeSep: 80,
        rankSep: 100,
        padding: 24,
      } as any,
      minZoom: 0.15,
      maxZoom: 3,
      wheelSensitivity: 0.3,
    });

    cy.on("tap", "node", (evt: cytoscape.EventObjectNode) => {
      const nodeData = evt.target.data("_node") as Node;
      if (nodeData) onSelect(nodeData);
    });
  }

  function updateSelection() {
    if (!cy) return;
    cy.nodes().unselect();
    const id = selectedNode?.id;
    if (id) {
      const ele = cy.getElementById(id);
      if (ele.length > 0) ele.select();
    }
  }

  // Initialize when container is bound or trajectory changes
  $effect(() => {
    const c = container;
    const t = trajectory;
    void t; // establish reactivity dependency
    if (c) {
      createCy();
    }
  });

  // Update selection highlight when selectedNode changes
  $effect(() => {
    const id = selectedNode?.id;
    void id;
    updateSelection();
  });

  onDestroy(() => {
    if (cy) {
      cy.destroy();
      cy = null;
    }
  });
</script>

<div class="dots-container" bind:this={container}></div>

<style>
  .dots-container {
    width: 100%;
    height: 100%;
    background: #f8f9fa;
  }
</style>
