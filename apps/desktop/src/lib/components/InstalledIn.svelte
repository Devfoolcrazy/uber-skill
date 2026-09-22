<script lang="ts">
  import { targetKey, targetLabel, type Skill } from "$lib/api";
  import { store, DRIFT_LABEL } from "$lib/store.svelte";

  /// Where an item is installed among the followed projects, and where it is behind.
  let { skill }: { skill: Skill } = $props();

  const copies = $derived(store.installationsOf(skill.id, skill.kind));
  const behind = $derived(copies.filter((c) => c.item.state === "library-updated"));
  const projects = $derived(new Set(copies.map((c) => c.project.path)).size);
  const severalTargets = $derived(new Set(copies.map((c) => targetKey(c.target))).size > 1);

  const request = (c: (typeof copies)[number]) => ({ project: c.project.path, kind: skill.kind, target: c.target, ids: [skill.id] });
</script>

{#if copies.length > 0}
  <details class="installed-in" open={behind.length > 0}>
    <summary>
      <span class="title">Installé dans</span>
      <span>{projects} projet{projects > 1 ? "s" : ""}</span>
      {#if behind.length > 0}<span class="behind">· {behind.length} en retard</span>{:else}<span class="ok">· à jour ou sans retard</span>{/if}
      <span class="spacer"></span>
      {#if behind.length > 1}
        <button class="small primary" disabled={store.editorDirty} title={store.editorDirty ? "Enregistrez vos modifications d’abord" : "Met à jour les copies simplement en retard ; les copies modifiées dans un projet ne sont pas touchées"} onclick={(e) => { e.preventDefault(); store.requestInstalls(behind.map(request)); }}>Mettre à jour partout ({behind.length})</button>
      {/if}
    </summary>
    <ul>
      {#each copies as c (c.project.path + targetKey(c.target))}
        <li>
          <span class="dot {c.item.state}"></span>
          <button class="link" title={c.project.global ? "Vos dossiers personnels : disponible dans tous les projets" : c.project.path} onclick={() => (store.view = "projects")}>{c.project.global ? "🌐 Global" : c.project.name}</button>
          {#if c.project.path === store.projectPath}<span class="chip">projet courant</span>{/if}
          {#if severalTargets}<span class="muted">{targetLabel(c.target)}</span>{/if}
          <span class="muted">{DRIFT_LABEL[c.item.state]}</span>
          <span class="spacer"></span>
          {#if c.item.state === "library-updated" || c.item.state === "missing"}
            <button class="small" aria-label={`Mettre à jour dans ${c.project.name}`} onclick={() => store.requestInstalls([request(c)])}>{c.item.state === "missing" ? "Réinstaller" : "Mettre à jour"}</button>
          {/if}
        </li>
      {/each}
    </ul>
  </details>
{/if}

<style>
  .installed-in { margin: 10px 0 4px; padding: 8px 10px; background: var(--panel-2); border-radius: var(--radius); font-size: 12px; }
  summary { display: flex; align-items: center; gap: 6px; cursor: pointer; list-style: none; flex-wrap: wrap; }
  summary span, li .chip, .link, button { white-space: nowrap; flex: none; }
  li .muted { min-width: 0; }
  summary::-webkit-details-marker { display: none; }
  .title { text-transform: uppercase; letter-spacing: 0.06em; font-size: 11px; color: var(--muted); }
  .behind { color: var(--warn); font-weight: 600; }
  .ok { color: var(--muted); }
  ul { list-style: none; margin: 8px 0 0; padding: 0; }
  li { display: flex; align-items: center; gap: 8px; padding: 4px 0; flex-wrap: wrap; }
  .link { border: none; background: none; padding: 0; color: var(--text); font-weight: 500; cursor: pointer; }
  .link:hover { text-decoration: underline; }
</style>
