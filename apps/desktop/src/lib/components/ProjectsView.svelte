<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, targetKey, targetLabel, type FileDiff, type InstalledSkill, type InstallRequest, type ItemKind, type ProjectOverview, type TargetInstall } from "$lib/api";
  import { store, DRIFT_LABEL } from "$lib/store.svelte";
  import SuggestDialog from "./SuggestDialog.svelte";

  let selectedPath = $state<string | null>(null);
  let suggestFor = $state<ProjectOverview | null>(null);
  let diff = $state<{ id: string; files: FileDiff[] } | null>(null);

  const selected = $derived(store.projects.find((p) => p.path === selectedPath) ?? store.projects[0] ?? null);
  const targets = $derived(new Set(selected?.installs.map((i) => targetKey(i.target)) ?? []));
  const total = $derived(selected?.installs.reduce((n, i) => n + i.items.length, 0) ?? 0);

  onMount(() => {
    selectedPath = store.projectPath;
    store.refreshProjects();
  });

  /// One request per target and kind, limited to the copies that are simply behind.
  function behindRequests(project: ProjectOverview): InstallRequest[] {
    return project.installs
      .map((i) => ({ project: project.path, kind: i.kind, target: i.target, ids: i.items.filter((s) => s.state === "library-updated").map((s) => s.id) }))
      .filter((r) => r.ids.length > 0);
  }

  async function follow() {
    const dir = await open({ directory: true, multiple: false, title: "Suivre un projet" });
    if (typeof dir !== "string") return;
    const config = await store.run("Projet suivi", () => api.trackProject(dir));
    if (config) {
      store.config = config;
      selectedPath = dir;
      await store.refreshProjects();
    }
  }

  async function unfollow(project: ProjectOverview) {
    const config = await store.run(`${project.name} retiré de la liste`, () => api.untrackProject(project.path));
    if (config) {
      store.config = config;
      await store.refreshProjects();
    }
  }

  async function workIn(project: ProjectOverview) {
    await store.setProject(project.path, project.installs[0]?.target);
    store.view = "library";
  }

  async function showDiff(project: ProjectOverview, install: TargetInstall, item: InstalledSkill) {
    const files = await store.run(null, () => api.diffInstalled(install.kind, item.id, project.path, install.target));
    if (files) diff = { id: item.id, files };
  }

  async function pushToLibrary(project: ProjectOverview, install: TargetInstall, item: InstalledSkill) {
    const done = await store.run(`${item.id} remonté dans la bibliothèque`, async () => {
      await api.syncSkill(install.kind, item.id, "push", project.path, install.target);
      return true;
    });
    if (done) await store.refreshAfterGit();
  }

  const inLibrary = (kind: ItemKind, id: string) => !!store.libraries[kind]?.skills.some((s) => s.id === id);

  /// Follow a copy the app did not install: link it to its library twin, or import it first.
  async function adopt(project: ProjectOverview, install: TargetInstall, item: InstalledSkill, importFirst: boolean) {
    const done = await store.run(importFirst ? `${item.id} importé dans la bibliothèque` : `${item.id} lié à la bibliothèque`, async () => {
      if (importFirst) await api.importSkill(install.kind, item.path, null);
      await api.adoptSkill(install.kind, item.id, project.path, install.target);
      return true;
    });
    if (done) await store.refreshAfterGit();
  }

  const update = (project: ProjectOverview, install: TargetInstall, item: InstalledSkill) =>
    store.requestInstalls([{ project: project.path, kind: install.kind, target: install.target, ids: [item.id] }]);
</script>

<div class="projects">
  <aside aria-label="Projets suivis">
    <h3>Projets suivis</h3>
    <ul>
      {#each store.projects as p (p.path)}
        <li>
          <button class="project" class:active={selected?.path === p.path} onclick={() => (selectedPath = p.path)} title={p.path}>
            <span class="name">{p.global ? "🌐 Global" : p.name}</span>
            {#if !p.exists}
              <span class="missing">introuvable</span>
            {:else if p.behind > 0}
              <span class="behind" title="Copies en retard sur la bibliothèque">{p.behind} en retard</span>
            {:else}
              <span class="ok" title="Aucune copie en retard">✓</span>
            {/if}
          </button>
        </li>
      {:else}
        <li class="muted empty">Aucun projet suivi pour l’instant. Un projet est suivi dès qu’un élément y est installé.</li>
      {/each}
    </ul>
    <button class="small" onclick={follow}>+ Suivre un projet…</button>
  </aside>

  <section class="detail" aria-label="Contenu du projet">
    {#if !selected}
      <p class="muted center">Suivez un projet pour voir ce qui y est installé et ce qui est en retard sur la bibliothèque.</p>
    {:else}
      <header>
        <div class="row">
          <h2 class="selectable">{selected.global ? "🌐 Global" : selected.name}</h2>
          {#if selected.path === store.projectPath}<span class="chip">projet courant</span>{/if}
          <span class="spacer"></span>
          {#if selected.exists && selected.behind > 0}
            <button class="small primary" onclick={() => store.requestInstalls(behindRequests(selected))}>Tout mettre à jour ({selected.behind})</button>
          {/if}
          {#if selected.exists && !selected.global}
            <button class="small" title="Demander à Claude quels skills et agents de la bibliothèque conviennent à ce projet" onclick={() => (suggestFor = selected)}>Suggérer des skills…</button>
          {/if}
          {#if selected.exists && selected.path !== store.projectPath}
            <button class="small" title={selected.global ? "Installer globalement depuis la bibliothèque" : "En faire le projet courant et revenir à la bibliothèque"} onclick={() => workIn(selected)}>{selected.global ? "Installer globalement…" : "Travailler dans ce projet"}</button>
          {/if}
          {#if !selected.global}
            <button class="small" title="Ne plus suivre ce projet. Rien n’est supprimé dans le projet." onclick={() => unfollow(selected)}>Retirer de la liste</button>
          {/if}
        </div>
        <p class="muted selectable">{selected.global ? "Vos dossiers personnels (~/.claude, ~/.agents) : ce qui y est installé est disponible dans tous les projets, pour Claude Code et Codex." : selected.path}</p>
        {#if selected.exists}
          <p class="muted">{total} élément(s) installé(s){selected.behind > 0 ? ` · ${selected.behind} en retard sur la bibliothèque` : total > 0 ? " · tout est à jour ou demande votre attention" : ""}</p>
        {/if}
      </header>

      {#if !selected.exists}
        <p class="warning">Ce dossier est introuvable : le projet a peut-être été déplacé ou supprimé. Il reste dans la liste tant que vous ne le retirez pas.</p>
      {:else if selected.installs.length === 0}
        <p class="muted">{selected.global ? "Rien n’est installé globalement." : "Rien n’a été installé dans ce projet depuis l’application."}</p>
      {/if}

      {#each selected.installs as install (targetKey(install.target) + install.kind)}
        <h3>{(install.kind === "skill" ? "Skills" : "Agents") + (targets.size > 1 ? ` · ${targetLabel(install.target)}` : "")}</h3>
        <ul class="items">
          {#each install.items as item (item.id)}
            <li>
              <span class="dot {item.state}"></span>
              <span class="id">{item.id}</span>
              <span class="muted state">{DRIFT_LABEL[item.state]}</span>
              <span class="spacer"></span>
              {#if item.state === "library-updated" || item.state === "missing"}
                <button class="small primary" aria-label={`Mettre à jour ${item.id}`} onclick={() => update(selected, install, item)}>{item.state === "missing" ? "Réinstaller" : "Mettre à jour"}</button>
              {/if}
              {#if item.state === "project-modified"}
                <button class="small" aria-label={`Remonter ${item.id} dans la bibliothèque`} onclick={() => pushToLibrary(selected, install, item)}>Remonter dans la bibliothèque</button>
              {/if}
              {#if item.state === "untracked"}
                {#if inLibrary(install.kind, item.id)}
                  <button class="small" aria-label={`Lier ${item.id} à la bibliothèque`} title="Cette copie existe déjà dans la bibliothèque : la suivre sans la modifier" onclick={() => adopt(selected, install, item, false)}>Lier à la bibliothèque</button>
                {:else}
                  <button class="small" aria-label={`Importer ${item.id} dans la bibliothèque`} title="Copier cet élément dans la bibliothèque, puis suivre cette copie" onclick={() => adopt(selected, install, item, true)}>Importer dans la bibliothèque</button>
                {/if}
              {/if}
              {#if item.state === "conflict"}
                <button class="small" aria-label={`Garder la bibliothèque pour ${item.id}`} onclick={() => update(selected, install, item)}>Garder la bibliothèque</button>
              {/if}
              {#if ["library-updated", "project-modified", "conflict"].includes(item.state)}
                <button class="small" aria-label={`Diff de ${item.id}`} onclick={() => showDiff(selected, install, item)}>Diff</button>
              {/if}
              <button class="small" aria-label={`Retirer ${item.id} du projet`} onclick={() => store.uninstall(item.id, { project: selected.path, target: install.target, kind: install.kind, state: item.state })}>{selected.global ? "Retirer…" : "Retirer du projet…"}</button>
            </li>
          {/each}
        </ul>
      {/each}
      {#if selected.exists && selected.installs.length > 0}
        <p class="muted note">« Tout mettre à jour » ne touche que les copies simplement en retard. Une copie modifiée dans le projet ou en conflit n’est jamais remplacée en lot.</p>
      {/if}
    {/if}
  </section>
</div>

{#if suggestFor}
  <SuggestDialog project={suggestFor} onclose={() => (suggestFor = null)} />
{/if}

{#if diff}
  <div class="modal-bg" role="presentation" onclick={() => (diff = null)}>
    <div class="modal" role="dialog" aria-label={`Diff de ${diff.id}`} tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.key === "Escape" && (diff = null)}>
      <div class="row">
        <h2>Diff : {diff.id}</h2>
        <span class="muted">− bibliothèque · + projet</span>
        <span class="spacer"></span>
        <button class="small" onclick={() => (diff = null)}>Fermer</button>
      </div>
      {#each diff.files as f}
        <h4>{f.file} <span class="muted">({f.kind})</span></h4>
        <pre class="diff selectable">{#each f.unified.split("\n") as line}<span class={line.startsWith("+") ? "add" : line.startsWith("-") ? "del" : line.startsWith("@@") ? "hunk" : ""}>{line}{"\n"}</span>{/each}</pre>
      {:else}
        <p class="muted">Aucune différence de contenu.</p>
      {/each}
    </div>
  </div>
{/if}

<style>
  .projects { display: flex; flex: 1; min-height: 0; min-width: 0; }
  aside { width: 280px; flex: none; padding: 14px 12px; border-right: 1px solid var(--border); overflow-y: auto; display: flex; flex-direction: column; gap: 10px; }
  aside h3, .detail h3 { margin: 0; font-size: 11px; text-transform: uppercase; letter-spacing: 0.06em; color: var(--muted); }
  ul { list-style: none; margin: 0; padding: 0; }
  .project { width: 100%; display: flex; align-items: center; gap: 8px; padding: 7px 8px; border: none; background: none; border-radius: 6px; text-align: left; }
  .project.active { background: var(--accent-soft); }
  .project .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 500; }
  .behind { color: var(--warn); font-size: 11px; font-weight: 600; white-space: nowrap; }
  .missing { color: var(--danger); font-size: 11px; }
  .ok { color: var(--ok); }
  .empty { padding: 6px 2px; }
  .detail { flex: 1; min-width: 0; padding: 18px 22px; overflow-y: auto; }
  .detail h2 { margin: 0; font-size: 18px; }
  .detail h3 { margin: 22px 0 6px; }
  header p { margin: 4px 0 0; overflow-wrap: anywhere; }
  .items li { display: flex; align-items: center; gap: 8px; padding: 7px 0; border-bottom: 1px solid var(--border); flex-wrap: wrap; }
  .id { font-family: var(--mono); }
  .state { font-size: 12px; }
  .center { margin-top: 20vh; text-align: center; }
  .warning { color: var(--warn); }
  .note { margin-top: 16px; font-size: 12px; }
  .modal-bg { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.45); display: flex; align-items: center; justify-content: center; z-index: 30; }
  .modal { width: min(900px, calc(100vw - 40px)); max-height: calc(100vh - 60px); overflow: auto; padding: 18px; background: var(--panel); border: 1px solid var(--border); border-radius: var(--radius); }
  .modal h2 { margin: 0; font-size: 16px; }
  .diff { font: 12px/1.5 var(--mono); background: var(--panel-2); padding: 10px; border-radius: 6px; overflow-x: auto; margin: 0; }
  .add { color: var(--ok); }
  .del { color: var(--danger); }
  .hunk { color: var(--info); }
</style>
