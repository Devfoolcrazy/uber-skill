<script lang="ts">
  import { onMount } from "svelte";
  import { confirm, open } from "@tauri-apps/plugin-dialog";
  import { store } from "$lib/store.svelte";

  let { onclose }: { onclose: () => void } = $props();
  let dialog: HTMLDialogElement;
  let mode = $state<"open" | "clone">("open");
  let url = $state("");
  let parent = $state("");
  let name = $state("");
  let picking = $state(false);
  const busy = $derived(store.libraryBusy || picking);
  const nameOk = $derived(name.trim() === name && name.length > 0 && !name.startsWith(".") && !/[\\/:\x00-\x1f\x7f]/.test(name));

  onMount(() => {
    store.error = null;
    dialog.showModal();
  });

  async function canSwitch() {
    if (!store.editorDirty) return true;
    return confirm("Le fichier ouvert contient des modifications non enregistrées. Les abandonner et changer de bibliothèque ?", {
      title: "Modifications non enregistrées", kind: "warning",
    });
  }

  async function chooseLocal() {
    picking = true;
    try {
      const path = await open({ directory: true, multiple: false, title: "Choisir la racine du dépôt Git" });
      if (typeof path !== "string" || !(await canSwitch())) return;
      if (await store.setLibrary(path)) onclose();
    } catch (e) {
      store.fail(e);
    } finally {
      picking = false;
    }
  }

  async function chooseParent() {
    picking = true;
    try {
      const path = await open({ directory: true, multiple: false, title: "Choisir où créer le dossier du dépôt" });
      if (typeof path === "string") parent = path;
    } catch (e) {
      store.fail(e);
    } finally {
      picking = false;
    }
  }

  async function clone() {
    if (busy || !nameOk || !url.trim() || !parent) return;
    picking = true;
    try {
      if (!(await canSwitch())) return;
      if (await store.cloneLibrary(url.trim(), parent, name)) onclose();
    } catch (e) {
      store.fail(e);
    } finally {
      picking = false;
    }
  }
</script>

<dialog bind:this={dialog} oncancel={(e) => { e.preventDefault(); if (!busy) onclose(); }} aria-labelledby="library-title">
  <h2 id="library-title">Ouvrir une bibliothèque Git</h2>
  <p>Le dépôt contient les skills et les agents de la bibliothèque.</p>
  <div class="row modes">
    <button class:primary={mode === "open"} disabled={busy} onclick={() => { mode = "open"; store.error = null; }}>Dépôt local</button>
    <button class:primary={mode === "clone"} disabled={busy} onclick={() => { mode = "clone"; store.error = null; }}>Cloner un dépôt</button>
  </div>
  {#if mode === "open"}
    <p>Sélectionnez la racine d’un dépôt déjà cloné sur cette machine.</p>
    <button class="primary" disabled={busy} onclick={chooseLocal}>Choisir le dépôt…</button>
  {:else}
    <form onsubmit={(e) => { e.preventDefault(); clone(); }}>
      <fieldset disabled={busy}>
        <label>URL du dépôt
          <input type="text" bind:value={url} placeholder="https://… ou git@…" required />
        </label>
        <label>Dossier parent
          <div class="row">
            <input type="text" value={parent} readonly placeholder="Choisir un emplacement…" aria-label="Dossier parent" />
            <button type="button" onclick={chooseParent}>Parcourir…</button>
          </div>
        </label>
        <label>Nom du nouveau dossier
          <input type="text" bind:value={name} placeholder="ma-bibliotheque" required aria-invalid={name.length > 0 && !nameOk} />
        </label>
        {#if name && !nameOk}<p class="error">Utilisez un nom de dossier sans séparateur de chemin et ne commençant pas par un point.</p>{/if}
        {#if parent && nameOk}<p class="destination selectable">Destination : {parent}/{name}</p>{/if}
        <p class="muted">Un nouveau dossier sera créé. Vos accès Git habituels doivent déjà être configurés sur cette machine.</p>
        <button class="primary" type="submit" disabled={!url.trim() || !parent || !nameOk}>Cloner et ouvrir</button>
      </fieldset>
    </form>
  {/if}
  {#if store.config?.agents_path}
    <p class="muted">L’ouverture d’un dépôt remplacera aussi le dossier d’agents personnalisé par celui de cette bibliothèque.</p>
  {/if}
  {#if store.libraryBusy}<p role="status">{mode === "clone" ? "Clonage et chargement en cours…" : "Chargement en cours…"}</p>{/if}
  {#if store.error}<p class="error selectable" role="alert">{store.error}</p>{/if}
  <div class="footer"><button disabled={busy} onclick={onclose}>Fermer</button></div>
</dialog>

<style>
  dialog { width: min(540px, calc(100vw - 40px)); max-height: calc(100vh - 40px); overflow-y: auto; padding: 22px; color: var(--text); background: var(--panel); border: 1px solid var(--border); border-radius: var(--radius); }
  dialog::backdrop { background: rgba(0, 0, 0, 0.45); }
  h2 { margin: 0 0 12px; font-size: 18px; }
  p { margin: 12px 0; }
  .modes { margin: 16px 0; }
  fieldset { border: 0; padding: 0; margin: 0; min-width: 0; display: flex; flex-direction: column; gap: 12px; }
  label { display: flex; flex-direction: column; gap: 6px; }
  input { width: 100%; min-width: 0; }
  .row input { flex: 1; }
  .row button { flex-shrink: 0; }
  .destination { font-family: var(--mono); overflow-wrap: anywhere; }
  .error { color: var(--danger); white-space: pre-wrap; overflow-wrap: anywhere; }
  .footer { display: flex; justify-content: flex-end; margin-top: 20px; }
</style>
