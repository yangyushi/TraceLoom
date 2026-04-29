<script lang="ts">
  interface Props {
    workspaceName: string;
    hasUnsavedChanges: boolean;
    onNewWorkspace: () => void;
    onOpenWorkspace: () => void;
    onSaveWorkspace: () => void;
    onSaveWorkspaceAs: () => void;
    onAddFile: () => void;
    onAddFolder: () => void;
    showBookmarkPanel: boolean;
    onToggleBookmarkPanel: () => void;
    theme: "dots" | "bricks";
    onSetTheme: (theme: "dots" | "bricks") => void;
  }

  let {
    workspaceName,
    hasUnsavedChanges,
    onNewWorkspace,
    onOpenWorkspace,
    onSaveWorkspace,
    onSaveWorkspaceAs,
    onAddFile,
    onAddFolder,
    showBookmarkPanel,
    onToggleBookmarkPanel,
    theme,
    onSetTheme,
  }: Props = $props();
</script>

<header class="toolbar">
  <div class="toolbar-left">
    <button onclick={onNewWorkspace}>New</button>
    <button onclick={onOpenWorkspace}>Open</button>
    <button onclick={onSaveWorkspace}>Save</button>
    <button onclick={onSaveWorkspaceAs}>Save As</button>
    <span class="divider"></span>
    <button onclick={onAddFile}>Add File</button>
    <button onclick={onAddFolder}>Add Folder</button>
    <span class="workspace-name" title={workspaceName}>
      {workspaceName}{hasUnsavedChanges ? " *" : ""}
    </span>
  </div>

  <div class="toolbar-center">
    <div class="theme-toggle">
      <button class:active={theme === "dots"} onclick={() => onSetTheme("dots")}>Dots</button>
      <button class:active={theme === "bricks"} onclick={() => onSetTheme("bricks")}>Bricks</button>
    </div>
  </div>

  <div class="toolbar-right">
    <button
      class="icon-btn"
      class:active={showBookmarkPanel}
      onclick={onToggleBookmarkPanel}
      title="Toggle bookmarks"
    >
      Bookmarks
    </button>
  </div>
</header>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 16px;
    background: #ffffff;
    border-bottom: 1px solid #dee2e6;
    flex-shrink: 0;
    justify-content: space-between;
  }

  .toolbar-left,
  .toolbar-center,
  .toolbar-right {
    display: flex;
    align-items: center;
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

  .divider {
    width: 1px;
    height: 20px;
    background: #dee2e6;
    margin: 0 4px;
  }

  .workspace-name {
    font-size: 13px;
    font-weight: 600;
    color: #495057;
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .icon-btn {
    padding: 6px 12px;
  }

  .theme-toggle {
    display: flex;
    gap: 4px;
  }
</style>
