<script lang="ts">
  import type { InstalledSkill } from "$lib/api";
  import { DRIFT_LABEL } from "$lib/store.svelte";

  /// A globe coloured by the state of the copy installed globally: one glyph says
  /// « available in every project » and how that copy stands against the library.
  let { copy, full = false }: { copy: InstalledSkill | undefined; full?: boolean } = $props();
  const text = $derived(copy ? `Installé globalement (~/.claude, ~/.agents) · ${DRIFT_LABEL[copy.state]}` : "");
</script>

{#if copy}
  <span class="globe {copy.state}" class:full title={text} aria-label={text} role="img">🌐{#if full} Global · {DRIFT_LABEL[copy.state]}{/if}</span>
{/if}

<style>
  .globe { font-size: 12px; line-height: 1; flex: none; filter: grayscale(1) opacity(0.55); }
  .globe.full { padding: 3px 7px; border-radius: 999px; border: 1px solid var(--border); font-size: 11px; color: var(--muted); }
  .globe.up-to-date { filter: hue-rotate(-75deg) saturate(1.3); }
  .globe.library-updated, .globe.project-modified { filter: sepia(1) saturate(4) hue-rotate(-10deg); }
  .globe.full.library-updated, .globe.full.project-modified { color: var(--warn); border-color: var(--warn); }
  .globe.conflict, .globe.missing, .globe.source-missing { filter: sepia(1) saturate(6) hue-rotate(-50deg); }
  .globe.full.conflict, .globe.full.missing { color: var(--danger); border-color: var(--danger); }
</style>
