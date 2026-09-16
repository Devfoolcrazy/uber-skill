<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api } from "$lib/api";
  import { store } from "$lib/store.svelte";

  let { onclose }: { onclose: () => void } = $props();
  let id = $state("");
  let description = $state("");
  let category = $state("");
  let tags = $state("");
  let hosts = $state("");

  const idOk = $derived(/^[a-z0-9]+(-[a-z0-9]+)*$/.test(id) && id.length <= 64);

  async function create() {
    const s = await store.run(`Skill ${id} créé`, () =>
      api.createSkill(
        id,
        description,
        category || null,
        tags.split(",").map((t) => t.trim()).filter(Boolean),
        hosts.split(",").map((t) => t.trim()).filter(Boolean),
      ),
    );
    if (s) {
      store.replaceSkill(s);
      store.selectedId = s.id;
      onclose();
    }
  }

  async function importDir() {
    const dir = await open({ directory: true, multiple: false, title: "Importer un dossier de skill" });
    if (typeof dir !== "string") return;
    const s = await store.run("Skill importé", () => api.importSkill(dir, null));
    if (s) {
      store.replaceSkill(s);
      store.selectedId = s.id;
      onclose();
    }
  }
</script>

<form class="new" onsubmit={(e) => { e.preventDefault(); if (idOk) create(); }}>
  <input type="text" placeholder="identifiant (ex: review-pr)" bind:value={id} class:bad={id && !idOk} />
  <input type="text" placeholder="description" bind:value={description} />
  <div class="row">
    <input type="text" placeholder="catégorie" bind:value={category} list="cats" />
    <input type="text" placeholder="tags, séparés, par, virgules" bind:value={tags} />
  </div>
  <input type="text" placeholder="harnais requis (vide = universel) : claude-code, codex…" bind:value={hosts} />
  <datalist id="cats">
    {#each store.library?.categories ?? [] as c}<option value={c}></option>{/each}
  </datalist>
  <div class="row">
    <button type="submit" class="small primary" disabled={!idOk}>Créer</button>
    <button type="button" class="small" onclick={importDir}>Importer un dossier…</button>
    <span class="spacer"></span>
    <button type="button" class="small" onclick={onclose}>Annuler</button>
  </div>
</form>

<style>
  .new {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px;
    border-bottom: 1px solid var(--border);
    background: var(--panel-2);
  }
  .row input {
    flex: 1;
    min-width: 0;
  }
  .bad {
    border-color: var(--danger);
  }
</style>
