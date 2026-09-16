<script lang="ts">
  import { onMount } from "svelte";
  import "../app.css";
  import { store } from "$lib/store.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import SkillList from "$lib/components/SkillList.svelte";
  import SkillDetail from "$lib/components/SkillDetail.svelte";
  import ProjectPanel from "$lib/components/ProjectPanel.svelte";

  onMount(() => {
    store.init();
  });
</script>

<div class="app">
  <Sidebar />
  <SkillList />
  <SkillDetail />
  {#if store.showProject}
    <ProjectPanel />
  {:else}
    <button class="show-project" onclick={() => (store.showProject = true)} title="Afficher le panneau projet">Projet ▸</button>
  {/if}
</div>

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
  .app {
    display: flex;
    height: 100vh;
    overflow: hidden;
  }
  .show-project {
    position: fixed;
    right: 12px;
    bottom: 12px;
  }
  .banner {
    position: fixed;
    left: 50%;
    transform: translateX(-50%);
    bottom: 16px;
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
