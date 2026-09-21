<script lang="ts">
  import type { ItemGitState } from "$lib/api";

  /// Difference between an item and the tracked remote branch. `full` spells it out.
  let { state, full = false }: { state: ItemGitState | undefined; full?: boolean } = $props();

  const parts = $derived(
    [
      state?.modified && { glyph: "✎", cls: "modified", short: "Modifié", text: "Modifications locales non publiées" },
      state?.unpublished && { glyph: "↑", cls: "unpublished", short: "À envoyer", text: "Commit local pas encore envoyé au dépôt distant" },
      state?.outdated && { glyph: "↓", cls: "outdated", short: "À récupérer", text: "Version plus récente sur le dépôt distant (d’après la dernière récupération)" },
    ].filter((p) => !!p),
  );
</script>

{#each parts as p}
  <span class="git {p.cls}" class:full title={p.text} aria-label={p.text} role="img">{p.glyph}{#if full} {p.short}{/if}</span>
{/each}

<style>
  .git { font-size: 11px; line-height: 1; font-weight: 600; flex: none; }
  .git.full { padding: 3px 7px; border-radius: 999px; border: 1px solid currentColor; font-weight: 500; }
  .modified, .unpublished { color: var(--warn); }
  .outdated { color: var(--info); }
</style>
