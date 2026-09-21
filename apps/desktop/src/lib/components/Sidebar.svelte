<script lang="ts">
  import { store } from "$lib/store.svelte";
  import { hostKind, KIND_LABEL } from "$lib/api";
  import { warningText } from "$lib/lint";
  import LibraryChooser from "./LibraryChooser.svelte";
  import PublishLibrary from "./PublishLibrary.svelte";

  let libraryChooserOpen = $state(false);
  let publishOpen = $state(false);

  function toggleTag(t: string) {
    store.selectedTags = store.selectedTags.includes(t)
      ? store.selectedTags.filter((x) => x !== t)
      : [...store.selectedTags, t];
  }

  const HOST_FILTERS = [
    { key: "claude-code", label: "Claude Code" },
    { key: "agents", label: "Codex / Amp" },
    { key: "cursor", label: "Cursor" },
    { key: "copilot", label: "Copilot" },
  ];

  const counts = $derived.by(() => {
    const tags = new Map<string, number>();
    const cats = new Map<string, number>();
    const hosts = new Map<string, number>();
    let uncategorized = 0;
    let agnostic = 0;
    for (const s of store.skills) {
      for (const t of s.tags) tags.set(t, (tags.get(t) ?? 0) + 1);
      if (s.category) cats.set(s.category, (cats.get(s.category) ?? 0) + 1);
      else uncategorized++;
      if (s.hosts.length === 0) agnostic++;
      for (const k of new Set(s.hosts.map(hostKind))) if (k) hosts.set(k, (hosts.get(k) ?? 0) + 1);
    }
    return { tags, cats, hosts, uncategorized, agnostic };
  });

</script>

<aside class="sidebar">
  <section>
    <h3>Bibliothèque</h3>
    {#if store.config?.library_path}
      <div class="path selectable" title={store.config.library_path}>{store.config.library_path}</div>
    {/if}
    {#if store.library}
      <div class="muted">{store.skills.length} {store.skills.length > 1 ? KIND_LABEL[store.kind].many : KIND_LABEL[store.kind].one}</div>
    {:else if store.kind === "skill"}
      <div class="muted">Aucune bibliothèque configurée.</div>
    {:else}
      <div class="muted">Dossier d'agents introuvable. Par défaut : <code>agents/</code> dans la bibliothèque.</div>
    {/if}
    <div class="row" style="margin-top:6px">
      <button class="small" disabled={store.loading} onclick={() => (libraryChooserOpen = true)}>Ouvrir / Cloner…</button>
      {#if store.kind === "agent" && store.config?.agents_path}
        <button class="small" title="Revenir à agents/ dans la bibliothèque" onclick={() => store.setAgentsLibrary(null)}>Par défaut</button>
      {/if}
      <button class="small" disabled={!store.library || store.loading} onclick={() => store.run("Rechargé", () => store.refreshLibrary())}>Recharger</button>
    </div>
    <button class="small" style="margin-top:6px" disabled={!store.config?.library_path || store.loading} onclick={() => (publishOpen = true)}>
      Publier…{#if store.unpublishedCount > 0} <span class="unknown-count" title="Skills et agents avec des modifications ou des commits à publier">{store.unpublishedCount}</span>{/if}
    </button>
    <button class="small" style="margin-top:6px" disabled={!store.config?.library_path || store.loading} onclick={() => (store.gitDialog = {})}>Récupérer…</button>
    <button class="small" style="margin-top:6px" disabled={!store.config?.library_path} onclick={() => (store.registryOpen = true)}>
      Tags et catégories…{#if store.unknownCount > 0} <span class="unknown-count" title="Valeurs absentes du référentiel">{store.unknownCount}</span>{/if}
    </button>
    {#if store.kind === "agent" && store.config?.agents_path}
      <div class="muted path" style="margin-top:6px">Dossier d’agents personnalisé : {store.config.agents_path}</div>
    {/if}
  </section>

  {#if store.library}
    <section>
      <h3>Catégories</h3>
      <ul class="facets">
        <li>
          <button class:active={store.category === null} onclick={() => (store.category = null)}>
            <span>Toutes</span><span class="count">{store.skills.length}</span>
          </button>
        </li>
        {#each store.library.categories as c (c)}
          <li>
            <button class:active={store.category === c} onclick={() => (store.category = store.category === c ? null : c)}>
              <span>{c}</span><span class="count">{counts.cats.get(c) ?? 0}</span>
            </button>
          </li>
        {/each}
        {#if counts.uncategorized > 0 && store.library.categories.length > 0}
          <li>
            <button class:active={store.category === ""} onclick={() => (store.category = store.category === "" ? null : "")}>
              <span class="muted">Sans catégorie</span><span class="count">{counts.uncategorized}</span>
            </button>
          </li>
        {/if}
      </ul>
    </section>

    <section>
      <h3>Tags</h3>
      {#if store.library.tags.length === 0}
        <div class="muted">Aucun tag pour l'instant.</div>
      {/if}
      <div class="wrap">
        {#each store.library.tags as t (t)}
          <button class="chip" class:active={store.selectedTags.includes(t)} onclick={() => toggleTag(t)}>
            {t} <span class="muted">{counts.tags.get(t) ?? 0}</span>
          </button>
        {/each}
      </div>
      {#if store.selectedTags.length > 0}
        <button class="small" style="margin-top:6px" onclick={() => (store.selectedTags = [])}>Effacer les tags</button>
      {/if}
    </section>

    {#if counts.hosts.size > 0}
      <section>
        <h3>Harnais</h3>
        <ul class="facets">
          <li>
            <button class:active={store.host === null} onclick={() => (store.host = null)}>
              <span>Tous</span><span class="count">{store.skills.length}</span>
            </button>
          </li>
          <li>
            <button class:active={store.host === "any"} onclick={() => (store.host = store.host === "any" ? null : "any")}>
              <span>Universels</span><span class="count">{counts.agnostic}</span>
            </button>
          </li>
          {#each HOST_FILTERS as h (h.key)}
            {#if counts.hosts.has(h.key)}
              <li>
                <button class:active={store.host === h.key} onclick={() => (store.host = store.host === h.key ? null : h.key)}>
                  <span>{h.label}</span><span class="count">{counts.hosts.get(h.key)}</span>
                </button>
              </li>
            {/if}
          {/each}
        </ul>
      </section>
    {/if}

    {#if store.library.warnings.length > 0}
      <section>
        <h3>Avertissements</h3>
        {#each store.library.warnings as w}
          <div class="warn selectable" title={w.path}>{w.path.split("/").pop()} : {warningText(w)}</div>
        {/each}
      </section>
    {/if}
  {/if}
</aside>

{#if libraryChooserOpen}
  <LibraryChooser onclose={() => (libraryChooserOpen = false)} />
{/if}
{#if publishOpen}
  <PublishLibrary onclose={() => { publishOpen = false; store.refreshGitStates("skill"); store.refreshGitStates("agent"); }} />
{/if}

<style>
  .unknown-count {
    color: var(--warn);
    font-weight: 600;
  }
  .sidebar {
    width: 240px;
    flex: none;
    border-right: 1px solid var(--border);
    background: var(--panel);
    overflow-y: auto;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .path {
    font-family: var(--mono);
    font-size: 11px;
    word-break: break-all;
  }
  .facets {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .facets button {
    width: 100%;
    display: flex;
    justify-content: space-between;
    background: none;
    border: none;
    padding: 4px 6px;
    border-radius: 6px;
    text-align: left;
  }
  .facets button:hover {
    background: var(--panel-2);
  }
  .facets button.active {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .count {
    color: var(--muted);
    font-size: 11px;
  }
  .chip {
    cursor: pointer;
  }
  .chip.active .muted {
    color: #fff;
  }
  .warn {
    color: var(--warn);
    font-size: 11px;
    margin-bottom: 4px;
  }
  section h3 {
    margin-bottom: 6px;
  }
</style>
