<script lang="ts">
  import { store, DRIFT_LABEL } from "$lib/store.svelte";
  import NewSkillForm from "./NewSkillForm.svelte";

  let showNew = $state(false);
  const list = $derived(store.filtered);
  const allChecked = $derived(list.length > 0 && list.every((s) => store.checked.has(s.id)));

  function toggleAll() {
    store.checked = allChecked ? new Set() : new Set(list.map((s) => s.id));
  }
</script>

<div class="list">
  <div class="toolbar">
    <input type="search" placeholder="Rechercher un {store.kind === 'skill' ? 'skill' : 'agent'} (nom, tag, description)…" bind:value={store.query} />
    <button class="small primary" title="Nouveau {store.kind === 'skill' ? 'skill' : 'agent'}" onclick={() => (showNew = !showNew)}>+ {store.kind === "skill" ? "Nouveau skill" : "Nouvel agent"}</button>
  </div>
  {#if showNew}
    <NewSkillForm onclose={() => (showNew = false)} />
  {/if}
  <div class="row head">
    <input type="checkbox" checked={allChecked} onchange={toggleAll} disabled={list.length === 0} />
    <span class="muted">{list.length} résultat{list.length > 1 ? "s" : ""}</span>
    <span class="spacer"></span>
    {#if store.checked.size > 0}
      <span class="muted">{store.checked.size} coché{store.checked.size > 1 ? "s" : ""}</span>
    {/if}
  </div>
  <ul>
    {#each list as s (s.id)}
      {@const st = store.statusOf(s.id)}
      <li class:selected={store.selectedId === s.id}>
        <input type="checkbox" checked={store.checked.has(s.id)} onchange={() => store.toggleChecked(s.id)} />
        <button class="item" onclick={() => (store.selectedId = s.id)}>
          <div class="row">
            <span class="name">{s.id}</span>
            {#if st}<span class="dot {st.state}" title={DRIFT_LABEL[st.state]}></span>{/if}
            <span class="spacer"></span>
            {#if s.category}<span class="chip cat">{s.category}</span>{/if}
            {#if s.hosts.length}<span class="chip host" title="Harnais : {s.hosts.join(', ')}">⌘ {s.hosts.join(", ")}</span>{/if}
          </div>
          <div class="desc">{s.description}</div>
          {#if s.tags.length}
            <div class="wrap">
              {#each s.tags as t}<span class="chip">{t}</span>{/each}
            </div>
          {/if}
        </button>
      </li>
    {/each}
  </ul>
  {#if store.library && list.length === 0}
    <div class="empty muted">Aucun {store.kind === "skill" ? "skill" : "agent"} ne correspond.</div>
  {/if}
</div>

<style>
  .list {
    width: 340px;
    flex: none;
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    background: var(--panel);
    min-height: 0;
  }
  .toolbar {
    display: flex;
    gap: 6px;
    padding: 10px;
    border-bottom: 1px solid var(--border);
  }
  .toolbar input {
    flex: 1;
  }
  .head {
    padding: 6px 12px;
    border-bottom: 1px solid var(--border);
    font-size: 12px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    flex: 1;
  }
  li {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    padding: 8px 10px 8px 12px;
    border-bottom: 1px solid var(--border);
  }
  li:hover {
    background: var(--panel-2);
  }
  li.selected {
    background: var(--accent-soft);
  }
  li input {
    margin-top: 3px;
  }
  .item {
    flex: 1;
    background: none;
    border: none;
    padding: 0;
    text-align: left;
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .chip.host {
    color: var(--warn);
  }
  .name {
    font-weight: 600;
    font-family: var(--mono);
    font-size: 12px;
  }
  .desc {
    color: var(--muted);
    font-size: 12px;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .empty {
    padding: 20px;
    text-align: center;
  }
</style>
