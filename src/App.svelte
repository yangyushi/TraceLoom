<script lang="ts">
  import {
    pickFolder,
    listJsonlFiles,
    loadTrajectory,
    readFileText,
    exportWorkspace,
    importWorkspace,
    pickJsonlFile,
    pickWorkspaceFile,
    pickSavePath,
    addRecentWorkspace,
  } from "./lib/api";
  import type { WorkspaceState, TrajectorySource, Annotation, Bookmark, RenderId, WorkspaceFile, WorkspaceFileSource, WorkspaceFileAnnotation, WorkspaceFileBookmark } from "./types/workspace";
  import { parseRenderId, findSourceById } from "./lib/workspace";
  import { findOrderedItem } from "./lib/order";
  import DotsRenderer from "./components/DotsRenderer.svelte";
  import BricksRenderer from "./components/BricksRenderer.svelte";
  import NodeDetail from "./components/NodeDetail.svelte";
  import WorkspaceToolbar from "./components/WorkspaceToolbar.svelte";
  import BookmarkPanel from "./components/BookmarkPanel.svelte";

  let workspaceState = $state<WorkspaceState | null>(null);
  let hasUnsavedChanges = $state(false);
  let selectedRenderId = $state<RenderId | null>(null);
  let selectedSourceId = $state<number | null>(null);
  let theme = $state<"dots" | "bricks">("dots");
  let error = $state<string | null>(null);
  let showBookmarkPanel = $state(false);

  let leftWidth = $state(220);
  let rightWidth = $state(420);
  let isResizingLeft = $state(false);
  let isResizingRight = $state(false);
  let resizeFrame: number | null = null;
  let pendingResizeX = 0;
  let resizeAppWidth = 0;

  let contextMenu = $state<{ x: number; y: number; sourceId: number; state: string; displayName: string; filePath: string | null } | null>(null);

  // ID counters for in-memory entities
  let nextSourceId = 1;
  let nextAnnotationId = 1;
  let nextBookmarkId = 1;

  // Derived: only loaded sources for canvas rendering
  const loadedSources = $derived.by(() => {
    if (!workspaceState) return [];
    return workspaceState.sources.filter((s) => s.visibility_state === "loaded");
  });

  // Derived: selected item for inspector
  const selectedItem = $derived.by(() => {
    if (!workspaceState || !selectedRenderId) return null;
    const { sourceId, nodeId } = parseRenderId(selectedRenderId);
    const source = findSourceById(workspaceState.sources, sourceId);
    if (!source || !source.trajectory) return null;
    return findOrderedItem(source.trajectory, nodeId);
  });

  // Derived: parse issues across all loaded sources
  const parseIssues = $derived.by(() => {
    if (!workspaceState) return [];
    return workspaceState.sources
      .filter((s) => s.trajectory)
      .flatMap((s) => [
        ...s.trajectory!.warnings.map((w) => `[${s.display_name}] ${w}`),
        ...s.trajectory!.orphans.map((id) => `[${s.display_name}] orphan message: ${id}`),
      ]);
  });

  // Derived: annotations for selected source
  const annotationsForSelectedSource = $derived.by(() => {
    if (!workspaceState || selectedSourceId == null) return [];
    return workspaceState.annotations.filter((a) => a.trajectory_source_id === selectedSourceId);
  });

  // Derived: bookmarks for active annotation
  const bookmarksForActiveAnnotation = $derived.by(() => {
    if (!workspaceState || !workspaceState.activeAnnotation) return [];
    return workspaceState.bookmarks.filter((b) => b.annotation_id === workspaceState!.activeAnnotation!.id);
  });

  // Warn on quit if unsaved changes
  function handleBeforeUnload(e: BeforeUnloadEvent) {
    if (hasUnsavedChanges) {
      e.preventDefault();
    }
  }

  function initApp() {
    workspaceState = {
      filePath: null,
      workspace: { name: "Untitled", created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
      sources: [],
      annotations: [],
      bookmarks: [],
      activeAnnotation: null,
    };
    nextSourceId = 1;
    nextAnnotationId = 1;
    nextBookmarkId = 1;
    hasUnsavedChanges = false;
  }

  function createSource(path: string): TrajectorySource {
    const displayName = path.split("/").pop() ?? path;
    const id = nextSourceId++;
    return {
      id,
      display_name: displayName,
      source_type: "imported_file",
      file_path: path,
      trajectory: null,
      color_key: `source_${(id - 1) % 8}`,
      sort_order: id - 1,
      visibility_state: "unloaded",
    };
  }

  async function handleAddFolder() {
    if (!workspaceState) return;
    error = null;
    try {
      const folder = await pickFolder();
      if (!folder) return;
      const files = await listJsonlFiles(folder);
      const newSources = files.map((path) => createSource(path));
      workspaceState.sources = [...workspaceState.sources, ...newSources];
      hasUnsavedChanges = true;
    } catch (e) {
      error = String(e);
    }
  }

  async function handleAddFileWithPath() {
    if (!workspaceState) return;
    error = null;
    try {
      const path = await pickJsonlFile();
      if (!path) return;
      const newSource = createSource(path);
      workspaceState.sources = [...workspaceState.sources, newSource];
      hasUnsavedChanges = true;
    } catch (e) {
      error = String(e);
    }
  }

  async function handleUpdateSourceState(sourceId: number, state: "unloaded" | "loaded" | "hidden") {
    if (!workspaceState) return;
    const source = workspaceState.sources.find((s) => s.id === sourceId);
    if (!source) return;

    if (state === "loaded" && source.visibility_state !== "loaded") {
      try {
        const trajectory = await loadTrajectory(source.file_path!);
        source.trajectory = trajectory;
      } catch (e) {
        error = String(e);
        return;
      }
    }
    if (state === "unloaded") {
      source.trajectory = null;
    }
    source.visibility_state = state;
    workspaceState = { ...workspaceState };
    hasUnsavedChanges = true;
  }

  function handleRemoveSource(sourceId: number) {
    if (!workspaceState) return;
    workspaceState.sources = workspaceState.sources.filter((s) => s.id !== sourceId);
    workspaceState.annotations = workspaceState.annotations.filter((a) => a.trajectory_source_id !== sourceId);
    const annotationIds = new Set(workspaceState.annotations.map((a) => a.id));
    workspaceState.bookmarks = workspaceState.bookmarks.filter((b) => annotationIds.has(b.annotation_id));
    if (selectedSourceId === sourceId) {
      selectedSourceId = null;
      selectedRenderId = null;
    }
    workspaceState = { ...workspaceState };
    hasUnsavedChanges = true;
  }

  function handleSelect(renderId: string) {
    selectedRenderId = renderId;
    const parsed = parseRenderId(renderId);
    selectedSourceId = parsed.sourceId;
  }

  function handleSelectSource(sourceId: number) {
    selectedSourceId = sourceId;
    if (selectedRenderId) {
      const parsed = parseRenderId(selectedRenderId);
      if (parsed.sourceId !== sourceId) {
        selectedRenderId = null;
      }
    }
    // Activate first annotation for this source (or create default)
    if (workspaceState) {
      const anns = workspaceState.annotations.filter((a) => a.trajectory_source_id === sourceId);
      if (anns.length === 0) {
        const newAnn: Annotation = {
          id: nextAnnotationId++,
          name: "Default",
          trajectory_source_id: sourceId,
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
        };
        workspaceState.annotations = [...workspaceState.annotations, newAnn];
        workspaceState.activeAnnotation = newAnn;
      } else {
        workspaceState.activeAnnotation = anns[0];
      }
    }
  }

  function handleRenameWorkspace(name: string) {
    if (!workspaceState) return;
    const trimmed = name.trim();
    if (!trimmed || trimmed === workspaceState.workspace.name) return;
    workspaceState.workspace.name = trimmed;
    workspaceState.workspace.updated_at = new Date().toISOString();
    workspaceState = { ...workspaceState };
    hasUnsavedChanges = true;
  }

  function handleCreateAnnotation(name: string) {
    if (!workspaceState || selectedSourceId == null) return;
    const newAnn: Annotation = {
      id: nextAnnotationId++,
      name,
      trajectory_source_id: selectedSourceId,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
    workspaceState.annotations = [...workspaceState.annotations, newAnn];
    workspaceState.activeAnnotation = newAnn;
    hasUnsavedChanges = true;
  }

  function handleSelectAnnotation(id: number) {
    if (!workspaceState) return;
    const ann = workspaceState.annotations.find((a) => a.id === id);
    if (ann) {
      workspaceState.activeAnnotation = ann;
    }
  }

  function handleDeleteAnnotation(id: number) {
    if (!workspaceState) return;
    workspaceState.annotations = workspaceState.annotations.filter((a) => a.id !== id);
    workspaceState.bookmarks = workspaceState.bookmarks.filter((b) => b.annotation_id !== id);
    if (workspaceState.activeAnnotation?.id === id) {
      workspaceState.activeAnnotation = null;
    }
    hasUnsavedChanges = true;
  }

  function handleAddBookmark(comment: string) {
    if (!selectedRenderId) {
      error = "No node selected.";
      return;
    }
    if (!workspaceState?.activeAnnotation) {
      error = "No annotation is active. Select a source to auto-create one.";
      return;
    }
    const bm: Bookmark = {
      id: nextBookmarkId++,
      annotation_id: workspaceState.activeAnnotation.id,
      node_id: selectedRenderId,
      comment: comment || null,
      created_at: new Date().toISOString(),
    };
    workspaceState.bookmarks = [...workspaceState.bookmarks, bm];
    hasUnsavedChanges = true;
  }

  function handleQuickAddBookmark(renderId: RenderId) {
    if (!workspaceState?.activeAnnotation) {
      error = "No annotation is active. Select a source to auto-create one.";
      return;
    }
    const bm: Bookmark = {
      id: nextBookmarkId++,
      annotation_id: workspaceState.activeAnnotation.id,
      node_id: renderId,
      comment: null,
      created_at: new Date().toISOString(),
    };
    workspaceState.bookmarks = [...workspaceState.bookmarks, bm];
    hasUnsavedChanges = true;
  }

  function handleRemoveBookmark(id: number) {
    if (!workspaceState) return;
    workspaceState.bookmarks = workspaceState.bookmarks.filter((b) => b.id !== id);
    hasUnsavedChanges = true;
  }

  function handleUpdateBookmarkComment(id: number, comment: string) {
    if (!workspaceState) return;
    workspaceState.bookmarks = workspaceState.bookmarks.map((b) =>
      b.id === id ? { ...b, comment: comment || null } : b
    );
    hasUnsavedChanges = true;
  }

  function handleNavigateToBookmark(renderId: RenderId) {
    selectedRenderId = renderId;
  }

  function serializeWorkspace(state: WorkspaceState): string {
    const fileSources: WorkspaceFileSource[] = state.sources.map((s) => ({
      file_path: s.file_path ?? "",
      display_name: s.display_name,
      color_key: s.color_key,
      sort_order: s.sort_order,
      visibility_state: s.visibility_state,
    }));

    const fileAnnotations: WorkspaceFileAnnotation[] = state.annotations.map((a) => {
      const sourceIndex = state.sources.findIndex((s) => s.id === a.trajectory_source_id);
      return {
        name: a.name,
        trajectory_source_index: sourceIndex >= 0 ? sourceIndex : 0,
        created_at: a.created_at,
        updated_at: a.updated_at,
      };
    });

    const fileBookmarks: WorkspaceFileBookmark[] = state.bookmarks.map((b) => {
      const annIndex = state.annotations.findIndex((a) => a.id === b.annotation_id);
      return {
        annotation_index: annIndex >= 0 ? annIndex : 0,
        node_id: b.node_id,
        comment: b.comment,
        created_at: b.created_at,
      };
    });

    const file: WorkspaceFile = {
      version: 1,
      workspace: state.workspace,
      sources: fileSources,
      annotations: fileAnnotations,
      bookmarks: fileBookmarks,
    };

    return JSON.stringify(file, null, 2);
  }

  async function hydrateWorkspace(file: WorkspaceFile, filePath: string | null): Promise<WorkspaceState> {
    const sources = await Promise.all(file.sources.map(async (fs, i): Promise<TrajectorySource> => {
      let trajectory = null;
      if (fs.visibility_state === "loaded") {
        try {
          trajectory = await loadTrajectory(fs.file_path);
        } catch (e) {
          console.error("Failed to load trajectory:", fs.file_path, e);
        }
      }
      return {
        id: i + 1,
        display_name: fs.display_name,
        source_type: "imported_file",
        file_path: fs.file_path,
        trajectory,
        color_key: fs.color_key,
        sort_order: fs.sort_order,
        visibility_state: fs.visibility_state,
      };
    }));

    const annotations: Annotation[] = [];
    for (let i = 0; i < file.annotations.length; i++) {
      const fa = file.annotations[i];
      const sourceId = sources[fa.trajectory_source_index]?.id ?? 0;
      annotations.push({
        id: i + 1,
        name: fa.name,
        trajectory_source_id: sourceId,
        created_at: fa.created_at,
        updated_at: fa.updated_at,
      });
    }

    const bookmarks: Bookmark[] = [];
    for (let i = 0; i < file.bookmarks.length; i++) {
      const fb = file.bookmarks[i];
      const annId = annotations[fb.annotation_index]?.id ?? 0;
      bookmarks.push({
        id: i + 1,
        annotation_id: annId,
        node_id: fb.node_id,
        comment: fb.comment,
        created_at: fb.created_at,
      });
    }

    return {
      filePath,
      workspace: file.workspace,
      sources,
      annotations,
      bookmarks,
      activeAnnotation: annotations[0] ?? null,
    };
  }

  async function handleSaveWorkspace() {
    if (!workspaceState) return;
    try {
      if (workspaceState.filePath) {
        workspaceState.workspace.updated_at = new Date().toISOString();
        const json = serializeWorkspace(workspaceState);
        await exportWorkspace(workspaceState.filePath, json);
        workspaceState = { ...workspaceState };
        hasUnsavedChanges = false;
      } else {
        await handleSaveWorkspaceAs();
      }
    } catch (e) {
      error = String(e);
    }
  }

  async function handleSaveWorkspaceAs() {
    if (!workspaceState) return;
    try {
      const path = await pickSavePath();
      if (!path) return;
      workspaceState.workspace.updated_at = new Date().toISOString();
      const json = serializeWorkspace(workspaceState);
      await exportWorkspace(path, json);
      workspaceState.filePath = path;
      workspaceState = { ...workspaceState };
      hasUnsavedChanges = false;
      await addRecentWorkspace(path, workspaceState.workspace.name);
    } catch (e) {
      error = String(e);
    }
  }

  async function handleOpenWorkspace() {
    try {
      const path = await pickWorkspaceFile();
      if (!path) return;
      const file = await importWorkspace(path);
      const state = await hydrateWorkspace(file, path);
      workspaceState = state;
      selectedRenderId = null;
      selectedSourceId = null;
      hasUnsavedChanges = false;
      nextSourceId = state.sources.length + 1;
      nextAnnotationId = state.annotations.length + 1;
      nextBookmarkId = state.bookmarks.length + 1;
      await addRecentWorkspace(path, state.workspace.name);
    } catch (e) {
      error = String(e);
    }
  }

  function handleNewWorkspace() {
    if (hasUnsavedChanges) {
      if (!confirm("You have unsaved changes. Discard them?")) {
        return;
      }
    }
    initApp();
    selectedRenderId = null;
    selectedSourceId = null;
  }

  function handleContextMenu(e: MouseEvent, source: TrajectorySource) {
    e.preventDefault();
    contextMenu = { x: e.clientX, y: e.clientY, sourceId: source.id, state: source.visibility_state, displayName: source.display_name, filePath: source.file_path };
  }

  async function copyToClipboard(text: string) {
    try {
      await navigator.clipboard.writeText(text);
    } catch (e) {
      error = "Failed to copy to clipboard";
    }
  }

  function copyName() {
    if (!contextMenu) return;
    copyToClipboard(contextMenu.displayName);
    closeContextMenu();
  }

  function copyPath() {
    if (!contextMenu || !contextMenu.filePath) return;
    copyToClipboard(contextMenu.filePath);
    closeContextMenu();
  }

  async function copyContent() {
    if (!contextMenu || !contextMenu.filePath) return;
    try {
      const content = await readFileText(contextMenu.filePath);
      await navigator.clipboard.writeText(content);
    } catch (e) {
      error = String(e);
    }
    closeContextMenu();
  }

  function handleDoubleClick(source: TrajectorySource) {
    if (source.visibility_state === "loaded") {
      handleUpdateSourceState(source.id, "unloaded");
    } else {
      handleUpdateSourceState(source.id, "loaded");
    }
  }

  function closeContextMenu() {
    contextMenu = null;
  }

  function startResizeLeft(e: PointerEvent) {
    isResizingLeft = true;
    resizeAppWidth = document.querySelector(".app")?.clientWidth ?? window.innerWidth;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    window.addEventListener("pointermove", onResizeMove);
    window.addEventListener("pointerup", stopResize);
    window.addEventListener("pointercancel", stopResize);
  }

  function startResizeRight(e: PointerEvent) {
    isResizingRight = true;
    resizeAppWidth = document.querySelector(".app")?.clientWidth ?? window.innerWidth;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    window.addEventListener("pointermove", onResizeMove);
    window.addEventListener("pointerup", stopResize);
    window.addEventListener("pointercancel", stopResize);
  }

  function onResizeMove(e: PointerEvent) {
    pendingResizeX = e.clientX;
    if (resizeFrame != null) return;

    resizeFrame = requestAnimationFrame(() => {
      resizeFrame = null;
      applyResize(pendingResizeX);
    });
  }

  function applyResize(clientX: number) {
    if (isResizingLeft) {
      leftWidth = Math.max(140, Math.min(400, clientX));
    }
    if (isResizingRight) {
      rightWidth = Math.max(280, Math.min(600, resizeAppWidth - clientX));
    }
  }

  function stopResize() {
    if (resizeFrame != null) {
      cancelAnimationFrame(resizeFrame);
      resizeFrame = null;
      applyResize(pendingResizeX);
    }
    isResizingLeft = false;
    isResizingRight = false;
    document.body.style.cursor = "";
    document.body.style.userSelect = "";
    window.removeEventListener("pointermove", onResizeMove);
    window.removeEventListener("pointerup", stopResize);
    window.removeEventListener("pointercancel", stopResize);
  }

  initApp();
</script>

<svelte:window onbeforeunload={handleBeforeUnload} />

<main class="app">
  <WorkspaceToolbar
    workspaceName={workspaceState?.workspace.name ?? "Untitled"}
    {hasUnsavedChanges}
    onNewWorkspace={handleNewWorkspace}
    onOpenWorkspace={handleOpenWorkspace}
    onSaveWorkspace={handleSaveWorkspace}
    onSaveWorkspaceAs={handleSaveWorkspaceAs}
    onAddFile={handleAddFileWithPath}
    onAddFolder={handleAddFolder}
    onRenameWorkspace={handleRenameWorkspace}
    {showBookmarkPanel}
    onToggleBookmarkPanel={() => (showBookmarkPanel = !showBookmarkPanel)}
    {theme}
    onSetTheme={(t) => (theme = t)}
  />

  {#if error}
    <div class="error">{error}</div>
  {/if}

  {#if parseIssues.length > 0}
    <details class="parse-issues">
      <summary>{parseIssues.length} parser issue{parseIssues.length > 1 ? 's' : ''}</summary>
      <ul>
        {#each parseIssues as issue}
          <li>{issue}</li>
        {/each}
      </ul>
    </details>
  {/if}

  <div class="workspace">
    {#if workspaceState && workspaceState.sources.length > 0}
      <aside class="source-list" style="width: {leftWidth}px;">
        <div class="source-list-header">Sources</div>
        <div class="source-items">
          {#each workspaceState.sources as source}
            <button
              class="source-item"
              class:active={selectedSourceId === source.id}
              class:unloaded={source.visibility_state === "unloaded"}
              class:hidden={source.visibility_state === "hidden"}
              onclick={() => handleSelectSource(source.id)}
              ondblclick={() => handleDoubleClick(source)}
              oncontextmenu={(e) => handleContextMenu(e, source)}
            >
              <span class="source-state" title={source.visibility_state}></span>
              <span class="source-name">{source.display_name}</span>
              <span class="source-count">{source.trajectory ? source.trajectory.messages.length : "—"}</span>
            </button>
          {/each}
        </div>
      </aside>
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <div class="resize-handle" onpointerdown={startResizeLeft} role="separator" tabindex="-1" aria-label="Resize source list"></div>
    {/if}

    <div class="canvas" class:dots-canvas={theme === "dots"} class:bricks-canvas={theme === "bricks"}>
      {#if workspaceState && workspaceState.sources.length > 0}
        {#if theme === "dots"}
          <DotsRenderer
            sources={loadedSources}
            onSelect={handleSelect}
            {selectedRenderId}
            onQuickAddBookmark={handleQuickAddBookmark}
          />
        {:else if selectedSourceId != null}
          {@const brickSource = findSourceById(workspaceState.sources, selectedSourceId)}
          {#if brickSource}
            <BricksRenderer
              source={brickSource}
              onSelect={handleSelect}
              {selectedRenderId}
              onQuickAddBookmark={handleQuickAddBookmark}
            />
          {:else}
            <div class="placeholder">Select a source</div>
          {/if}
        {:else}
          <div class="placeholder">Select a source to view in bricks mode</div>
        {/if}
      {:else}
        <div class="placeholder">Open a file or folder to visualize</div>
      {/if}
    </div>

    {#if selectedItem || showBookmarkPanel}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <div class="resize-handle" onpointerdown={startResizeRight} role="separator" tabindex="-1" aria-label="Resize inspector"></div>
      <aside class="right-panel" style="width: {rightWidth}px;">
        {#if showBookmarkPanel}
          <BookmarkPanel
            annotations={annotationsForSelectedSource}
            activeAnnotation={workspaceState?.activeAnnotation ?? null}
            bookmarks={bookmarksForActiveAnnotation}
            sources={workspaceState?.sources ?? []}
            {selectedRenderId}
            onSelectAnnotation={handleSelectAnnotation}
            onCreateAnnotation={handleCreateAnnotation}
            onDeleteAnnotation={handleDeleteAnnotation}
            onNavigate={handleNavigateToBookmark}
            onUpdateComment={handleUpdateBookmarkComment}
            onDelete={handleRemoveBookmark}
          />
        {/if}
        {#if selectedItem}
          <div class="inspector" class:with-bookmarks={showBookmarkPanel}>
            <NodeDetail
              item={selectedItem}
              onClose={() => (selectedRenderId = null)}
              nodeId={selectedRenderId}
              bookmarks={bookmarksForActiveAnnotation}
              onAddBookmark={handleAddBookmark}
              onRemoveBookmark={handleRemoveBookmark}
            />
          </div>
        {/if}
      </aside>
    {/if}
  </div>

  {#if contextMenu}
    {@const cm = contextMenu}
    <div
      class="context-menu-overlay"
      onclick={closeContextMenu}
      oncontextmenu={(e) => { e.preventDefault(); closeContextMenu(); }}
      role="presentation"
    ></div>
    <div
      class="context-menu"
      style="left: {cm.x}px; top: {cm.y}px;"
    >
      {#if cm.state !== "loaded"}
        <button onclick={() => { handleUpdateSourceState(cm.sourceId, "loaded"); closeContextMenu(); }}>Load</button>
      {/if}
      {#if cm.state !== "unloaded"}
        <button onclick={() => { handleUpdateSourceState(cm.sourceId, "unloaded"); closeContextMenu(); }}>Unload</button>
      {/if}
      {#if cm.state !== "hidden"}
        <button onclick={() => { handleUpdateSourceState(cm.sourceId, "hidden"); closeContextMenu(); }}>Hide</button>
      {/if}
      <div class="context-menu-separator"></div>
      <button onclick={() => copyName()}>Copy name</button>
      {#if cm.filePath}
        <button onclick={() => copyPath()}>Copy full path</button>
        <button onclick={() => copyContent()}>Copy content</button>
      {/if}
      <div class="context-menu-separator"></div>
      <button class="danger" onclick={() => { handleRemoveSource(cm.sourceId); closeContextMenu(); }}>Remove source</button>
    </div>
  {/if}
</main>

<style>
  :global(html),
  :global(body),
  :global(#app) {
    height: 100%;
    overflow: hidden;
  }

  :global(body) {
    margin: 0;
    font-family: system-ui, -apple-system, sans-serif;
    background: #f8f9fa;
    color: #212529;
  }

  .app {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    overflow: hidden;
  }

  .error {
    padding: 8px 16px;
    background: #f8d7da;
    color: #842029;
    border-bottom: 1px solid #f5c2c7;
    flex-shrink: 0;
  }

  .parse-issues {
    padding: 8px 16px;
    background: #fff3bf;
    color: #5f3f00;
    border-bottom: 1px solid #ffe066;
    font-size: 12px;
    flex-shrink: 0;
  }

  .parse-issues summary {
    cursor: pointer;
    font-weight: 600;
  }

  .parse-issues ul {
    margin: 8px 0 0;
    padding-left: 18px;
    max-height: 140px;
    overflow: auto;
  }

  .parse-issues li {
    margin: 3px 0;
    word-break: break-word;
  }

  .workspace {
    display: flex;
    flex: 1;
    overflow: hidden;
    min-width: 0;
    min-height: 0;
  }

  .source-list {
    border-right: 1px solid #dee2e6;
    background: #ffffff;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    flex-shrink: 0;
    min-height: 0;
    user-select: none;
  }

  .source-list-header {
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

  .source-items {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    padding: 4px;
    min-height: 0;
  }

  .source-items::-webkit-scrollbar {
    width: 8px;
  }

  .source-items::-webkit-scrollbar-track {
    background: #f1f3f5;
    border-radius: 4px;
  }

  .source-items::-webkit-scrollbar-thumb {
    background: #ced4da;
    border-radius: 4px;
  }

  .source-items::-webkit-scrollbar-thumb:hover {
    background: #adb5bd;
  }

  .source-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
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
    flex-shrink: 0;
  }

  .source-item:hover {
    background: #f1f3f5;
  }

  .source-item.active {
    background: #e7f5ff;
    color: #1864ab;
    font-weight: 500;
  }

  .source-count {
    color: #adb5bd;
    font-size: 11px;
  }

  .source-state {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    margin-right: 6px;
    flex-shrink: 0;
    background: #51cf66;
  }

  .source-item.unloaded .source-state {
    background: #adb5bd;
  }

  .source-item.hidden .source-state {
    background: #ff922b;
  }

  .source-item.unloaded,
  .source-item.hidden {
    opacity: 0.7;
  }

  .resize-handle {
    width: 5px;
    cursor: col-resize;
    background: #e9ecef;
    transition: background 0.15s;
    flex-shrink: 0;
    touch-action: none;
  }

  .resize-handle:hover {
    background: #adb5bd;
  }

  .canvas {
    flex: 1;
    position: relative;
    background: #f8f9fa;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .canvas.bricks-canvas {
    overflow: auto;
  }

  .canvas.dots-canvas {
    overflow: hidden;
  }

  .placeholder {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #adb5bd;
    font-size: 18px;
  }

  .right-panel {
    border-left: 1px solid #dee2e6;
    background: #ffffff;
    overflow: auto;
    flex-shrink: 0;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .inspector {
    flex: 1;
    overflow: auto;
    min-height: 0;
  }

  .inspector.with-bookmarks {
    border-top: 1px solid #dee2e6;
  }

  .context-menu-overlay {
    position: fixed;
    inset: 0;
    z-index: 9998;
  }

  .context-menu {
    position: fixed;
    z-index: 9999;
    background: #ffffff;
    border: 1px solid #dee2e6;
    border-radius: 6px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
    display: flex;
    flex-direction: column;
    min-width: 160px;
    padding: 4px;
  }

  .context-menu button {
    background: none;
    border: none;
    padding: 8px 12px;
    text-align: left;
    font-size: 13px;
    color: #212529;
    cursor: pointer;
    border-radius: 4px;
  }

  .context-menu button:hover {
    background: #f1f3f5;
  }

  .context-menu button.danger {
    color: #c92a2a;
  }

  .context-menu button.danger:hover {
    background: #fff5f5;
  }

  .context-menu-separator {
    height: 1px;
    background: #e9ecef;
    margin: 4px 0;
  }
</style>
