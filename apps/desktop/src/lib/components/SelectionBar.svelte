<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { KIND_LABEL, targetDirFor } from "$lib/api";
  import { store } from "$lib/store.svelte";

  const n = $derived(store.checked.size);
  const label = $derived(n > 1 ? KIND_LABEL[store.kind].many : KIND_LABEL[store.kind].one);

  async function install() {
    if (!store.projectPath) {
      const dir = await open({ directory: true, multiple: false, title: "Choisir le dossier du projet" });
      if (typeof dir !== "string") return;
      await store.setProject(dir);
      if (!store.projectPath) return;
    }
    store.requestInstall([...store.checked]);
  }
</script>

{#if n > 0}
  <div class="bar">
    <span><strong>{n}</strong> {label} coché{n > 1 ? "s" : ""}</span>
    <span class="spacer"></span>
    {#if store.projectPath && !store.targetOk}
      <span class="bad">Cette cible ne gère pas les {KIND_LABEL[store.kind].many}</span>
      <button class="primary" disabled>Installer</button>
    {:else if store.projectPath}
      <span class="muted">→ {store.projectName} <code>{targetDirFor(store.kind, store.target)}</code></span>
      <button class="primary" onclick={install}>Installer</button>
    {:else}
      <button class="primary" onclick={install}>Choisir un projet…</button>
    {/if}
    <button onclick={() => (store.checked = new Set())}>Tout décocher</button>
  </div>
{/if}

<style>
  .bar {
    position: fixed;
    left: 50%;
    bottom: 18px;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px 8px 16px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 8px 28px rgba(0, 0, 0, 0.22);
    z-index: 15;
    min-width: 420px;
  }
  .bad {
    color: var(--danger);
  }
</style>
