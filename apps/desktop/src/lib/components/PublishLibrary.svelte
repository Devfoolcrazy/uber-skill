<script lang="ts">
  import { onMount } from "svelte";
  import { api, type PublicationPreview, type PublicationResult } from "$lib/api";
  import { BLOCK_LABEL, errorText } from "$lib/errors";
  import { store } from "$lib/store.svelte";

  let { onclose }: { onclose: () => void } = $props();
  let dialog: HTMLDialogElement;
  let preview = $state<PublicationPreview | null>(null);
  let selected = $state<string[]>([]);
  let active = $state<string | null>(null);
  let message = $state("Mettre à jour la bibliothèque de skills et agents");
  let busy = $state(false);
  let error = $state<string | null>(null);
  let result = $state<PublicationResult | null>(null);
  const activeFile = $derived(preview?.files.find((f) => f.path === active));
  const canPublish = $derived(!busy && !!preview && !preview.blocked &&
    (selected.length > 0 ? message.trim().length > 0 : preview.pending_count > 0));
  const labels: Record<string, string> = { A: "Ajout", M: "Modification", D: "Suppression", T: "Type modifié" };

  onMount(() => { dialog.showModal(); refresh(); });

  async function refresh(keepResult = false) {
    if (busy) return;
    busy = true;
    error = null;
    if (!keepResult) result = null;
    try {
      const next = await api.publicationPreview();
      selected = selected.filter((p) => next.files.some((f) => f.path === p));
      preview = next;
      if (!next.files.some((f) => f.path === active)) active = next.files[0]?.path ?? null;
    } catch (e) {
      preview = null;
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  function toggle(path: string) {
    selected = selected.includes(path) ? selected.filter((p) => p !== path) : [...selected, path];
  }

  async function publish() {
    if (!canPublish || !preview) return;
    busy = true;
    error = null;
    result = null;
    try {
      result = await api.publishLibrary(preview.snapshot, selected, message);
      selected = [];
      if (result.pushed) store.notify("Bibliothèque publiée");
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
    // A successful commit changes HEAD even when the network push fails.
    if (result) await refresh(true);
  }
</script>

<dialog bind:this={dialog} oncancel={(e) => { e.preventDefault(); if (!busy) onclose(); }} aria-labelledby="publish-title">
  <h2 id="publish-title">Publier la bibliothèque</h2>
  <p>Choisissez les fichiers enregistrés sur disque à inclure dans le commit.</p>
  {#if store.editorDirty}
    <p class="warning">L’éditeur contient des modifications non enregistrées. Fermez cette fenêtre et enregistrez-les pour les inclure.</p>
  {/if}
  {#if preview}
    <p class="muted selectable">{preview.branch ?? "HEAD détachée"} → {preview.remote ?? "aucun dépôt distant"}/{preview.remote_branch ?? "aucune branche de suivi"}</p>
    {#if preview.blocked}<p class="warning" role="alert">{BLOCK_LABEL[preview.blocked]}</p>{/if}
    {#if preview.pending_count > 0}
      <details class="pending">
        <summary>{preview.pending_count} commit(s) local(aux) seront également envoyés</summary>
        <p class="muted">Selon le dernier état distant connu. Un push refusé ne forcera pas la mise à jour distante.</p>
        <ul>{#each preview.pending_commits as commit}<li class="selectable">{commit}</li>{/each}</ul>
        {#if preview.pending_count > preview.pending_commits.length}<p class="muted">Les 20 premiers commits sont affichés.</p>{/if}
      </details>
    {/if}
    {#if preview.files.length > 0}
      <div class="row selection">
        <button disabled={busy} onclick={() => (selected = preview!.files.map((f) => f.path))}>Tout sélectionner</button>
        <button disabled={busy || !selected.length} onclick={() => (selected = [])}>Tout désélectionner</button>
        <span>{selected.length} / {preview.files.length} fichier(s)</span>
      </div>
      <div class="review">
        <div class="files">
          {#each preview.files as file (file.path)}
            <div class="file" class:active={active === file.path}>
              <input type="checkbox" aria-label={`Inclure ${file.path}`} disabled={busy} checked={selected.includes(file.path)} onchange={() => toggle(file.path)} />
              <button disabled={busy} onclick={() => (active = file.path)} title={file.path}>
                <span class="kind">{labels[file.status] ?? file.status}</span>
                <span class="filename">{file.path}</span>
              </button>
            </div>
          {/each}
        </div>
        <div class="diff">
          <h3>{activeFile?.path ?? "Aperçu"}</h3>
          <pre class="selectable">{activeFile?.diff || "Aucune différence textuelle."}</pre>
        </div>
      </div>
      <p class="muted">La sélection porte sur le fichier entier. Un renommage apparaît comme une suppression et un ajout : cochez les deux.</p>
      <label>Message de commit
        <textarea rows="2" bind:value={message} disabled={busy}></textarea>
      </label>
    {:else}
      <p>Aucun fichier modifié à committer.</p>
    {/if}
  {/if}
  {#if busy}<p role="status">Opération Git en cours…</p>{/if}
  {#if error}<p class="error selectable" role="alert">{error}</p>{/if}
  {#if result?.pushed}<p class="success" role="status">Publication réussie{result.commit ? ` — commit ${result.commit.slice(0, 8)}` : ""}.</p>{/if}
  {#if result && !result.pushed}
    <p class="warning" role="alert">{result.commit ? `Le commit ${result.commit.slice(0, 8)} a été créé localement, mais son envoi a échoué.` : "L’envoi des commits a échoué."} Vous pouvez réessayer avec « Envoyer les commits » sans créer un nouveau commit.</p>
    <pre class="error selectable">{errorText(result.push_error)}</pre>
  {/if}
  <div class="footer">
    <button disabled={busy} onclick={() => refresh()}>Actualiser</button>
    <span class="spacer"></span>
    <button disabled={busy} onclick={onclose}>Fermer</button>
    <button class="primary" disabled={!canPublish} onclick={publish}>{selected.length ? "Commit et push" : "Envoyer les commits"}</button>
  </div>
</dialog>

<style>
  dialog { width: min(1000px, calc(100vw - 40px)); max-height: calc(100vh - 40px); overflow-y: auto; padding: 22px; color: var(--text); background: var(--panel); border: 1px solid var(--border); border-radius: var(--radius); }
  dialog::backdrop { background: rgba(0, 0, 0, 0.45); }
  h2 { margin: 0 0 12px; font-size: 18px; }
  p { margin: 12px 0; }
  .pending { padding: 10px; background: var(--panel-2); border-radius: 6px; }
  .selection { margin: 12px 0; flex-wrap: wrap; }
  .review { display: grid; grid-template-columns: minmax(200px, 32%) minmax(0, 1fr); height: 310px; border: 1px solid var(--border); }
  .files, .diff { overflow: auto; }
  .files { border-right: 1px solid var(--border); }
  .file { display: flex; align-items: center; gap: 5px; padding: 5px; }
  .file.active { background: var(--accent-soft); }
  .file button { border: none; background: none; text-align: left; min-width: 0; flex: 1; }
  .kind { display: block; font-size: 11px; color: var(--muted); }
  .filename { overflow-wrap: anywhere; }
  .diff { padding: 10px; }
  h3 { overflow-wrap: anywhere; margin: 0 0 10px; }
  pre { font: 12px/1.5 var(--mono); margin: 0; }
  label { display: flex; flex-direction: column; gap: 6px; }
  textarea { resize: vertical; }
  .footer { display: flex; align-items: center; gap: 8px; margin-top: 18px; }
  .warning { color: var(--warn); }
  .success { color: var(--ok); }
  .error { color: var(--danger); white-space: pre-wrap; overflow-wrap: anywhere; }
  @media (max-width: 650px) { .review { grid-template-columns: 1fr; height: auto; } .files { max-height: 150px; } .diff { max-height: 260px; } }
</style>
