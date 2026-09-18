<script lang="ts">
  import { onMount } from "svelte";
  import { api, type Facet, type RegistryView, type ValueUsage } from "$lib/api";
  import { errorText } from "$lib/errors";
  import { store } from "$lib/store.svelte";

  let { onclose }: { onclose: () => void } = $props();
  let dialog: HTMLDialogElement;
  let busy = $state(false);
  let error = $state<string | null>(null);
  /// The one row being renamed, removed or mapped, with its draft value.
  let action = $state<{ facet: Facet; value: string; mode: "rename" | "remove" | "map"; draft: string } | null>(null);
  let added = $state<Record<Facet, string>>({ category: "", tag: "" });

  const FACETS: { facet: Facet; title: string; one: string }[] = [
    { facet: "category", title: "Catégories", one: "catégorie" },
    { facet: "tag", title: "Tags", one: "tag" },
  ];
  /// Values are always trimmed, so a leading space cannot collide with one.
  const STRIP = " strip";

  onMount(() => { dialog.showModal(); store.refreshRegistry(); });

  const known = (facet: Facet) => store.usages(facet).filter((u) => u.known);
  const unknown = (facet: Facet) => store.usages(facet).filter((u) => !u.known);
  const isActive = (facet: Facet, u: ValueUsage, mode: string) =>
    action?.facet === facet && action.value === u.value && action.mode === mode;
  const uses = (u: ValueUsage) => (u.items.length === 0 ? "inutilisé" : `${u.items.length} élément(s)`);
  const usedBy = (u: ValueUsage) => u.items.map((i) => i.id).join(", ");

  /// `rewrites`: the operation edits skills and agents on disk, which an unsaved editor would overwrite.
  async function apply(rewrites: boolean, run: () => Promise<RegistryView>) {
    if (busy || (rewrites && store.editorDirty)) return;
    busy = true; error = null;
    try {
      store.registry = await run();
      action = null;
      if (rewrites) await store.refreshAfterGit();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  function add(facet: Facet) {
    const value = added[facet].trim();
    if (value) apply(false, async () => { const r = await api.registryAdd(facet, value); added[facet] = ""; return r; });
  }

  function confirmAction() {
    if (!action) return;
    const { facet, value, mode, draft } = action;
    const used = store.usages(facet).some((u) => u.value === value && u.items.length > 0);
    if (mode === "remove") {
      if (used && !draft) return;
      apply(used, () => api.registryRemove(facet, value, used && draft !== STRIP ? draft : null, used && draft === STRIP));
    } else if (draft.trim()) {
      apply(used, () => api.registryRename(facet, value, draft.trim()));
    }
  }
</script>

<dialog bind:this={dialog} oncancel={(e) => { e.preventDefault(); if (!busy) onclose(); }} aria-labelledby="registry-title">
  <h2 id="registry-title">Bibliothèque : tags et catégories</h2>
  {#if store.registry}
    <p class="muted selectable">{store.registry.path}</p>
    {#if !store.registry.exists}
      <p>Cette bibliothèque n’a pas encore de référentiel : toutes les valeurs sont acceptées. Le créer limite les choix proposés dans l’éditeur et signale les valeurs inconnues, sans jamais bloquer un skill ou un agent.</p>
      <button class="primary" disabled={busy} onclick={() => apply(false, api.registryInit)}>Créer le référentiel à partir des valeurs utilisées</button>
    {/if}
    {#if store.editorDirty}
      <p class="warning">Des modifications ne sont pas enregistrées dans l’éditeur. Enregistrez-les avant de renommer, remplacer ou retirer une valeur utilisée.</p>
    {/if}
    {#each FACETS as { facet, title, one }}
      <section aria-label={title}>
        <h3>{title}</h3>
        <ul>
          {#each known(facet) as u (u.value)}
            <li>
              <div class="line">
                <span class="value">{u.value}</span>
                <span class="muted" title={usedBy(u)}>{uses(u)}</span>
                <span class="spacer"></span>
                {#if store.registry.exists}
                  <button class="small" disabled={busy} aria-label={`Renommer ${u.value}`} onclick={() => (action = { facet, value: u.value, mode: "rename", draft: u.value })}>Renommer</button>
                  <button class="small" disabled={busy} aria-label={`Supprimer ${u.value}`} onclick={() => (action = { facet, value: u.value, mode: "remove", draft: "" })}>Supprimer</button>
                {/if}
              </div>
              {#if isActive(facet, u, "rename") && action}
                <form class="line panel" onsubmit={(e) => { e.preventDefault(); confirmAction(); }}>
                  <input type="text" bind:value={action.draft} aria-label={`Nouveau nom pour ${u.value}`} />
                  <button type="submit" class="small primary" disabled={busy || !action.draft.trim() || (u.items.length > 0 && store.editorDirty)}>Renommer partout</button>
                  <button type="button" class="small" onclick={() => (action = null)}>Annuler</button>
                </form>
                {#if u.items.length}<p class="muted">Met à jour {usedBy(u)}. Un nom déjà existant fusionne les deux valeurs.</p>{/if}
              {/if}
              {#if isActive(facet, u, "remove") && action}
                <form class="line panel" onsubmit={(e) => { e.preventDefault(); confirmAction(); }}>
                  {#if u.items.length}
                    <select bind:value={action.draft} aria-label={`Que faire des éléments utilisant ${u.value}`}>
                      <option value="">Choisir pour {u.items.length} élément(s)…</option>
                      {#each known(facet).filter((k) => k.value !== u.value) as k}<option value={k.value}>Remplacer par {k.value}</option>{/each}
                      <option value={STRIP}>Retirer {one === "tag" ? "ce tag" : "cette catégorie"} des éléments</option>
                    </select>
                  {:else}
                    <span>Supprimer « {u.value} » du référentiel ?</span>
                  {/if}
                  <button type="submit" class="small danger" disabled={busy || (u.items.length > 0 && (!action.draft || store.editorDirty))}>Supprimer</button>
                  <button type="button" class="small" onclick={() => (action = null)}>Annuler</button>
                </form>
                {#if u.items.length}<p class="muted">Utilisé par {usedBy(u)}.</p>{/if}
              {/if}
            </li>
          {:else}
            <li class="muted">Aucune valeur.</li>
          {/each}
        </ul>
        {#if unknown(facet).length}
          <h4>Valeurs inconnues</h4>
          <ul>
            {#each unknown(facet) as u (u.value)}
              <li>
                <div class="line">
                  <span class="value unknown">{u.value}</span>
                  <span class="muted" title={usedBy(u)}>{uses(u)}</span>
                  <span class="spacer"></span>
                  <button class="small" disabled={busy} aria-label={`Ajouter ${u.value} au référentiel`} onclick={() => apply(false, () => api.registryAdd(facet, u.value))}>Ajouter au référentiel</button>
                  <button class="small" disabled={busy || known(facet).length === 0} aria-label={`Remplacer ${u.value}`} onclick={() => (action = { facet, value: u.value, mode: "map", draft: "" })}>Remplacer…</button>
                </div>
                {#if isActive(facet, u, "map") && action}
                  <form class="line panel" onsubmit={(e) => { e.preventDefault(); confirmAction(); }}>
                    <select bind:value={action.draft} aria-label={`Valeur autorisée remplaçant ${u.value}`}>
                      <option value="">Choisir une valeur autorisée…</option>
                      {#each known(facet) as k}<option value={k.value}>{k.value}</option>{/each}
                    </select>
                    <button type="submit" class="small primary" disabled={busy || !action.draft || store.editorDirty}>Remplacer dans {u.items.length} élément(s)</button>
                    <button type="button" class="small" onclick={() => (action = null)}>Annuler</button>
                  </form>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
        <form class="line" onsubmit={(e) => { e.preventDefault(); add(facet); }}>
          <input type="text" bind:value={added[facet]} placeholder={`Nouvelle valeur (${one})`} aria-label={`Ajouter une valeur (${one})`} />
          <button type="submit" class="small" disabled={busy || !added[facet].trim()}>Ajouter</button>
        </form>
      </section>
    {/each}
    <p class="muted">Les changements sont enregistrés sur disque, dans le référentiel et dans les skills et agents concernés. Utilisez « Publier… » pour les partager.</p>
  {:else}
    <p class="muted">Aucune bibliothèque ouverte.</p>
  {/if}
  {#if error}<p class="error selectable" role="alert">{error}</p>{/if}
  <div class="footer"><button disabled={busy} onclick={onclose}>Fermer</button></div>
</dialog>

<style>
  dialog { width: min(640px, calc(100vw - 40px)); max-height: calc(100vh - 40px); overflow-y: auto; padding: 22px; color: var(--text); background: var(--panel); border: 1px solid var(--border); border-radius: var(--radius); }
  dialog::backdrop { background: rgba(0, 0, 0, 0.45); }
  h2 { margin: 0 0 12px; font-size: 18px; }
  h3 { margin: 18px 0 6px; font-size: 14px; }
  h4 { margin: 12px 0 4px; font-size: 12px; color: var(--warn); }
  p { margin: 10px 0; overflow-wrap: anywhere; }
  ul { list-style: none; margin: 0 0 8px; padding: 0; }
  li { padding: 4px 0; border-bottom: 1px solid var(--border); }
  .line { display: flex; align-items: center; gap: 8px; }
  .panel { margin: 6px 0; padding: 8px; background: var(--panel-2); border-radius: 6px; flex-wrap: wrap; }
  .panel input, .panel select, section > .line input { flex: 1; min-width: 0; }
  .value { font-family: var(--mono); }
  .unknown { color: var(--warn); }
  .warning { color: var(--warn); }
  .error { color: var(--danger); white-space: pre-wrap; }
  .footer { display: flex; justify-content: flex-end; margin-top: 18px; }
</style>
