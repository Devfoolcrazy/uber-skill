<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api } from "$lib/api";
  import { store } from "$lib/store.svelte";
  import MetaFields from "./MetaFields.svelte";

  let { onclose }: { onclose: () => void } = $props();
  let id = $state("");
  let description = $state("");
  let category = $state("");
  let tags = $state("");
  let hosts = $state("");

  const idOk = $derived(/^[a-z0-9]+(-[a-z0-9]+)*$/.test(id) && id.length <= 64);

  async function create() {
    const s = await store.run(`Brouillon ${id} créé : il n’est pas encore publié`, () =>
      api.createSkill(store.kind, 
        id,
        description,
        category || null,
        tags.split(",").map((t) => t.trim()).filter(Boolean),
        hosts.split(",").map((t) => t.trim()).filter(Boolean),
      ),
    );
    if (s) {
      store.replaceSkill(s);
      await store.refreshRegistry();
      store.editRequest = s.id;
      store.selectedId = s.id;
      onclose();
    }
  }

  async function importDir() {
    const dir =
      store.kind === "skill"
        ? await open({ directory: true, multiple: false, title: "Importer un dossier de skill" })
        : await open({ multiple: false, title: "Importer un fichier d'agent", filters: [{ name: "Markdown", extensions: ["md"] }] });
    if (typeof dir !== "string") return;
    const s = await store.run(store.kind === "skill" ? "Skill importé" : "Agent importé", () => api.importSkill(store.kind, dir, null));
    if (s) {
      store.replaceSkill(s);
      await store.refreshRegistry();
      store.selectedId = s.id;
      onclose();
    }
  }
</script>

<form class="new" onsubmit={(e) => { e.preventDefault(); if (idOk) create(); }}>
  <strong>{store.kind === "skill" ? "Nouveau skill" : "Nouvel agent"}</strong>
  <input type="text" placeholder={store.kind === "skill" ? "identifiant (ex: review-pr)" : "identifiant (ex: reviewer)"} bind:value={id} class:bad={id && !idOk} />
  <input type="text" placeholder="description" bind:value={description} />
  <MetaFields bind:category bind:tags />
  <input type="text" placeholder="harnais requis (vide = universel) : claude-code, codex…" bind:value={hosts} />
  {#if id && !idOk}<div class="hint bad-text">Minuscules, chiffres et tirets uniquement, 64 caractères au plus.</div>{/if}
  <div class="hint">Créé à partir du modèle intégré (objectif, instructions, exemple d’utilisation), puis ouvert dans l’éditeur. Rien n’est publié tant que vous n’utilisez pas « Publier… ».</div>
  <div class="row">
    <button type="submit" class="small primary" disabled={!idOk}>Créer et éditer</button>
    <button type="button" class="small" onclick={importDir}>{store.kind === "skill" ? "Importer un dossier…" : "Importer un fichier…"}</button>
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
  .hint {
    font-size: 11px;
    color: var(--muted);
  }
  .bad-text {
    color: var(--danger);
  }
  .bad {
    border-color: var(--danger);
  }
</style>
