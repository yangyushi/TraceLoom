<script lang="ts">
  import { pickFolder, listJsonlFiles, loadTrajectory, pickAndLoadTrajectory } from "./lib/api";
  import type { Trajectory, Node } from "./types/ir";
  import DotsRenderer from "./components/DotsRenderer.svelte";
  import BricksRenderer from "./components/BricksRenderer.svelte";
  import NodeDetail from "./components/NodeDetail.svelte";

  let trajectory = $state<Trajectory | null>(null);
  let selectedNode = $state<Node | null>(null);
  let theme = $state<"dots" | "bricks">("dots");
  let error = $state<string | null>(null);

  let files = $state<string[]>([]);
  let currentFile = $state<string | null>(null);
  let folderPath = $state<string | null>(null);

  let leftWidth = $state(220);
  let rightWidth = $state(420);
  let isResizingLeft = $state(false);
  let isResizingRight = $state(false);

  async function handleOpenFolder() {
    error = null;
    try {
      const folder = await pickFolder();
      if (!folder) return;
      folderPath = folder;
      files = await listJsonlFiles(folder);
      if (files.length > 0) {
        await loadFile(files[0]);
      } else {
        trajectory = null;
        currentFile = null;
        selectedNode = null;
      }
    } catch (e) {
      error = String(e);
    }
  }

  async function handleOpenFile() {
    error = null;
    try {
      const result = await pickAndLoadTrajectory();
      if (result) {
        trajectory = result;
        selectedNode = null;
        currentFile = null;
        folderPath = null;
        files = [];
      }
    } catch (e) {
      error = String(e);
    }
  }

  async function loadFile(path: string) {
    error = null;
    try {
      const result = await loadTrajectory(path);
      trajectory = result;
      currentFile = path;
      selectedNode = null;
    } catch (e) {
      error = String(e);
    }
  }

  function handleSelect(node: Node) {
    selectedNode = node;
  }

  function fileName(path: string): string {
    return path.split(/[/\\]/).pop() ?? path;
  }

  function startResizeLeft(e: MouseEvent) {
    isResizingLeft = true;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    window.addEventListener("mousemove", onResizeMove);
    window.addEventListener("mouseup", stopResize);
  }

  function startResizeRight(e: MouseEvent) {
    isResizingRight = true;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    window.addEventListener("mousemove", onResizeMove);
    window.addEventListener("mouseup", stopResize);
  }

  function onResizeMove(e: MouseEvent) {
    if (isResizingLeft) {
      leftWidth = Math.max(140, Math.min(400, e.clientX));
    }
    if (isResizingRight) {
      const appWidth = document.querySelector(".app")?.clientWidth ?? window.innerWidth;
      rightWidth = Math.max(280, Math.min(600, appWidth - e.clientX));
    }
  }

  function stopResize() {
    isResizingLeft = false;
    isResizingRight = false;
    document.body.style.cursor = "";
    document.body.style.userSelect = "";
    window.removeEventListener("mousemove", onResizeMove);
    window.removeEventListener("mouseup", stopResize);
  }
</script>

<main class="app">
  <header class="toolbar">
    <div class="open-buttons">
      <button onclick={handleOpenFolder}>Open Folder</button>
      <button onclick={handleOpenFile}>Open File</button>
    </div>
    <div class="theme-toggle">
      <button class:active={theme === "dots"} onclick={() => (theme = "dots")}>Dots</button>
      <button class:active={theme === "bricks"} onclick={() => (theme = "bricks")}>Bricks</button>
    </div>
    {#if trajectory}
      <span class="meta">{trajectory.nodes.length} nodes</span>
    {/if}
  </header>

  {#if error}
    <div class="error">{error}</div>
  {/if}

  <div class="workspace">
    {#if files.length > 0}
      <aside class="file-list" style="width: {leftWidth}px;">
        <div class="file-list-header">{fileName(folderPath ?? "")}</div>
        <div class="file-items">
          {#each files as path}
            <button
              class="file-item"
              class:active={currentFile === path}
              onclick={() => loadFile(path)}
            >
              {fileName(path)}
            </button>
          {/each}
        </div>
      </aside>
      <div class="resize-handle" onmousedown={startResizeLeft} role="separator" tabindex="-1" aria-label="Resize file list"></div>
    {/if}

    <div class="canvas">
      {#if trajectory}
        {#if theme === "dots"}
          <DotsRenderer {trajectory} onSelect={handleSelect} {selectedNode} />
        {:else}
          <BricksRenderer {trajectory} onSelect={handleSelect} {selectedNode} />
        {/if}
      {:else}
        <div class="placeholder">Open a folder or file to visualize</div>
      {/if}
    </div>

    {#if selectedNode}
      <div class="resize-handle" onmousedown={startResizeRight} role="separator" tabindex="-1" aria-label="Resize inspector"></div>
      <aside class="inspector" style="width: {rightWidth}px;">
        <NodeDetail node={selectedNode} onClose={() => (selectedNode = null)} />
      </aside>
    {/if}
  </div>
</main>

<style>
  :global(body) {
    margin: 0;
    font-family: system-ui, -apple-system, sans-serif;
    background: #f8f9fa;
    color: #212529;
    height: 100vh;
    overflow: hidden;
  }

  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 16px;
    background: #ffffff;
    border-bottom: 1px solid #dee2e6;
    flex-shrink: 0;
  }

  .open-buttons {
    display: flex;
    gap: 8px;
  }

  .toolbar button {
    background: #f1f3f5;
    color: #212529;
    border: 1px solid #ced4da;
    padding: 6px 14px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 13px;
  }

  .toolbar button:hover {
    background: #e9ecef;
  }

  .toolbar button.active {
    background: #dee2e6;
    border-color: #adb5bd;
    font-weight: 600;
  }

  .theme-toggle {
    display: flex;
    gap: 4px;
  }

  .meta {
    margin-left: auto;
    color: #6c757d;
    font-size: 12px;
  }

  .error {
    padding: 8px 16px;
    background: #f8d7da;
    color: #842029;
    border-bottom: 1px solid #f5c2c7;
    flex-shrink: 0;
  }

  .workspace {
    display: flex;
    flex: 1;
    overflow: hidden;
  }

  .file-list {
    border-right: 1px solid #dee2e6;
    background: #ffffff;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    flex-shrink: 0;
  }

  .file-list-header {
    padding: 10px 12px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: #6c757d;
    border-bottom: 1px solid #e9ecef;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 0;
  }

  .file-items {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    padding: 4px;
  }

  .file-items::-webkit-scrollbar {
    width: 8px;
  }

  .file-items::-webkit-scrollbar-track {
    background: #f1f3f5;
    border-radius: 4px;
  }

  .file-items::-webkit-scrollbar-thumb {
    background: #ced4da;
    border-radius: 4px;
  }

  .file-items::-webkit-scrollbar-thumb:hover {
    background: #adb5bd;
  }

  .file-item {
    text-align: left;
    background: none;
    border: none;
    padding: 6px 8px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 12px;
    color: #495057;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .file-item:hover {
    background: #f1f3f5;
  }

  .file-item.active {
    background: #e7f5ff;
    color: #1864ab;
    font-weight: 500;
  }

  .resize-handle {
    width: 5px;
    cursor: col-resize;
    background: #e9ecef;
    transition: background 0.15s;
    flex-shrink: 0;
  }

  .resize-handle:hover {
    background: #adb5bd;
  }

  .canvas {
    flex: 1;
    overflow: auto;
    position: relative;
    background: #f8f9fa;
  }

  .placeholder {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #adb5bd;
    font-size: 18px;
  }

  .inspector {
    border-left: 1px solid #dee2e6;
    background: #ffffff;
    overflow: auto;
    flex-shrink: 0;
  }
</style>
