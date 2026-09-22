<script lang="ts">
  import { onDestroy } from "svelte";
  import DOMPurify from "dompurify";
  import { marked } from "marked";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, splitFrontmatter, type Issue, type Skill } from "$lib/api";
  import { issueText, SEVERITY_LABEL } from "$lib/lint";
  import { store, DRIFT_LABEL } from "$lib/store.svelte";
  import GitBadge from "./GitBadge.svelte";
  import GlobalBadge from "./GlobalBadge.svelte";
  import InstalledIn from "./InstalledIn.svelte";
  import MetaFields from "./MetaFields.svelte";
  import RefineDialog from "./RefineDialog.svelte";

  type Tab = "preview" | "edit" | "files" | "lint";
  let tab = $state<Tab>("preview");
  let skill = $derived(store.selected);

  // Editor state
  let currentFile = $state("SKILL.md");
  let text = $state("");
  let original = $state("");
  let loadedFor = $state<string | null>(null);
  const dirty = $derived(text !== original);

  onDestroy(() => { store.editorDirty = false; });

  // Metadata editing
  let tagsDraft = $state("");
  let categoryDraft = $state("");
  let descDraft = $state("");
  let hostsDraft = $state("");
  let metaOpen = $state(false);
  const metaDirty = $derived(metaOpen && !!skill && (
    tagsDraft !== skill.tags.join(", ") || categoryDraft !== (skill.category ?? "") ||
    descDraft !== skill.description || hostsDraft !== skill.hosts.join(", ")
  ));
  $effect(() => { store.editorDirty = dirty || metaDirty; });

  let issues = $state<Issue[] | null>(null);
  let refineOpen = $state(false);
  const installed = $derived(skill ? store.statusOf(skill.id) : undefined);

  async function installHere() {
    if (!skill || !store.projectPath) return;
    store.requestInstall([skill.id]);
  }

  const mainFile = (s: Skill) => (s.kind === "skill" ? "SKILL.md" : s.files[0]);
  const isMain = $derived(skill ? currentFile === mainFile(skill) : false);
  const isText = (f: string) => /\.(md|txt|json|ya?ml|toml|py|sh|js|ts|rs|csv|html|css|xml|mjs|cjs|svelte|sql)$/i.test(f) || !f.includes(".");

  async function load(s: Skill, file: string) {
    const t = await api.readSkillFile(store.kind, s.id, file).catch((e) => {
      store.fail(e);
      return "";
    });
    currentFile = file;
    text = t;
    original = t;
    loadedFor = s.id;
  }

  $effect(() => {
    const s = skill;
    if (!s) return;
    if (loadedFor !== s.id) {
      issues = null;
      metaOpen = false;
      load(s, mainFile(s));
      if (store.editRequest === s.id) {
        tab = "edit";
        store.editRequest = null;
      }
    }
  });

  $effect(() => {
    if (tab === "lint" && skill && issues === null) {
      api.lintSkill(store.kind, skill.id).then((r) => (issues = r)).catch(store.fail);
    }
  });

  const isMarkdown = $derived(/\.(md|markdown)$/i.test(currentFile));
  const preview = $derived.by(() => {
    if (!skill || !isMarkdown) return "";
    const body = isMain ? splitFrontmatter(text).body : text;
    // A skill may come from anywhere, and this window can run the app's commands:
    // no script, event handler or embedded frame from its markdown reaches the page.
    return DOMPurify.sanitize(marked.parse(body, { async: false }) as string, { FORBID_TAGS: ["style", "form"] });
  });

  /// Path of a relative link inside the skill, from the file being previewed; null when it escapes the skill.
  function linkedFile(href: string): string | null {
    const target = decodeURIComponent(href.split(/[?#]/)[0]);
    const parts = currentFile.split("/").slice(0, -1);
    for (const part of target.split("/")) {
      if (part === "" || part === ".") continue;
      if (part === "..") {
        if (parts.length === 0) return null;
        parts.pop();
      } else parts.push(part);
    }
    return parts.join("/");
  }

  /// Links of the preview never navigate the app window: a file of the skill opens
  /// here, a web address opens in the browser.
  async function followLink(e: MouseEvent) {
    const anchor = (e.target as HTMLElement).closest("a");
    const href = anchor?.getAttribute("href");
    if (!anchor || !href || !skill) return;
    e.preventDefault();
    if (href.startsWith("#")) {
      document.getElementById(decodeURIComponent(href.slice(1)))?.scrollIntoView({ behavior: "smooth" });
    } else if (/^[a-z][a-z0-9+.-]*:/i.test(href)) {
      if (/^(https?|mailto):/i.test(href)) await openUrl(href).catch(store.fail);
    } else {
      const file = href.startsWith("/") ? null : linkedFile(href);
      if (!file || !skill.files.includes(file)) store.fail(`Lien introuvable dans ce skill : ${href}`);
      else if (dirty) store.fail("Enregistrez vos modifications avant d’ouvrir un autre fichier.");
      else await load(skill, file);
    }
  }

  async function save() {
    if (!skill || !dirty) return;
    const s = await store.run("Enregistré", () => api.writeSkillFile(store.kind, skill!.id, currentFile, text));
    if (s) {
      original = text;
      store.replaceSkill(s);
      issues = null;
      await projectCopyChanged(s.id);
    }
  }

  /// A saved change leaves the copy installed in the current project behind: say so right away.
  async function projectCopyChanged(id: string) {
    await store.refreshProject().catch(store.fail);
    const behind = store.installationsOf(id).filter((c) => c.item.state === "library-updated");
    const elsewhere = behind.filter((c) => c.project.path !== store.projectPath);
    if (elsewhere.length > 0) {
      const n = new Set(behind.map((c) => c.project.path)).size;
      store.notify(`Enregistré. ${n} projet${n > 1 ? "s ont" : " a"} une copie en retard : voir « Installé dans ».`);
    } else if (store.statusOf(id)?.state === "library-updated") {
      store.notify(`Enregistré. La copie dans ${store.projectName} est en retard : « Mettre à jour la copie du projet ».`);
    }
  }

  async function refined(s: Skill) {
    store.replaceSkill(s);
    issues = null;
    await load(s, mainFile(s));
    await projectCopyChanged(s.id);
  }

  async function fixIssue(i: Issue) {
    if (!skill || !i.fix || dirty) return;
    const fix = i.fix;
    const s = await store.run(`Lien corrigé dans ${fix.file}`, () => api.applyLintFix(store.kind, skill!.id, fix));
    if (s) {
      store.replaceSkill(s);
      issues = null;
      await load(s, currentFile);
      await projectCopyChanged(s.id);
    }
  }

  async function allow(i: Issue) {
    const facet = i.code === "tag-unknown" ? "tag" : "category";
    const view = await store.run(`« ${i.args[0]} » ajouté au référentiel`, () => api.registryAdd(facet, i.args[0]));
    if (view) {
      store.registry = view;
      issues = null;
    }
  }

  function openMeta() {
    if (!skill) return;
    tagsDraft = skill.tags.join(", ");
    categoryDraft = skill.category ?? "";
    descDraft = skill.description;
    hostsDraft = skill.hosts.join(", ");
    metaOpen = true;
  }

  async function saveMeta() {
    if (!skill) return;
    const s = await store.run("Métadonnées enregistrées", () =>
      api.updateMeta(store.kind, skill!.id, {
        tags: tagsDraft.split(",").map((t) => t.trim()).filter(Boolean),
        category: categoryDraft.trim() || null,
        set_category: true,
        hosts: hostsDraft.split(",").map((t) => t.trim()).filter(Boolean),
        description: descDraft.trim() !== skill!.description ? descDraft.trim() : undefined,
      }),
    );
    if (s) {
      store.replaceSkill(s);
      await store.refreshRegistry();
      await projectCopyChanged(s.id);
      metaOpen = false;
      if (isMain) await load(s, mainFile(s));
      issues = null;
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === "s") {
      e.preventDefault();
      save();
    }
  }
</script>

<svelte:window onkeydown={onkeydown} />

{#if !skill}
  <div class="detail empty muted">
    {#if store.library}Sélectionne un {store.kind === "skill" ? "skill" : "agent"} dans la liste.{:else}Ouvrez d’abord une bibliothèque (« Ouvrir / Cloner… »). Le bouton « ? » en haut à droite explique l’application.{/if}
  </div>
{:else}
  <div class="detail">
    <header>
      <div class="row">
        <h2 class="selectable">{skill.id}</h2>
        {#if skill.category}<span class="chip cat" class:unknown={!store.isKnown("category", skill.category)} title={store.isKnown("category", skill.category) ? undefined : "Catégorie absente du référentiel"}>{skill.category}</span>{/if}
        <GitBadge state={store.gitStateOf(skill.id)} full />
        {#if !store.isGlobal}<GlobalBadge copy={store.globalStateOf(skill.id)} full />{/if}
      </div>
      <div class="row actions">
        {#if store.projectPath}
          {#if !installed}
            <button class="small primary" disabled={!store.targetOk} title={store.targetOk ? "" : "Cette cible ne gère pas les agents"} onclick={installHere}>{store.isGlobal ? "Installer globalement" : `Installer dans ${store.projectName}`}</button>
          {:else}
            {#if installed.state === "library-updated"}
              <button class="small primary" disabled={!store.targetOk} title="La bibliothèque contient une version plus récente que la copie installée dans ce projet" onclick={installHere}>Mettre à jour la copie du projet</button>
            {/if}
            <button class="small" onclick={() => (store.drawerOpen = true)} title={`${DRIFT_LABEL[installed.state]} · voir les éléments installés dans ce projet`}>
              <span class="dot {installed.state}"></span> Installés
            </button>
            <button class="small" title="Supprime la copie installée dans ce projet ; l’élément reste dans la bibliothèque" onclick={() => store.uninstall(skill!.id)}>{store.isGlobal ? "Retirer…" : "Retirer du projet…"}</button>
          {/if}
        {/if}
        <button class="small" onclick={() => api.openInEditor(skill!.path).catch(store.fail)}>Ouvrir dans l'éditeur</button>
        <button class="small" onclick={openMeta}>Tags & catégorie</button>
        {#if skill.kind === "skill"}
          <button class="small" disabled={dirty || metaDirty} title={dirty || metaDirty ? "Enregistrez vos modifications avant de raffiner" : "Proposer une amélioration de SKILL.md avec Claude"} onclick={() => (refineOpen = true)}>Raffiner…</button>
        {/if}
        <button class="small danger" title="Supprime l’élément de la bibliothèque elle-même, pas seulement d’un projet" onclick={() => store.deleteFromLibrary(skill!.id)}>Supprimer de la bibliothèque…</button>
      </div>
      <p class="desc selectable">{skill.description}</p>
      <InstalledIn {skill} />
      <div class="wrap">
        {#each skill.tags as t}<span class="chip" class:unknown={!store.isKnown("tag", t)} title={store.isKnown("tag", t) ? undefined : "Tag absent du référentiel"}>{t}</span>{/each}
        {#if skill.hosts.length}
          <span class="chip host" title="Ce skill dépend de ce(s) harnais">harnais : {skill.hosts.join(", ")}</span>
        {:else}
          <span class="chip" title="Aucune dépendance de harnais déclarée">universel</span>
        {/if}
        {#each Object.entries(skill.extra) as [k, v]}<span class="chip" title={v}>{k}: {v.length > 30 ? v.slice(0, 30) + "…" : v}</span>{/each}
      </div>
      {#if metaOpen}
        <form class="meta" onsubmit={(e) => { e.preventDefault(); saveMeta(); }}>
          <label>Description <input type="text" bind:value={descDraft} /></label>
          <MetaFields bind:category={categoryDraft} bind:tags={tagsDraft} />
          <label>Harnais requis <input type="text" bind:value={hostsDraft} placeholder="vide = universel ; sinon claude-code, codex, cursor, copilot" /></label>
          <div class="row">
            <button type="submit" class="small primary">Enregistrer</button>
            <button type="button" class="small" onclick={() => (metaOpen = false)}>Annuler</button>
          </div>
        </form>
      {/if}
      <nav class="tabs">
        <button class:active={tab === "preview"} onclick={() => (tab = "preview")}>Aperçu</button>
        <button class:active={tab === "edit"} onclick={() => (tab = "edit")}>Éditer {dirty ? "•" : ""}</button>
        {#if skill.kind === "skill"}
          <button class:active={tab === "files"} onclick={() => (tab = "files")}>Fichiers ({skill.files.length})</button>
        {/if}
        <button class:active={tab === "lint"} onclick={() => (tab = "lint")}>Lint</button>
        <span class="spacer"></span>
        {#if tab === "edit"}
          <span class="muted"><code>{currentFile}</code></span>
          <button class="small primary" disabled={!dirty} onclick={save}>Enregistrer (⌘S)</button>
        {/if}
      </nav>
    </header>

    <div class="body">
      {#if tab === "preview"}
        {#if skill && !isMain}
          <button class="small back" onclick={() => { if (dirty) store.fail("Enregistrez vos modifications avant d’ouvrir un autre fichier."); else load(skill!, mainFile(skill!)); }}>← {mainFile(skill)}</button>
          <span class="muted selectable">{currentFile}</span>
        {/if}
        {#if !isMarkdown}
          <pre class="file">{text}</pre>
        {:else}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
          <article class="md selectable" onclick={followLink}>{@html preview}</article>
        {/if}
      {:else if tab === "edit"}
        {#if isText(currentFile)}
          <textarea bind:value={text} spellcheck="false"></textarea>
        {:else}
          <div class="muted">Fichier binaire, non éditable ici.</div>
        {/if}
      {:else if tab === "files"}
        <ul class="files">
          {#each skill.files as f}
            <li>
              <button class:active={currentFile === f} disabled={!isText(f)} onclick={() => { load(skill!, f); tab = "edit"; }}>{f}</button>
            </li>
          {/each}
        </ul>
      {:else if tab === "lint"}
        {#if issues === null}
          <div class="muted">Analyse…</div>
        {:else if issues.length === 0}
          <div class="ok">Aucun problème détecté.</div>
        {:else}
          <ul class="issues">
            {#each issues as i}
              <li class={i.severity}>
                <span class="sev">{SEVERITY_LABEL[i.severity]}</span><code>{i.rule}</code><span class="selectable">{issueText(i)}</span>
                {#if i.fix}
                  <button class="small" disabled={dirty} title={dirty ? "Enregistrez vos modifications avant de corriger" : `Remplacer ${i.fix.from} par ${i.fix.to} dans ${i.fix.file}`} onclick={() => fixIssue(i)}>Corriger</button>
                {:else if i.code === "tag-unknown" || i.code === "category-unknown"}
                  <button class="small" onclick={() => allow(i)}>Ajouter au référentiel</button>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
        <button class="small" style="margin-top:8px" onclick={() => { issues = null; api.lintSkill(store.kind, skill!.id).then((r) => (issues = r)).catch(store.fail); }}>Relancer</button>
      {/if}
    </div>
  </div>
{/if}

{#if refineOpen && skill}
  <RefineDialog {skill} onaccepted={refined} onclose={() => (refineOpen = false)} />
{/if}

<style>
  .detail {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }
  .empty {
    align-items: center;
    justify-content: center;
    display: flex;
  }
  header {
    padding: 12px 16px 0;
    border-bottom: 1px solid var(--border);
    background: var(--panel);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  h2 {
    font-family: var(--mono);
    overflow-wrap: anywhere;
  }
  header .row {
    flex-wrap: wrap;
  }
  .actions :global(button) {
    white-space: nowrap;
  }
  .desc {
    margin: 0;
    color: var(--muted);
  }
  .back {
    margin: 0 8px 10px 0;
  }
  .chip.unknown {
    outline: 1px dashed var(--warn);
    color: var(--warn);
  }
  .chip.host {
    color: var(--warn);
  }
  .meta {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px;
    background: var(--panel-2);
    border-radius: var(--radius);
  }
  .meta label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    font-size: 11px;
    color: var(--muted);
  }
  .tabs {
    display: flex;
    gap: 2px;
    align-items: center;
  }
  .tabs > button:not(.small) {
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    border-radius: 0;
    padding: 6px 10px;
  }
  .tabs > button.active {
    border-bottom-color: var(--accent);
    color: var(--accent);
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 16px;
    display: flex;
    flex-direction: column;
  }
  textarea {
    flex: 1;
    width: 100%;
    resize: none;
    font-family: var(--mono);
    font-size: 12.5px;
    line-height: 1.5;
    padding: 12px;
    tab-size: 2;
  }
  .md {
    max-width: 820px;
    line-height: 1.55;
  }
  .md :global(h1) {
    font-size: 20px;
    margin: 0 0 12px;
  }
  .md :global(h2) {
    font-size: 16px;
    margin: 20px 0 8px;
  }
  .md :global(h3) {
    font-size: 13px;
    text-transform: none;
    letter-spacing: 0;
    color: var(--text);
    margin: 14px 0 6px;
  }
  .md :global(pre) {
    background: var(--panel-2);
    padding: 10px;
    border-radius: 6px;
    overflow-x: auto;
  }
  .md :global(a) {
    color: var(--accent);
  }
  .md :global(table) {
    border-collapse: collapse;
  }
  .md :global(td),
  .md :global(th) {
    border: 1px solid var(--border);
    padding: 3px 8px;
  }
  .file {
    white-space: pre-wrap;
    margin: 0;
  }
  .files {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .files button {
    background: none;
    border: none;
    font-family: var(--mono);
    font-size: 12px;
    padding: 3px 6px;
    text-align: left;
    width: 100%;
  }
  .files button:hover:not(:disabled) {
    background: var(--panel-2);
  }
  .files button.active {
    color: var(--accent);
  }
  .issues {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .issues li {
    display: flex;
    gap: 8px;
    align-items: baseline;
  }
  .sev {
    width: 96px;
    flex: none;
    font-size: 11px;
    text-transform: uppercase;
    font-weight: 600;
  }
  li.error .sev {
    color: var(--danger);
  }
  li.warning .sev {
    color: var(--warn);
  }
  li.info .sev {
    color: var(--info);
  }
  .ok {
    color: var(--ok);
  }
</style>
