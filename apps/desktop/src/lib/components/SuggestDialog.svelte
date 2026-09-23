<script lang="ts">
  import { onMount } from "svelte";
  import { api, KIND_LABEL, TARGETS, targetKey, targetSupports, type InstallRequest, type ItemKind, type ProjectOverview, type SuggestContext, type Suggestions, type Target } from "$lib/api";
  import { errorText } from "$lib/errors";
  import { store } from "$lib/store.svelte";

  let { project, onclose }: { project: ProjectOverview; onclose: () => void } = $props();
  let dialog: HTMLDialogElement;
  let context = $state<SuggestContext | null>(null);
  let kind = $state("");
  let stack = $state("");
  let goal = $state("");
  /// Where the chosen items go: the target the project already uses, else Claude Code.
  // svelte-ignore state_referenced_locally
  let targetKind = $state<Target["kind"]>(project.installs[0]?.target.kind ?? "claude-code");
  let result = $state<Suggestions | null>(null);
  let checked = $state<Set<string>>(new Set());
  let busy = $state(false);
  let error = $state<string | null>(null);
  /// Bumped to ignore the answer of a request the user gave up on.
  let attempt = 0;

  const CONFIDENCE_LABEL = { high: "confiance élevée", medium: "confiance moyenne", low: "confiance faible" } as const;
  const target = $derived<Target>(TARGETS.find((t) => t.target.kind === targetKind)?.target ?? { kind: "claude-code" });
  const key = (k: ItemKind, id: string) => `${k}:${id}`;
  const selectedCount = $derived(result ? result.suggestions.filter((s) => checked.has(key(s.kind, s.id)) && targetSupports(s.kind, target)).length : 0);

  onMount(async () => {
    dialog.showModal();
    try {
      context = await api.suggestContext(project.path);
    } catch (e) {
      error = errorText(e);
    }
  });

  async function ask() {
    if (busy) return;
    const mine = ++attempt;
    busy = true; error = null; result = null;
    try {
      const next = await api.suggestItems(project.path, { kind: kind.trim() || null, stack: stack.trim() || null, goal: goal.trim() || null });
      if (mine !== attempt) return;
      result = next;
      checked = new Set(next.suggestions.filter((s) => !s.installed).map((s) => key(s.kind, s.id)));
    } catch (e) {
      if (mine === attempt) error = errorText(e);
    } finally {
      if (mine === attempt) busy = false;
    }
  }

  /// The CLI call cannot be interrupted; its answer is simply discarded.
  function giveUp() {
    attempt++;
    busy = false;
  }

  function toggle(k: ItemKind, id: string) {
    const next = new Set(checked);
    const item = key(k, id);
    if (next.has(item)) next.delete(item); else next.add(item);
    checked = next;
  }

  /// One install request per kind, reviewed by the usual install dialog.
  function install() {
    if (!result) return;
    const requests: InstallRequest[] = (["skill", "agent"] as ItemKind[])
      .map((k) => ({ project: project.path, kind: k, target, ids: result!.suggestions.filter((s) => s.kind === k && checked.has(key(k, s.id))).map((s) => s.id) }))
      .filter((r) => r.ids.length > 0 && targetSupports(r.kind, r.target));
    onclose();
    store.requestInstalls(requests);
  }
</script>

<dialog bind:this={dialog} oncancel={(e) => { e.preventDefault(); giveUp(); onclose(); }} aria-labelledby="suggest-title">
  <h2 id="suggest-title">Suggérer des skills pour {project.name}</h2>
  {#if !result}
    {#if context}
      <p class="muted">Claude reçoit l’index de la bibliothèque ({context.catalogue_items} élément{context.catalogue_items > 1 ? "s" : ""}), l’arborescence du projet ({context.tree.length} entrée{context.tree.length > 1 ? "s" : ""}{context.tree_truncated ? ", tronquée" : ""}){#if context.excerpts.length}, le texte de {context.excerpts.map((e) => e.path).join(", ")}{/if} et la liste de ce qui y est déjà installé ({context.installed.length}). Aucun fichier source n’est envoyé. Rien n’est installé sans votre accord.</p>
      {#if context.empty}
        <p class="warning">Ce dossier semble vide ou tout neuf : décrivez le projet ci-dessous pour que Claude ait de quoi choisir.</p>
      {/if}
    {:else if !error}
      <p class="muted" role="status">Lecture du projet…</p>
    {/if}
    <div class="fields">
      <label><span>Type de projet <span class="muted">(facultatif)</span></span>
        <input type="text" bind:value={kind} disabled={busy} placeholder="Ex. : jeu, application de bureau, site web, bibliothèque" />
      </label>
      <label><span>Stack <span class="muted">(facultatif)</span></span>
        <input type="text" bind:value={stack} disabled={busy} placeholder="Ex. : Rust, Tauri, SvelteKit" />
      </label>
      <label><span>Ce que vous allez y faire <span class="muted">(facultatif)</span></span>
        <input type="text" bind:value={goal} disabled={busy} placeholder="Ex. : écrire le lore et les quêtes, refondre l’architecture" />
      </label>
      <label><span>Installer dans</span>
        <select bind:value={targetKind} disabled={busy}>
          {#each TARGETS as t (targetKey(t.target))}<option value={t.target.kind}>{t.label}</option>{/each}
        </select>
      </label>
    </div>
    {#if busy}<p role="status">Claude lit le projet et la bibliothèque… Cela prend généralement de 10 à 60 secondes.</p>{/if}
  {:else}
    {#if result.summary}<p class="selectable">{result.summary}</p>{/if}
    {#if result.suggestions.length === 0}
      <p>Claude ne propose rien pour ce projet.</p>
    {:else}
      <ul class="suggestions" aria-label="Suggestions">
        {#each result.suggestions as s (key(s.kind, s.id))}
          {@const supported = targetSupports(s.kind, target)}
          <li class:dim={s.installed || !supported}>
            <label>
              <input type="checkbox" checked={checked.has(key(s.kind, s.id))} disabled={!supported} onchange={() => toggle(s.kind, s.id)} aria-label={`Installer ${s.id}`} />
              <span class="id">{s.id}</span>
              <span class="chip">{KIND_LABEL[s.kind].one}</span>
              <span class="chip {s.confidence}">{CONFIDENCE_LABEL[s.confidence]}</span>
              {#if s.installed}<span class="chip">déjà installé</span>{/if}
              {#if !supported}<span class="chip" title="Cette cible ne gère pas ce type d’élément">pas pour cette cible</span>{/if}
            </label>
            <p class="reason muted selectable">{s.reason}</p>
          </li>
        {/each}
      </ul>
    {/if}
    {#if result.unknown.length}
      <p class="muted">Ignoré, absent de la bibliothèque : {result.unknown.join(", ")}.</p>
    {/if}
    <p class="muted">Une suggestion est un avis, pas une garantie : la sélection passe par la vérification d’installation habituelle.</p>
  {/if}
  {#if error}<p class="error selectable" role="alert">{error}</p>{/if}
  <div class="footer">
    {#if !result}
      <button onclick={() => { giveUp(); onclose(); }}>Fermer</button>
      {#if busy}
        <button onclick={giveUp}>Abandonner</button>
      {:else}
        <button class="primary" disabled={!context} onclick={ask}>Demander à Claude</button>
      {/if}
    {:else}
      <button onclick={() => (result = null)}>Modifier les réponses</button>
      <span class="spacer"></span>
      <button onclick={onclose}>Fermer</button>
      <button class="primary" disabled={selectedCount === 0} onclick={install}>Installer la sélection ({selectedCount})</button>
    {/if}
  </div>
</dialog>

<style>
  dialog { width: min(760px, calc(100vw - 40px)); max-height: calc(100vh - 40px); overflow-y: auto; padding: 22px; color: var(--text); background: var(--panel); border: 1px solid var(--border); border-radius: var(--radius); }
  dialog::backdrop { background: rgba(0, 0, 0, 0.45); }
  h2 { margin: 0 0 12px; font-size: 18px; }
  p { margin: 12px 0; overflow-wrap: anywhere; }
  .fields { display: flex; flex-direction: column; gap: 10px; }
  .fields label { display: flex; flex-direction: column; gap: 4px; }
  .warning { color: var(--warn); }
  .suggestions { list-style: none; margin: 0; padding: 0; }
  .suggestions li { padding: 8px 0; border-bottom: 1px solid var(--border); }
  .suggestions li.dim { opacity: 0.7; }
  .suggestions label { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; cursor: pointer; }
  .id { font-family: var(--mono); font-weight: 600; }
  .chip.high { color: var(--ok); }
  .chip.low { color: var(--warn); }
  .reason { margin: 4px 0 0 26px; font-size: 13px; }
  .error { color: var(--danger); white-space: pre-wrap; }
  .footer { display: flex; align-items: center; justify-content: flex-end; gap: 8px; margin-top: 18px; }
</style>
