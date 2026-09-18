<script lang="ts">
  import { onMount } from "svelte";
  import { api, type RefineProposal, type Skill } from "$lib/api";
  import { errorText } from "$lib/errors";

  let { skill, onaccepted, onclose }: { skill: Skill; onaccepted: (s: Skill) => void; onclose: () => void } = $props();
  let dialog: HTMLDialogElement;
  let instruction = $state("");
  let proposal = $state<RefineProposal | null>(null);
  let busy = $state<"propose" | "accept" | null>(null);
  let error = $state<string | null>(null);
  /// Bumped to ignore the answer of a request the user gave up on.
  let attempt = 0;

  const SUGGESTIONS = [
    "Clarifie les instructions et lève les ambiguïtés.",
    "Structure les instructions en étapes et ajoute un exemple d’utilisation.",
    "Améliore la description pour préciser quand utiliser ce skill.",
    "Raccourcis le texte sans perdre d’information utile.",
  ];

  onMount(() => dialog.showModal());

  async function propose() {
    if (busy || !instruction.trim()) return;
    const mine = ++attempt;
    busy = "propose"; error = null; proposal = null;
    try {
      const next = await api.refinePropose(skill.kind, skill.id, instruction);
      if (mine === attempt) proposal = next;
    } catch (e) {
      if (mine === attempt) error = errorText(e);
    } finally {
      if (mine === attempt) busy = null;
    }
  }

  /// The CLI call cannot be interrupted; its answer is simply discarded.
  function giveUp() {
    attempt++;
    busy = null;
  }

  async function accept() {
    if (busy || !proposal) return;
    busy = "accept"; error = null;
    try {
      onaccepted(await api.refineAccept(skill.kind, proposal));
      onclose();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = null;
    }
  }
</script>

<dialog bind:this={dialog} oncancel={(e) => { e.preventDefault(); if (busy !== "accept") { giveUp(); onclose(); } }} aria-labelledby="refine-title">
  <h2 id="refine-title">Raffiner {skill.id} avec Claude</h2>
  {#if !proposal}
    <p class="muted">Claude reçoit uniquement le texte de <code>SKILL.md</code> tel qu’enregistré sur disque ; les scripts et autres fichiers du skill ne sont pas analysés. Il peut proposer un nouveau corps et une nouvelle description ; le nom, les tags, la catégorie et les autres clés du frontmatter sont verrouillés. Rien n’est modifié avant votre acceptation.</p>
    <label>Que doit améliorer Claude ?
      <textarea rows="3" bind:value={instruction} disabled={!!busy} placeholder="Ex. : clarifie les étapes et ajoute un exemple d’utilisation"></textarea>
    </label>
    <div class="wrap">
      {#each SUGGESTIONS as s}<button type="button" class="chip suggestion" disabled={!!busy} onclick={() => (instruction = s)}>{s}</button>{/each}
    </div>
    {#if busy === "propose"}<p role="status">Claude rédige une proposition… Cela prend généralement de 10 à 60 secondes.</p>{/if}
  {:else}
    <p class="muted selectable">Consigne : {instruction}</p>
    {#if proposal.diff}
      <pre class="diff selectable" aria-label="Différences proposées">{#each proposal.diff.split("\n") as line}<span class={line.startsWith("+") ? "add" : line.startsWith("-") ? "del" : line.startsWith("@@") ? "hunk" : ""}>{line}{"\n"}</span>{/each}</pre>
    {:else}
      <p>Claude ne propose aucun changement.</p>
    {/if}
    {#if proposal.restored_keys.length}
      <p class="muted">Clés du frontmatter modifiées par Claude et rétablies : {proposal.restored_keys.join(", ")}.</p>
    {/if}
    <p class="muted">Accepter enregistre le fichier localement, sans commit ni push. Une proposition bien rédigée ne garantit pas le comportement du skill : essayez-le dans un projet.</p>
  {/if}
  {#if error}<p class="error selectable" role="alert">{error}</p>{/if}
  <div class="footer">
    {#if !proposal}
      <button onclick={() => { giveUp(); onclose(); }}>Fermer</button>
      {#if busy === "propose"}
        <button onclick={giveUp}>Abandonner</button>
      {:else}
        <button class="primary" disabled={!instruction.trim()} onclick={propose}>Proposer</button>
      {/if}
    {:else}
      <button disabled={!!busy} onclick={() => (proposal = null)}>Modifier la consigne</button>
      <span class="spacer"></span>
      <button disabled={!!busy} onclick={onclose}>Rejeter</button>
      <button class="primary" disabled={!!busy || !proposal.diff} onclick={accept}>Accepter</button>
    {/if}
  </div>
</dialog>

<style>
  dialog { width: min(900px, calc(100vw - 40px)); max-height: calc(100vh - 40px); overflow-y: auto; padding: 22px; color: var(--text); background: var(--panel); border: 1px solid var(--border); border-radius: var(--radius); }
  dialog::backdrop { background: rgba(0, 0, 0, 0.45); }
  h2 { margin: 0 0 12px; font-size: 18px; }
  p { margin: 12px 0; overflow-wrap: anywhere; }
  label { display: flex; flex-direction: column; gap: 6px; }
  textarea { resize: vertical; }
  .suggestion { cursor: pointer; border: 1px solid var(--border); background: none; color: var(--muted); margin-top: 8px; }
  .diff { max-height: 50vh; overflow: auto; margin: 0; padding: 10px; font: 12px/1.5 var(--mono); background: var(--panel-2); border-radius: 6px; white-space: pre-wrap; overflow-wrap: anywhere; }
  .add { color: var(--ok); }
  .del { color: var(--danger); }
  .hunk { color: var(--info); }
  .error { color: var(--danger); white-space: pre-wrap; }
  .footer { display: flex; align-items: center; justify-content: flex-end; gap: 8px; margin-top: 18px; }
</style>
