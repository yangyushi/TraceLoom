<script lang="ts">
  import type { Annotation } from "../types/workspace";

  interface Props {
    annotations: Annotation[];
    activeAnnotation: Annotation | null;
    onSelect: (id: number) => void;
    onCreate: (name: string) => void;
    onDelete: (id: number) => void;
  }

  let { annotations, activeAnnotation, onSelect, onCreate, onDelete }: Props = $props();

  let showDropdown = $state(false);
  let showCreate = $state(false);
  let newName = $state("");

  function handleCreate() {
    const name = newName.trim();
    if (name) {
      onCreate(name);
      newName = "";
      showCreate = false;
    }
  }

  function handleDelete(id: number, e: Event) {
    e.stopPropagation();
    if (confirm("Delete this annotation?")) {
      onDelete(id);
    }
  }
</script>

<div class="annotation-manager">
  <button class="dropdown-toggle" onclick={() => (showDropdown = !showDropdown)}>
    {#if activeAnnotation}
      {activeAnnotation.name}
    {:else}
      No annotation
    {/if}
    <span class="caret">{showDropdown ? "▲" : "▼"}</span>
  </button>

  {#if showDropdown}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="dropdown-overlay" onclick={() => (showDropdown = false)}></div>
    <div class="dropdown-menu">
      {#each annotations as ann}
        <div class="dropdown-item" class:active={activeAnnotation?.id === ann.id}>
          <button
            class="item-name"
            onclick={() => { onSelect(ann.id); showDropdown = false; }}
          >
            {ann.name}
          </button>
          <button class="delete-btn" onclick={(e) => handleDelete(ann.id, e)}>×</button>
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
        </div>
      {:else}
        <button class="create-btn" onclick={() => (showCreate = true)}>+ New Annotation</button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .annotation-manager {
    position: relative;
    display: inline-block;
  }

  .dropdown-toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    background: #f1f3f5;
    color: #212529;
    border: 1px solid #ced4da;
    padding: 6px 14px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 13px;
  }

  .dropdown-toggle:hover {
    background: #e9ecef;
  }

  .caret {
    font-size: 10px;
    color: #868e96;
  }

  .dropdown-overlay {
    position: fixed;
    inset: 0;
    z-index: 9998;
  }

  .dropdown-menu {
    position: absolute;
    top: 100%;
    left: 0;
    margin-top: 4px;
    background: #ffffff;
    border: 1px solid #dee2e6;
    border-radius: 6px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
    min-width: 220px;
    z-index: 9999;
    padding: 4px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .dropdown-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 8px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 13px;
  }

  .dropdown-item:hover,
  .dropdown-item.active {
    background: #e7f5ff;
  }

  .item-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    background: none;
    border: none;
    padding: 0;
    text-align: left;
    cursor: pointer;
    font-size: 13px;
    color: #212529;
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
    padding: 8px;
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
</style>
