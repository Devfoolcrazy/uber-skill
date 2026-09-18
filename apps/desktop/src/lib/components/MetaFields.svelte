<script lang="ts">
  import { store } from "$lib/store.svelte";

  /// Category and tags inputs. With a registry they only offer allowed values,
  /// while values already on the item stay visible and removable even when unknown.
  let { category = $bindable(""), tags = $bindable("") }: { category: string; tags: string } = $props();

  const parse = (text: string) => [...new Set(text.split(",").map((t) => t.trim().toLowerCase()).filter(Boolean))];
  const selected = $derived(parse(tags));
  const allowedCategories = $derived(store.usages("category").filter((u) => u.known).map((u) => u.value));
  const allowedTags = $derived(store.usages("tag").filter((u) => u.known).map((u) => u.value));
  const unknownTags = $derived(selected.filter((t) => !allowedTags.includes(t)));

  function toggle(tag: string) {
    const next = selected.includes(tag) ? selected.filter((t) => t !== tag) : [...selected, tag];
    tags = next.sort().join(", ");
  }
</script>

{#if store.registry?.exists}
  <label>Catégorie
    <select bind:value={category}>
      <option value="">Aucune</option>
      {#each allowedCategories as c}<option value={c}>{c}</option>{/each}
      {#if category && !allowedCategories.includes(category)}<option value={category}>{category} (inconnue)</option>{/if}
    </select>
  </label>
  <fieldset>
    <legend>Tags</legend>
    <div class="wrap">
      {#each allowedTags as t}
        <button type="button" class="chip toggle" class:on={selected.includes(t)} aria-pressed={selected.includes(t)} onclick={() => toggle(t)}>{t}</button>
      {/each}
      {#each unknownTags as t}
        <button type="button" class="chip toggle on unknown" aria-pressed="true" title="Absent du référentiel : cliquer pour le retirer" onclick={() => toggle(t)}>{t} (inconnu)</button>
      {/each}
      {#if allowedTags.length === 0 && unknownTags.length === 0}<span class="muted">Aucun tag dans le référentiel.</span>{/if}
    </div>
  </fieldset>
  <button type="button" class="link" onclick={() => (store.registryOpen = true)}>Gérer les tags et catégories…</button>
{:else}
  <div class="row">
    <label>Catégorie <input type="text" bind:value={category} list="meta-categories" /></label>
    <label>Tags <input type="text" bind:value={tags} placeholder="a, b, c" /></label>
  </div>
  <datalist id="meta-categories">
    {#each store.library?.categories ?? [] as c}<option value={c}></option>{/each}
  </datalist>
{/if}

<style>
  label { display: flex; flex-direction: column; gap: 2px; flex: 1; font-size: 11px; color: var(--muted); }
  input, select { min-width: 0; }
  fieldset { border: 0; padding: 0; margin: 0; min-width: 0; }
  legend { padding: 0; font-size: 11px; color: var(--muted); margin-bottom: 4px; }
  .toggle { cursor: pointer; border: 1px solid var(--border); background: none; color: var(--muted); }
  .toggle.on { background: var(--accent-soft); color: var(--text); border-color: var(--accent); }
  .toggle.unknown { border-color: var(--warn); color: var(--warn); }
  .link { align-self: flex-start; border: none; background: none; padding: 0; color: var(--accent); font-size: 11px; cursor: pointer; }
</style>
