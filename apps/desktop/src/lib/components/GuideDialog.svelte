<script lang="ts">
  import { onMount } from "svelte";
  import DOMPurify from "dompurify";
  import { marked } from "marked";
  import guide from "$lib/guide.md?raw";

  let { onclose }: { onclose: () => void } = $props();
  let dialog: HTMLDialogElement;
  const html = DOMPurify.sanitize(marked.parse(guide, { async: false }) as string);

  onMount(() => dialog.showModal());
</script>

<dialog bind:this={dialog} oncancel={(e) => { e.preventDefault(); onclose(); }} aria-label="Mode d’emploi">
  <div class="bar">
    <span class="muted">Ce guide décrit l’application telle qu’elle est ; il est embarqué avec elle.</span>
    <span class="spacer"></span>
    <button class="small" onclick={onclose}>Fermer</button>
  </div>
  <article class="md selectable">{@html html}</article>
</dialog>

<style>
  dialog { width: min(820px, calc(100vw - 40px)); max-height: calc(100vh - 40px); overflow-y: auto; padding: 18px 26px 26px; color: var(--text); background: var(--panel); border: 1px solid var(--border); border-radius: var(--radius); }
  dialog::backdrop { background: rgba(0, 0, 0, 0.45); }
  .bar { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; position: sticky; top: -18px; padding: 8px 0; background: var(--panel); }
  article :global(h1) { font-size: 22px; margin: 6px 0 12px; }
  article :global(h2) { font-size: 15px; margin: 22px 0 8px; }
  article :global(p), article :global(li) { line-height: 1.55; }
  article :global(table) { border-collapse: collapse; margin: 10px 0; font-size: 13px; }
  article :global(th), article :global(td) { border: 1px solid var(--border); padding: 6px 10px; text-align: left; vertical-align: top; }
  article :global(th) { background: var(--panel-2); }
  article :global(code) { font: 12px var(--mono); background: var(--panel-2); padding: 1px 4px; border-radius: 4px; }
</style>
