<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, KIND_LABEL, TARGETS, targetDirFor, targetKey, targetSupports, type ItemKind } from "$lib/api";

  const KINDS: ItemKind[] = ["skill", "agent"];
  import { store } from "$lib/store.svelte";

  let settingsOpen = $state(false);
  let editorDraft = $state("");

  async function pickProject() {
    store.pickerOpen = false;
    const dir = await open({ directory: true, multiple: false, title: "Choisir le dossier du projet" });
    if (typeof dir === "string") await store.setProject(dir);
  }

  async function changeTarget(e: Event) {
    const key = (e.target as HTMLSelectElement).value;
    const t = TARGETS.find((x) => targetKey(x.target) === key)?.target ?? { kind: "claude-code" as const };
    await store.setProject(store.projectPath, t);
  }

  async function saveEditor() {
    await store.run("Éditeur enregistré", async () => {
      store.config = await api.setEditor(editorDraft || null);
    });
    settingsOpen = false;
  }

  function onWindowClick(e: MouseEvent) {
    const el = e.target as HTMLElement;
    if (!el.closest(".picker") && !el.closest(".project-btn")) store.pickerOpen = false;
    if (!el.closest(".settings") && !el.closest(".settings-btn")) settingsOpen = false;
  }
</script>

<svelte:window onclick={onWindowClick} />

<header class="topbar">
  <h1>Uber Skill</h1>
  <nav class="kinds">
    {#each KINDS as k (k)}
      <button class:active={store.kind === k} onclick={() => store.setKind(k)}>
        {k === "skill" ? "Skills" : "Agents"}
        <span class="count">{store.libraries[k]?.skills.length ?? 0}</span>
      </button>
    {/each}
  </nav>
  <span class="spacer"></span>

  <div class="project">
    <button class="project-btn" class:empty={!store.projectPath} onclick={() => (store.pickerOpen = !store.pickerOpen)} title={store.projectPath ?? "Aucun projet"}>
      <span class="label">Projet</span>
      <span class="name">{store.projectName ?? "Choisir un projet…"}</span>
      <span class="caret">▾</span>
    </button>
    {#if store.pickerOpen}
      <div class="picker">
        <button class="item" onclick={pickProject}>Choisir un dossier…</button>
        {#if store.config?.recent_projects.length}
          <div class="sep">Récents</div>
          {#each store.config.recent_projects as p (p.path)}
            <button class="item recent" class:active={p.path === store.projectPath} onclick={() => { store.pickerOpen = false; store.setProject(p.path, p.target); }} title={p.path}>
              <span>{p.path.split("/").filter(Boolean).pop()}</span>
              <span class="muted">{p.path.split("/").slice(-3, -1).join("/")}</span>
            </button>
          {/each}
        {/if}
        {#if store.projectPath}
          <div class="sep"></div>
          <button class="item" onclick={() => { store.pickerOpen = false; store.setProject(null); }}>Fermer le projet</button>
        {/if}
      </div>
    {/if}
  </div>

  {#if store.projectPath}
    <select value={targetKey(store.target)} onchange={changeTarget} title="Dossier cible : {targetDirFor(store.kind, store.target) ?? 'non géré'}" class:bad={!store.targetOk}>
      {#each TARGETS as t}
        <option value={targetKey(t.target)} disabled={!targetSupports(store.kind, t.target)}>{t.label}{targetSupports(store.kind, t.target) ? "" : " (pas d'agents)"}</option>
      {/each}
    </select>
    <button class="installed-btn" class:active={store.drawerOpen} onclick={() => (store.drawerOpen = !store.drawerOpen)}>
      Installés
      <span class="count">{store.status.length}</span>
      {#if store.totalDrift > 0}
        <span class="badge" title="{store.totalDrift} élément(s) à synchroniser (skills et agents)">{store.totalDrift}</span>
      {/if}
    </button>
  {/if}

  <div class="settings-wrap">
    <button class="small settings-btn" title="Réglages" onclick={() => { settingsOpen = !settingsOpen; editorDraft = store.config?.editor_command ?? ""; }}>⚙</button>
    {#if settingsOpen}
      <form class="settings" onsubmit={(e) => { e.preventDefault(); saveEditor(); }}>
        <label>Éditeur externe <input type="text" placeholder="code (par défaut)" bind:value={editorDraft} /></label>
        <div class="muted">Commande suivie du chemin à ouvrir.</div>
        <button type="button" class="small" disabled={!store.config?.library_path} onclick={() => { settingsOpen = false; store.registryOpen = true; }}>Bibliothèque : tags et catégories…</button>
        <div class="row">
          <button type="submit" class="small primary">Enregistrer</button>
          <button type="button" class="small" onclick={() => (settingsOpen = false)}>Annuler</button>
        </div>
      </form>
    {/if}
  </div>
</header>

<style>
  .topbar {
    height: 44px;
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
    border-bottom: 1px solid var(--border);
    background: var(--panel);
    position: relative;
    z-index: 5;
  }
  h1 {
    font-size: 14px;
    margin-right: 8px;
  }
  .kinds {
    display: flex;
    background: var(--panel-2);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 2px;
    gap: 2px;
  }
  .kinds button {
    background: none;
    border: none;
    padding: 3px 10px;
    border-radius: 6px;
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .kinds button.active {
    background: var(--panel);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.12);
    font-weight: 600;
  }
  select.bad {
    border-color: var(--danger);
  }
  .project {
    position: relative;
  }
  .project-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    max-width: 320px;
  }
  .project-btn.empty {
    border-style: dashed;
  }
  .project-btn .label {
    color: var(--muted);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .project-btn .name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .caret {
    color: var(--muted);
  }
  .picker,
  .settings {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    min-width: 280px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.18);
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .picker .item {
    background: none;
    border: none;
    text-align: left;
    padding: 6px 8px;
    border-radius: 6px;
  }
  .picker .item:hover {
    background: var(--panel-2);
  }
  .picker .item.active {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .picker .recent {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .picker .recent .muted {
    font-size: 11px;
  }
  .sep {
    font-size: 11px;
    color: var(--muted);
    padding: 6px 8px 2px;
    border-top: 1px solid var(--border);
    margin-top: 4px;
  }
  .installed-btn {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .installed-btn.active {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .count {
    color: var(--muted);
    font-size: 11px;
  }
  .badge {
    background: var(--warn);
    color: #fff;
    border-radius: 999px;
    padding: 0 6px;
    font-size: 11px;
    font-weight: 600;
  }
  .settings-wrap {
    position: relative;
  }
  .settings {
    gap: 8px;
    padding: 10px;
  }
  .settings label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 11px;
    color: var(--muted);
  }
</style>
