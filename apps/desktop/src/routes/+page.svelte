<script lang="ts">
  import { onMount } from "svelte";
  import "../app.css";
  import { store } from "$lib/store.svelte";
  import TopBar from "$lib/components/TopBar.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import SkillList from "$lib/components/SkillList.svelte";
  import SkillDetail from "$lib/components/SkillDetail.svelte";
  import ProjectDrawer from "$lib/components/ProjectDrawer.svelte";
  import SelectionBar from "$lib/components/SelectionBar.svelte";
  import GitSyncDialog from "$lib/components/GitSyncDialog.svelte";
  import LibrarySettings from "$lib/components/LibrarySettings.svelte";
  import ProjectsView from "$lib/components/ProjectsView.svelte";

  onMount(() => {
    store.init();
  });
</script>

<div class="app">
  <TopBar />
  <div class="main">
    {#if store.view === "projects"}
      <ProjectsView />
    {/if}
    <!-- Kept mounted while the projects are shown: the editor may hold unsaved changes. -->
    <div class="library" class:away={store.view !== "library"}>
      <Sidebar />
      <SkillList />
      {#key `${store.libraryGeneration}:${store.config?.agents_path}:${store.kind}`}
        <SkillDetail />
      {/key}
    </div>
  </div>
</div>

<ProjectDrawer />
<SelectionBar />
{#if store.registryOpen}
  <LibrarySettings onclose={() => (store.registryOpen = false)} />
{/if}
{#if store.gitDialog}
  <GitSyncDialog request={store.gitDialog.install} onclose={() => (store.gitDialog = null)} />
{/if}

{#if store.error}
  <div class="banner error" role="alert">
    <span class="selectable">{store.error}</span>
    <button class="small" onclick={() => (store.error = null)}>✕</button>
  </div>
{/if}
{#if store.toast}
  <div class="banner toast">{store.toast}</div>
{/if}
{#if store.loading}
  <div class="loading"></div>
{/if}

<style>
  .library {
    display: contents;
  }
  .library.away {
    display: none;
  }
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
  }
  .main {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .banner {
    position: fixed;
    left: 50%;
    transform: translateX(-50%);
    bottom: 72px;
    padding: 8px 14px;
    border-radius: 8px;
    display: flex;
    gap: 12px;
    align-items: center;
    z-index: 20;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.25);
  }
  .error {
    background: var(--danger);
    color: #fff;
  }
  .toast {
    background: var(--text);
    color: var(--bg);
  }
  .loading {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    height: 2px;
    background: var(--accent);
    animation: pulse 1s infinite alternate;
    z-index: 30;
  }
  @keyframes pulse {
    from {
      opacity: 0.3;
    }
    to {
      opacity: 1;
    }
  }
</style>
