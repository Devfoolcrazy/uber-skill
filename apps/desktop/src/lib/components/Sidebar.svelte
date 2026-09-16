<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { store } from "$lib/store.svelte";
  import { hostKind, KIND_LABEL } from "$lib/api";

  async function pickLibrary() {
    const dir = await open({
      directory: true,
      multiple: false,
      title: store.kind === "skill" ? "Choisir le dossier de la bibliothèque" : "Choisir le dossier des agents",
    });
    if (typeof dir !== "string") return;
    if (store.kind === "skill") await store.setLibrary(dir);
    else await store.setAgentsLibrary(dir);
  }

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
    <h3>{store.kind === "skill" ? "Bibliothèque" : "Agents"}</h3>
    {#if store.library}
      <div class="path selectable" title={store.library.root}>{store.library.root}</div>
      <div class="muted">{store.skills.length} {store.skills.length > 1 ? KIND_LABEL[store.kind].many : KIND_LABEL[store.kind].one}</div>
    {:else if store.kind === "skill"}
      <div class="muted">Aucune bibliothèque configurée.</div>
    {:else}
      <div class="muted">Dossier d'agents introuvable. Par défaut : <code>agents/</code> dans la bibliothèque.</div>
    {/if}
    <div class="row" style="margin-top:6px">
      <button class="small" onclick={pickLibrary}>Choisir…</button>
      {#if store.kind === "agent" && store.config?.agents_path}
        <button class="small" title="Revenir à agents/ dans la bibliothèque" onclick={() => store.setAgentsLibrary(null)}>Par défaut</button>
      {/if}
      <button class="small" disabled={!store.library} onclick={() => store.run("Rechargé", () => store.refreshLibrary())}>Recharger</button>
    </div>
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
          <div class="warn selectable" title={w.path}>{w.message}</div>
        {/each}
      </section>
    {/if}
  {/if}
</aside>

<style>
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
