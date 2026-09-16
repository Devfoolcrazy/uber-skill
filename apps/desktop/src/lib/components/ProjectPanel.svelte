<script lang="ts">
  import { confirm, open } from "@tauri-apps/plugin-dialog";
  import { api, TARGETS, targetDir, targetKey, type FileDiff, type InstalledSkill } from "$lib/api";
  import { store, DRIFT_LABEL } from "$lib/store.svelte";

  let diff = $state<{ id: string; files: FileDiff[] } | null>(null);

  async function pickProject() {
    const dir = await open({ directory: true, multiple: false, title: "Choisir le dossier du projet" });
    if (typeof dir === "string") await store.setProject(dir);
  }

  async function changeTarget(e: Event) {
    const key = (e.target as HTMLSelectElement).value;
    const t = TARGETS.find((x) => targetKey(x.target) === key)?.target ?? { kind: "claude-code" as const };
    await store.setProject(store.projectPath, t);
  }

  /// Returns false if the user declined a host-mismatch warning.
  async function confirmHosts(ids: string[]): Promise<boolean> {
    const warnings = await api.checkHosts(ids, store.target).catch((e) => { store.fail(e); return [] as string[]; });
    if (warnings.length === 0) return true;
    return confirm(`${warnings.join("\n")}\n\nInstaller quand même ?`, { title: "Harnais incompatible", kind: "warning" });
  }

  async function installChecked() {
    if (!store.projectPath || store.checked.size === 0) return;
    const ids = [...store.checked];
    if (!(await confirmHosts(ids))) return;
    const r = await store.run(`${ids.length} skill(s) installé(s)`, () => api.installSkills(ids, store.projectPath!, store.target));
    if (r) {
      store.checked = new Set();
      await store.refreshProject();
    }
  }

  async function installOne(id: string) {
    if (!store.projectPath) return;
    if (!(await confirmHosts([id]))) return;
    await store.run(`${id} installé`, () => api.installSkills([id], store.projectPath!, store.target));
    await store.refreshProject();
  }

  async function act(label: string, fn: () => Promise<unknown>) {
    await store.run(label, fn);
    await store.refreshProject().catch(store.fail);
    await store.refreshLibrary().catch(store.fail);
  }

  async function showDiff(id: string) {
    const files = await store.run(null, () => api.diffInstalled(id, store.projectPath!, store.target));
    if (files) diff = { id, files };
  }

  const groups = $derived.by(() => {
    const drifted = store.projectStatus.filter((s) => s.state !== "up-to-date");
    const fine = store.projectStatus.filter((s) => s.state === "up-to-date");
    return { drifted, fine };
  });

  const selectedInstalled = $derived(store.selectedId ? store.statusOf(store.selectedId) : undefined);

  function actions(s: InstalledSkill) {
    const p = store.projectPath!;
    const t = store.target;
    const a: { label: string; cls?: string; run: () => Promise<unknown> }[] = [];
    if (s.state === "library-updated") a.push({ label: "Mettre à jour", cls: "primary", run: () => api.syncSkill(s.id, "pull", p, t) });
    if (s.state === "project-modified") a.push({ label: "Remonter dans la bibliothèque", cls: "primary", run: () => api.syncSkill(s.id, "push", p, t) });
    if (s.state === "conflict") {
      a.push({ label: "Garder la bibliothèque", run: () => api.syncSkill(s.id, "pull", p, t) });
      a.push({ label: "Garder le projet", run: () => api.syncSkill(s.id, "push", p, t) });
    }
    if (s.state === "untracked") {
      if (store.skills.some((x) => x.id === s.id)) a.push({ label: "Lier à la bibliothèque", run: () => api.adoptSkill(s.id, p, t) });
      else a.push({ label: "Importer dans la bibliothèque", run: () => api.importSkill(s.path, null).then(() => api.adoptSkill(s.id, p, t)) });
    }
    if (s.state === "missing") a.push({ label: "Réinstaller", run: () => api.syncSkill(s.id, "pull", p, t) });
    return a;
  }
</script>

<aside class="project">
  <header>
    <div class="row">
      <h3>Projet</h3>
      <span class="spacer"></span>
      <button class="small" onclick={() => (store.showProject = false)} title="Masquer">✕</button>
    </div>
    <div class="row">
      <button class="small" onclick={pickProject}>Choisir…</button>
      {#if store.config?.recent_projects.length}
        <select class="small" onchange={(e) => { const v = (e.target as HTMLSelectElement).value; const r = store.config!.recent_projects.find((p) => p.path === v); if (r) store.setProject(r.path, r.target); }}>
          <option value="">Récents…</option>
          {#each store.config.recent_projects as p}
            <option value={p.path} selected={p.path === store.projectPath}>{p.path.split("/").slice(-2).join("/")}</option>
          {/each}
        </select>
      {/if}
    </div>
    {#if store.projectPath}
      <div class="path selectable" title={store.projectPath}>{store.projectPath}</div>
      <label class="row">
        <span class="muted">Cible</span>
        <select value={targetKey(store.target)} onchange={changeTarget}>
          {#each TARGETS as t}<option value={targetKey(t.target)}>{t.label}</option>{/each}
        </select>
      </label>
      <div class="muted"><code>{targetDir(store.target)}</code></div>
    {:else}
      <div class="muted">Choisis un projet pour y installer des skills.</div>
    {/if}
  </header>

  {#if store.projectPath}
    <section class="install">
      <button class="primary" disabled={store.checked.size === 0} onclick={installChecked}>
        Installer {store.checked.size > 0 ? `${store.checked.size} skill(s) coché(s)` : "la sélection"}
      </button>
      {#if store.selectedId && !selectedInstalled}
        <button onclick={() => installOne(store.selectedId!)}>Installer « {store.selectedId} »</button>
      {/if}
    </section>

    <section class="installed">
      <div class="row">
        <h3>Installés ({store.projectStatus.length})</h3>
        <span class="spacer"></span>
        <button class="small" onclick={() => store.run(null, () => store.refreshProject())}>↻</button>
      </div>
      {#if store.projectStatus.length === 0}
        <div class="muted">Aucun skill installé pour cette cible.</div>
      {/if}
      {#each [...groups.drifted, ...groups.fine] as s (s.id)}
        <div class="entry" class:selected={store.selectedId === s.id}>
          <div class="row">
            <span class="dot {s.state}"></span>
            <button class="name" onclick={() => { if (store.skills.some((x) => x.id === s.id)) store.selectedId = s.id; }}>{s.id}</button>
            <span class="spacer"></span>
            <span class="state muted">{DRIFT_LABEL[s.state]}</span>
          </div>
          <div class="wrap">
            {#each actions(s) as a}
              <button class="small {a.cls ?? ''}" onclick={() => act(`${s.id} : ${a.label}`, a.run)}>{a.label}</button>
            {/each}
            {#if ["library-updated", "project-modified", "conflict"].includes(s.state)}
              <button class="small" onclick={() => showDiff(s.id)}>Diff</button>
            {/if}
            <button class="small" onclick={() => api.openInEditor(s.path).catch(store.fail)} title="Ouvrir la copie installée">Ouvrir</button>
            <button class="small danger" onclick={() => act(`${s.id} retiré`, () => api.uninstallSkill(s.id, store.projectPath!, store.target))}>Retirer</button>
          </div>
        </div>
      {/each}
    </section>
  {/if}
</aside>

{#if diff}
  <div class="modal-bg" role="presentation" onclick={() => (diff = null)} onkeydown={(e) => { if (e.key === "Escape") diff = null; }}>
    <div class="modal" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => { if (e.key === "Escape") diff = null; }}>
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
  .project {
    width: 340px;
    flex: none;
    border-left: 1px solid var(--border);
    background: var(--panel);
    display: flex;
    flex-direction: column;
    overflow-y: auto;
  }
  header,
  section {
    padding: 12px;
    border-bottom: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .path {
    font-family: var(--mono);
    font-size: 11px;
    word-break: break-all;
  }
  select {
    flex: 1;
    min-width: 0;
  }
  .install button {
    width: 100%;
  }
  .entry {
    padding: 8px;
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
