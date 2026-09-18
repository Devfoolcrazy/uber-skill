<script lang="ts">
  import { api, KIND_LABEL, SOURCE_LABEL, targetDirFor, type FileDiff, type InstalledSkill } from "$lib/api";
  import { store, DRIFT_LABEL } from "$lib/store.svelte";

  let diff = $state<{ id: string; files: FileDiff[] } | null>(null);

  async function act(label: string, fn: () => Promise<unknown>) {
    await store.run(label, fn);
    await store.refreshProject().catch(store.fail);
    await store.refreshLibrary().catch(store.fail);
  }

  async function showDiff(id: string) {
    const files = await store.run(null, () => api.diffInstalled(store.kind, id, store.projectPath!, store.target));
    if (files) diff = { id, files };
  }

  const ordered = $derived([
    ...store.status.filter((s) => s.state !== "up-to-date"),
    ...store.status.filter((s) => s.state === "up-to-date"),
  ]);

  function actions(s: InstalledSkill) {
    const p = store.projectPath!;
    const t = store.target;
    const a: { label: string; cls?: string; install?: boolean; run: () => Promise<unknown> }[] = [];
    const requestInstall = async () => { store.requestInstall([s.id]); };
    if (s.state === "library-updated") a.push({ label: "Mettre à jour", cls: "primary", install: true, run: requestInstall });
    if (s.state === "project-modified") a.push({ label: "Remonter dans la bibliothèque", cls: "primary", run: () => api.syncSkill(store.kind, s.id, "push", p, t) });
    if (s.state === "conflict") {
      a.push({ label: "Garder la bibliothèque", install: true, run: requestInstall });
      a.push({ label: "Garder le projet", run: () => api.syncSkill(store.kind, s.id, "push", p, t) });
    }
    if (s.state === "untracked") {
      if (store.skills.some((x) => x.id === s.id)) a.push({ label: "Lier à la bibliothèque", run: () => api.adoptSkill(store.kind, s.id, p, t) });
      else a.push({ label: "Importer dans la bibliothèque", run: () => api.importSkill(store.kind, s.path, null).then(() => api.adoptSkill(store.kind, s.id, p, t)) });
    }
    if (s.state === "missing") a.push({ label: "Réinstaller", install: true, run: requestInstall });
    return a;
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      if (diff) diff = null;
      else store.drawerOpen = false;
    }
  }
</script>

<svelte:window onkeydown={onkeydown} />

{#if store.drawerOpen && store.projectPath}
  <div class="backdrop" role="presentation" onclick={() => (store.drawerOpen = false)}></div>
  <aside class="drawer">
    <header>
      <div class="row">
        <h2>{KIND_LABEL[store.kind].many.charAt(0).toUpperCase() + KIND_LABEL[store.kind].many.slice(1)} installés dans {store.projectName}</h2>
        <span class="spacer"></span>
        <button class="small" onclick={() => store.run(null, () => store.refreshProject())} title="Rafraîchir">↻</button>
        <button class="small" onclick={() => (store.drawerOpen = false)}>✕</button>
      </div>
      <div class="muted selectable">{store.projectPath}/<code>{targetDirFor(store.kind, store.target) ?? "cible non gérée"}</code></div>
      {#if store.driftCount > 0}
        <div class="drift">{store.driftCount} {store.driftCount > 1 ? KIND_LABEL[store.kind].many : KIND_LABEL[store.kind].one} à synchroniser</div>
      {/if}
    </header>

    <div class="list">
      {#if store.status.length === 0}
        <div class="muted empty">
          {#if store.targetOk}
            Aucun {KIND_LABEL[store.kind].one} installé pour cette cible. Coche des {KIND_LABEL[store.kind].many} dans la liste puis « Installer ».
          {:else}
            Cette cible ne gère pas les {KIND_LABEL[store.kind].many}. Choisis Claude Code.
          {/if}
        </div>
      {/if}
      {#each ordered as s (s.id)}
        <div class="entry" class:selected={store.selectedId === s.id}>
          <div class="row">
            <span class="dot {s.state}"></span>
            <button class="name" onclick={() => { if (store.skills.some((x) => x.id === s.id)) { store.selectedId = s.id; store.drawerOpen = false; } }}>{s.id}</button>
            <span class="spacer"></span>
            <span class="state muted">{DRIFT_LABEL[s.state]}</span>
          </div>
          {#if s.description}<div class="desc muted">{s.description}</div>{/if}
          {#if s.lock?.source_state}<div class="muted">À l’installation : {SOURCE_LABEL[s.lock.source_state]}</div>{/if}
          <div class="wrap">
            {#each actions(s) as a}
              <button class="small {a.cls ?? ''}" onclick={() => a.install ? a.run() : act(`${s.id} : ${a.label}`, a.run)}>{a.label}</button>
            {/each}
            {#if ["library-updated", "project-modified", "conflict"].includes(s.state)}
              <button class="small" onclick={() => showDiff(s.id)}>Diff</button>
            {/if}
            <button class="small" onclick={() => api.openInEditor(s.path).catch(store.fail)} title="Ouvrir la copie installée">Ouvrir</button>
            <button class="small danger" onclick={() => act(`${s.id} retiré`, () => api.uninstallSkill(store.kind, s.id, store.projectPath!, store.target))}>Retirer</button>
          </div>
        </div>
      {/each}
    </div>
  </aside>
{/if}

{#if diff}
  <div class="modal-bg" role="presentation" onclick={() => (diff = null)}>
    <div class="modal" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
      <div class="row">
        <h2>Diff : {diff.id}</h2>
        <span class="muted">bibliothèque (a) → projet (b)</span>
        <span class="spacer"></span>
        <button class="small" onclick={() => (diff = null)}>Fermer</button>
      </div>
      {#if diff.files.length === 0}
        <div class="muted">Aucune différence de contenu.</div>
      {/if}
      {#each diff.files as f}
        <h3>{f.file} <span class="muted">({f.kind})</span></h3>
        <pre class="diff">{#each f.unified.split("\n") as line}<span class={line.startsWith("+") ? "add" : line.startsWith("-") ? "del" : line.startsWith("@@") ? "hunk" : ""}>{line}\n</span>{/each}</pre>
      {/each}
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.25);
    z-index: 8;
  }
  .drawer {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(440px, 90vw);
    background: var(--panel);
    border-left: 1px solid var(--border);
    box-shadow: -8px 0 28px rgba(0, 0, 0, 0.2);
    z-index: 9;
    display: flex;
    flex-direction: column;
  }
  header {
    padding: 12px 14px;
    border-bottom: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .drift {
    color: var(--warn);
    font-weight: 600;
    font-size: 12px;
  }
  .list {
    flex: 1;
    overflow-y: auto;
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .empty {
    padding: 20px 0;
    text-align: center;
  }
  .entry {
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .entry.selected {
    border-color: var(--accent);
  }
  .name {
    background: none;
    border: none;
    padding: 0;
    font-family: var(--mono);
    font-size: 12px;
    font-weight: 600;
  }
  .state {
    font-size: 11px;
  }
  .desc {
    font-size: 12px;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .modal-bg {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 10;
  }
  .modal {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    width: min(900px, 90vw);
    max-height: 85vh;
    overflow: auto;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .diff {
    background: var(--panel-2);
    padding: 10px;
    border-radius: 6px;
    overflow-x: auto;
    margin: 0;
  }
  .diff .add {
    color: var(--ok);
  }
  .diff .del {
    color: var(--danger);
  }
  .diff .hunk {
    color: var(--info);
  }
</style>
