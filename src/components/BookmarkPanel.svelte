<script lang="ts">
  import type { Bookmark, TrajectorySource, Annotation, RenderId } from "../types/workspace";
  import { parseRenderId } from "../lib/workspace";
  import { findOrderedItem } from "../lib/order";

  interface Props {
    annotations: Annotation[];
    activeAnnotation: Annotation | null;
    bookmarks: Bookmark[];
    sources: TrajectorySource[];
    selectedRenderId: RenderId | null;
    onSelectAnnotation: (id: number) => void;
    onCreateAnnotation: (name: string) => void;
    onDeleteAnnotation: (id: number) => void;
    onNavigate: (renderId: RenderId) => void;
    onUpdateComment: (id: number, comment: string) => void;
    onDelete: (id: number) => void;
  }

  let {
    annotations,
    activeAnnotation,
    bookmarks,
    sources,
    selectedRenderId,
    onSelectAnnotation,
    onCreateAnnotation,
    onDeleteAnnotation,
    onNavigate,
    onUpdateComment,
    onDelete,
  }: Props = $props();

  let editingId = $state<number | null>(null);
  let editText = $state("");
  let showCreate = $state(false);
  let newName = $state("");
  let annotationsOpen = $state(true);
  let bookmarksOpen = $state(true);

  function getNodeLabel(bookmark: Bookmark): string {
    const { sourceId, nodeId } = parseRenderId(bookmark.node_id);
    const source = sources.find((s) => s.id === sourceId);
    if (!source || !source.trajectory) return nodeId;
    const item = findOrderedItem(source.trajectory, nodeId);
    if (!item) return nodeId;
    if (item.type === "message") return `${item.message.role} (${nodeId})`;
    return `${item.block.kind} (${nodeId})`;
  }

  function startEdit(bookmark: Bookmark) {
    editingId = bookmark.id;
    editText = bookmark.comment ?? "";
  }

  function saveEdit(id: number) {
    onUpdateComment(id, editText);
    editingId = null;
  }

  function handleDeleteBookmark(id: number) {
    if (confirm("Delete this bookmark?")) {
      onDelete(id);
    }
  }

  function handleCreate() {
    const name = newName.trim();
    if (name) {
      onCreateAnnotation(name);
      newName = "";
      showCreate = false;
    }
  }

  function handleDeleteAnnotation(id: number, e: Event) {
    e.stopPropagation();
    if (confirm("Delete this annotation?")) {
      onDeleteAnnotation(id);
    }
  }
</script>

<div class="bookmark-panel">
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="panel-header" onclick={() => (annotationsOpen = !annotationsOpen)}>
    <span class="panel-title">Annotations</span>
    <span class="header-right">
      <span class="count">{annotations.length}</span>
      <span class="fold-caret">{annotationsOpen ? "▼" : "▶"}</span>
    </span>
  </div>

  {#if annotationsOpen}
  <div class="annotation-list">
    {#each annotations as ann}
      <div
        class="annotation-item"
        class:active={activeAnnotation?.id === ann.id}
        role="button"
        tabindex="0"
        onclick={() => onSelectAnnotation(ann.id)}
        onkeydown={(e) => e.key === "Enter" && onSelectAnnotation(ann.id)}
      >
        <span class="annotation-name">{ann.name}</span>
        <button class="delete-btn" onclick={(e) => handleDeleteAnnotation(ann.id, e)}>×</button>
      </div>
    {/each}

    {#if showCreate}
      <div class="create-row">
        <input
          type="text"
          placeholder="Annotation name"
          bind:value={newName}
          onkeydown={(e) => e.key === "Enter" && handleCreate()}
        />
        <button onclick={handleCreate}>Create</button>
        <button class="cancel-btn" onclick={() => { showCreate = false; newName = ""; }}>Cancel</button>
      </div>
    {:else}
      <button class="create-btn" onclick={() => (showCreate = true)}>+ New Annotation</button>
    {/if}
  </div>
  {/if}

  {#if activeAnnotation}
    <div class="bookmarks-section">
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="bookmarks-header" onclick={() => (bookmarksOpen = !bookmarksOpen)}>
        <span class="bookmarks-title">Bookmarks</span>
        <span class="header-right">
          <span class="count">{bookmarks.length}</span>
          <span class="fold-caret">{bookmarksOpen ? "▼" : "▶"}</span>
        </span>
      </div>
      {#if bookmarksOpen}
      <div class="bookmark-list">
        {#if bookmarks.length === 0}
          <div class="empty">No bookmarks yet. Select a node and add a bookmark.</div>
        {:else}
          {#each bookmarks as bookmark}
            {@const isSelected = selectedRenderId === bookmark.node_id}
            <div class="bookmark-item" class:selected={isSelected}>
              <button class="navigate-btn" onclick={() => onNavigate(bookmark.node_id)}>
                <span class="node-label">{getNodeLabel(bookmark)}</span>
              </button>

              {#if editingId === bookmark.id}
                <textarea
                  bind:value={editText}
                  onkeydown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); saveEdit(bookmark.id); } }}
                  rows={2}
                ></textarea>
                <div class="edit-actions">
                  <button onclick={() => saveEdit(bookmark.id)}>Save</button>
                  <button onclick={() => (editingId = null)}>Cancel</button>
                </div>
              {:else}
                {#if bookmark.comment}
                  <div class="comment">{bookmark.comment}</div>
                {/if}
                <div class="actions">
                  <button onclick={() => startEdit(bookmark)}>Edit</button>
                  <button onclick={() => handleDeleteBookmark(bookmark.id)}>Delete</button>
                </div>
              {/if}
            </div>
          {/each}
        {/if}
      </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .bookmark-panel {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    background: #ffffff;
    overflow: hidden;
  }

  .panel-header,
  .bookmarks-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px;
    border-bottom: 1px solid #e9ecef;
    flex-shrink: 0;
    cursor: pointer;
    user-select: none;
  }

  .panel-header:hover,
  .bookmarks-header:hover {
    background: #f8f9fa;
  }

  .panel-title,
  .bookmarks-title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: #6c757d;
  }

  .header-right {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .count {
    font-size: 11px;
    color: #adb5bd;
    background: #f1f3f5;
    padding: 2px 6px;
    border-radius: 10px;
  }

  .fold-caret {
    font-size: 10px;
    color: #adb5bd;
    width: 14px;
    text-align: center;
  }

  .annotation-list {
    display: flex;
    flex-direction: column;
    padding: 8px;
    gap: 4px;
    flex-shrink: 0;
    max-height: 40%;
    overflow-y: auto;
    border-bottom: 1px solid #e9ecef;
  }

  .annotation-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 8px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 13px;
    background: none;
    border: none;
    text-align: left;
    color: #495057;
  }

  .annotation-item:hover {
    background: #f1f3f5;
  }

  .annotation-item.active {
    background: #e7f5ff;
    color: #1864ab;
    font-weight: 500;
  }

  .annotation-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .delete-btn {
    background: none;
    border: none;
    color: #adb5bd;
    cursor: pointer;
    font-size: 16px;
    line-height: 1;
    padding: 0 4px;
  }

  .delete-btn:hover {
    color: #e03131;
  }

  .create-btn {
    background: none;
    border: none;
    color: #1864ab;
    cursor: pointer;
    font-size: 13px;
    padding: 6px 8px;
    text-align: left;
    border-radius: 4px;
  }

  .create-btn:hover {
    background: #e7f5ff;
  }

  .create-row {
    display: flex;
    gap: 4px;
    padding: 4px;
  }

  .create-row input {
    flex: 1;
    border: 1px solid #ced4da;
    border-radius: 4px;
    padding: 4px 8px;
    font-size: 13px;
  }

  .create-row button {
    background: #1864ab;
    color: #fff;
    border: none;
    border-radius: 4px;
    padding: 4px 10px;
    cursor: pointer;
    font-size: 13px;
  }

  .bookmarks-section {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .create-row button.cancel-btn {
    background: #f1f3f5;
    color: #495057;
    border: 1px solid #ced4da;
  }

  .bookmark-list {
    flex: 1;
    overflow-y: auto;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .empty {
    color: #adb5bd;
    font-size: 12px;
    text-align: center;
    padding: 20px 8px;
  }

  .bookmark-item {
    border: 1px solid #e9ecef;
    border-radius: 6px;
    padding: 8px;
    background: #f8f9fa;
    transition: background 0.15s;
  }

  .bookmark-item.selected {
    background: #e7f5ff;
    border-color: #74c0fc;
  }

  .navigate-btn {
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    text-align: left;
    width: 100%;
  }

  .node-label {
    font-size: 12px;
    font-weight: 600;
    color: #343a40;
  }

  .comment {
    font-size: 12px;
    color: #495057;
    margin-top: 4px;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .actions {
    display: flex;
    gap: 6px;
    margin-top: 6px;
  }

  .actions button {
    background: none;
    border: none;
    color: #868e96;
    cursor: pointer;
    font-size: 11px;
    padding: 2px 6px;
    border-radius: 3px;
  }

  .actions button:hover {
    background: #e9ecef;
    color: #495057;
  }

  textarea {
    width: 100%;
    border: 1px solid #ced4da;
    border-radius: 4px;
    padding: 6px;
    font-size: 12px;
    resize: vertical;
    margin-top: 4px;
  }

  .edit-actions {
    display: flex;
    gap: 6px;
    margin-top: 4px;
  }

  .edit-actions button {
    background: #f1f3f5;
    border: 1px solid #ced4da;
    border-radius: 4px;
    padding: 4px 10px;
    cursor: pointer;
    font-size: 12px;
  }

  .edit-actions button:first-child {
    background: #1864ab;
    color: #fff;
    border-color: #1864ab;
  }
</style>
