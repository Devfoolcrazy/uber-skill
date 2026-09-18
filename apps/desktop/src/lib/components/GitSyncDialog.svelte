<script lang="ts">
  import { onMount } from "svelte";
  import { api, SOURCE_LABEL, type GitSyncStatus, type InstallationPlan, type InstallRequest } from "$lib/api";
  import { store } from "$lib/store.svelte";

  let { request, onclose }: { request?: InstallRequest; onclose: () => void } = $props();
  let dialog: HTMLDialogElement;
  let status = $state<GitSyncStatus | null>(null);
  let plan = $state<InstallationPlan | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let success = $state<string | null>(null);
  let acceptHosts = $state(false);
  const canUpdate = $derived(!!status?.verified && !status.blocked && status.behind > 0 && status.ahead === 0 && !store.editorDirty);
  const canInstall = $derived(!!plan && (plan.warnings.length === 0 || acceptHosts));

  onMount(() => { dialog.showModal(); refresh(); });

  async function load() {
    acceptHosts = false;
    if (request) {
      plan = await api.prepareInstall(request.kind, request.ids, request.target);
      status = plan.git;
    } else {
      status = await api.checkLibraryGit();
    }
  }

  async function refresh() {
    busy = true; error = null; success = null; plan = null; status = null;
    try { await load(); } catch (e) { error = String(e); }
    finally { busy = false; }
  }

  async function performInstall(prepared: InstallationPlan) {
    if (!request) return;
    await api.installSkills(prepared, request.project);
    store.checked = new Set();
    await store.refreshProject();
    store.notify(`${prepared.items.length} élément(s) installé(s)`);
    onclose();
  }

  async function installLocal() {
    if (busy || !plan || !canInstall) return;
    busy = true; error = null;
    try { await performInstall(plan); } catch (e) { error = String(e); }
    finally { busy = false; }
  }

  async function update() {
    if (busy || !status || !canUpdate || (request && !canInstall)) return;
    busy = true; error = null; success = null;
    const acceptedWarnings = plan?.warnings.join("\n") ?? "";
    const accepted = acceptHosts;
    try {
      status = await api.updateLibraryGit(status.snapshot);
      await store.refreshAfterGit();
      success = "Bibliothèque mise à jour.";
      if (request) {
        // Re-resolve the selected IDs and host requirements after checkout.
        plan = null;
        await load();
        if (plan && status?.verified && !status.blocked && status.behind === 0) {
          const next = plan as InstallationPlan;
          if (next.warnings.length === 0 || (accepted && next.warnings.join("\n") === acceptedWarnings)) {
            await performInstall(next);
          } else {
            success = "Bibliothèque mise à jour. Vérifiez les nouveaux avertissements avant d’installer.";
          }
        }
      }
    } catch (e) {
      error = String(e);
      // Never fall back to a local installation after a failed update.
    } finally { busy = false; }
  }
</script>

<dialog bind:this={dialog} oncancel={(e) => { e.preventDefault(); if (!busy) onclose(); }} aria-labelledby="sync-title">
  <h2 id="sync-title">{request ? "Vérifier avant installation" : "Récupérer les changements Git"}</h2>
  {#if request}<p class="selectable">{request.ids.join(", ")} → {request.project}</p>{/if}
  {#if status}
    <p>{status.branch ?? "HEAD détachée"} → {status.remote ?? "aucun dépôt distant"} / {status.remote_ref?.replace("refs/heads/", "") ?? "aucune branche de suivi"}</p>
    {#if status.verified}
      <p>{status.behind} commit(s) distant(s) à récupérer · {status.ahead} commit(s) local(aux) à publier.</p>
      {#if status.behind === 0 && !status.blocked}<p class="success">La bibliothèque contient les derniers changements distants.</p>{/if}
    {:else}
      <p class="warning">La fraîcheur distante n’a pas pu être vérifiée. {request ? "Vous pouvez choisir explicitement d’installer la version locale." : "Vérifiez le réseau et la configuration Git, puis réessayez."}</p>
    {/if}
    {#if status.fetch_error}<p class="error selectable">{status.fetch_error}</p>{/if}
    {#if status.blocked}<p class="warning" role="alert">{status.blocked}</p>{/if}
    <details>
      <summary>{status.changed_files.length} fichier(s) avec des changements locaux</summary>
      <ul>{#each status.changed_files as path}<li class="selectable">{path}</li>{/each}</ul>
    </details>
  {/if}
  {#if plan?.git_error}
    <p class="warning">La fraîcheur distante n’a pas pu être vérifiée. Une installation locale reste possible.</p>
    <p class="error selectable">{plan.git_error}</p>
  {/if}
  {#if store.editorDirty}
    <p class="warning">Des modifications ne sont pas enregistrées dans l’éditeur. L’installation utilise les fichiers sur disque. Enregistrez vos modifications avant de mettre à jour la bibliothèque.</p>
  {/if}
  {#if plan}
    <ul>{#each plan.items as item}<li><strong>{item.id}</strong> — {SOURCE_LABEL[item.source_state]}</li>{/each}</ul>
    {#if plan.warnings.length}
      <div class="warning">{#each plan.warnings as warning}<p>{warning}</p>{/each}</div>
      <label><input type="checkbox" bind:checked={acceptHosts} disabled={busy} /> Installer malgré ces incompatibilités de harnais</label>
    {/if}
  {/if}
  {#if busy}<p role="status">Vérification ou mise à jour en cours…</p>{/if}
  {#if error}<p class="error selectable" role="alert">{error}</p>{/if}
  {#if success}<p class="success" role="status">{success}</p>{/if}
  <div class="footer">
    <button disabled={busy} onclick={refresh}>Vérifier à nouveau</button>
    <button disabled={busy} onclick={onclose}>Annuler</button>
    {#if request}<button disabled={busy || !canInstall} onclick={installLocal}>Installer la version locale</button>{/if}
    <button class="primary" disabled={busy || !canUpdate || (!!request && !canInstall)} onclick={update}>{request ? "Mettre à jour puis installer" : "Mettre à jour la bibliothèque"}</button>
  </div>
</dialog>

<style>
  dialog { width: min(720px, calc(100vw - 40px)); max-height: calc(100vh - 40px); overflow-y: auto; padding: 22px; color: var(--text); background: var(--panel); border: 1px solid var(--border); border-radius: var(--radius); }
  dialog::backdrop { background: rgba(0, 0, 0, 0.45); }
  h2 { margin: 0 0 12px; font-size: 18px; }
  p { margin: 12px 0; overflow-wrap: anywhere; }
  .footer { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 8px; margin-top: 18px; }
  .warning { color: var(--warn); }
  .error { color: var(--danger); white-space: pre-wrap; }
  .success { color: var(--ok); }
  li { overflow-wrap: anywhere; }
</style>
