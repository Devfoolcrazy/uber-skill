import {
  api,
  hostKind,
  targetSupports,
  type Config,
  type Facet,
  type InstalledSkill,
  type InstallRequest,
  type ItemGitState,
  type ItemKind,
  type LibraryView,
  type ProjectOverview,
  type RegistryView,
  type Skill,
  type Target,
} from "./api";
import { confirm } from "@tauri-apps/plugin-dialog";
import { errorText } from "./errors";

export const DRIFT_LABEL: Record<string, string> = {
  "up-to-date": "À jour",
  "library-updated": "Copie du projet en retard sur la bibliothèque",
  "project-modified": "Modifié dans le projet",
  conflict: "Conflit",
  untracked: "Non suivi",
  missing: "Dossier manquant",
  "source-missing": "Source introuvable",
};

class AppStore {
  config = $state<Config | null>(null);
  kind = $state<ItemKind>("skill");
  libraries = $state<Record<ItemKind, LibraryView | null>>({ skill: null, agent: null });
  loading = $state(false);
  error = $state<string | null>(null);
  toast = $state<string | null>(null);

  query = $state("");
  selectedTags = $state<string[]>([]);
  category = $state<string | null>(null);
  /// Host filter: null = all, "any" = host-agnostic only, otherwise a Target kind.
  host = $state<string | null>(null);
  selectedId = $state<string | null>(null);
  checked = $state<Set<string>>(new Set());

  projectPath = $state<string | null>(null);
  target = $state<Target>({ kind: "claude-code" });
  projectStatus = $state<Record<ItemKind, InstalledSkill[]>>({ skill: [], agent: [] });
  drawerOpen = $state(false);
  pickerOpen = $state(false);
  libraryBusy = $state(false);
  libraryGeneration = $state(0);
  editorDirty = $state(false);
  gitDialog = $state<{ install?: InstallRequest[] } | null>(null);
  /// Main area: the library (skills or agents), or the followed projects.
  view = $state<"library" | "projects">("library");
  /// Followed projects with what each has installed, across all their targets.
  projects = $state<ProjectOverview[]>([]);

  async refreshProjects() {
    this.projects = this.config?.library_path ? await api.projectsOverview().catch(() => []) : [];
  }

  /// Every installed copy of an item of the current kind, one per project and target.
  installationsOf(id: string, kind: ItemKind = this.kind) {
    return this.projects.flatMap((project) =>
      project.installs
        .filter((i) => i.kind === kind)
        .flatMap((i) => i.items.filter((item) => item.id === id).map((item) => ({ project, target: i.target, item }))),
    );
  }

  /// How many other projects hold a copy of this item that is behind the library.
  behindElsewhere(id: string): number {
    return this.installationsOf(id).filter((c) => c.item.state === "library-updated" && c.project.path !== this.projectPath).length;
  }

  /// Copies simply behind the library, the only ones a bulk update touches.
  get behindTotal(): number {
    return this.projects.reduce((n, p) => n + p.behind, 0);
  }

  requestInstalls(requests: InstallRequest[]) {
    if (requests.length > 0) this.gitDialog = { install: requests };
  }

  /// Allowed categories and tags of the library, merged with the values in use.
  registry = $state<RegistryView | null>(null);
  registryOpen = $state(false);
  /// Items that differ from the tracked remote branch. Decoration only: never blocks anything.
  gitStates = $state<Record<ItemKind, Record<string, ItemGitState>>>({ skill: {}, agent: {} });

  async refreshGitStates(kind: ItemKind = this.kind) {
    this.gitStates[kind] = this.config?.library_path ? await api.libraryGitStates(kind).catch(() => ({})) : {};
  }

  gitStateOf(id: string): ItemGitState | undefined {
    return this.gitStates[this.kind][id];
  }

  /// Items of both kinds with something to publish.
  get unpublishedCount(): number {
    return (["skill", "agent"] as ItemKind[]).reduce(
      (n, k) => n + Object.values(this.gitStates[k]).filter((s) => s.modified || s.unpublished).length,
      0,
    );
  }

  /// Id of a just-created item that the detail view should open straight in the editor.
  editRequest = $state<string | null>(null);

  async refreshRegistry() {
    this.registry = this.config?.library_path ? await api.getRegistry().catch(() => null) : null;
  }

  usages(facet: Facet) {
    return (facet === "category" ? this.registry?.categories : this.registry?.tags) ?? [];
  }

  /// Without a registry every value is accepted.
  isKnown(facet: Facet, value: string): boolean {
    return this.usages(facet).find((u) => u.value === value)?.known ?? !this.registry?.exists;
  }

  get unknownCount(): number {
    return [...this.usages("category"), ...this.usages("tag")].filter((u) => !u.known).length;
  }

  requestInstall(ids: string[]) {
    if (!this.projectPath || !this.targetOk) return;
    this.gitDialog = { install: [{ kind: this.kind, ids, project: this.projectPath, target: this.target }] };
  }

  async refreshAfterGit() {
    await this.refreshLibrary("skill");
    await this.refreshLibrary("agent");
    await this.refreshRegistry();
    this.libraryGeneration++;
    await this.refreshProject();
  }

  private toastTimer: ReturnType<typeof setTimeout> | null = null;

  get library(): LibraryView | null {
    return this.libraries[this.kind];
  }

  get skills(): Skill[] {
    return this.library?.skills ?? [];
  }

  get selected(): Skill | null {
    return this.skills.find((s) => s.id === this.selectedId) ?? null;
  }

  get status(): InstalledSkill[] {
    return this.projectStatus[this.kind];
  }

  /// Whether the current target can host items of the current kind.
  get targetOk(): boolean {
    return targetSupports(this.kind, this.target);
  }

  get filtered(): Skill[] {
    const tokens = this.query.toLowerCase().split(/\s+/).filter(Boolean);
    const scored: [number, Skill][] = [];
    for (const s of this.skills) {
      if (this.selectedTags.some((t) => !s.tags.includes(t))) continue;
      if (this.category !== null && (s.category ?? "") !== this.category) continue;
      if (this.host === "any" && s.hosts.length > 0) continue;
      if (this.host !== null && this.host !== "any" && s.hosts.length > 0 && !s.hosts.some((h) => hostKind(h) === this.host)) continue;
      let score = 0;
      let ok = true;
      for (const t of tokens) {
        let hit = 0;
        const id = s.id.toLowerCase();
        if (id === t) hit += 100;
        else if (id.includes(t)) hit += 50;
        if (s.tags.includes(t)) hit += 40;
        else if (s.tags.some((x) => x.includes(t))) hit += 20;
        if ((s.category ?? "").toLowerCase().includes(t)) hit += 15;
        if (s.description.toLowerCase().includes(t)) hit += 10;
        if (hit === 0) {
          ok = false;
          break;
        }
        score += hit;
      }
      if (ok) scored.push([score, s]);
    }
    scored.sort((a, b) => b[0] - a[0] || a[1].id.localeCompare(b[1].id));
    return scored.map(([, s]) => s);
  }

  get driftCount(): number {
    return this.status.filter((s) => s.state !== "up-to-date").length;
  }

  /// Drift across both kinds, for the top bar badge.
  get totalDrift(): number {
    return (["skill", "agent"] as ItemKind[]).reduce(
      (n, k) => n + this.projectStatus[k].filter((s) => s.state !== "up-to-date").length,
      0,
    );
  }

  get projectName(): string | null {
    return this.projectPath ? this.projectPath.split("/").filter(Boolean).pop() ?? this.projectPath : null;
  }

  statusOf(id: string): InstalledSkill | undefined {
    return this.status.find((s) => s.id === id);
  }

  notify(msg: string) {
    this.toast = msg;
    if (this.toastTimer) clearTimeout(this.toastTimer);
    // Long messages stay long enough to be read.
    this.toastTimer = setTimeout(() => (this.toast = null), Math.max(2600, msg.length * 60));
  }

  fail(e: unknown) {
    this.error = errorText(e);
    console.error(e);
  }

  async run<T>(label: string | null, fn: () => Promise<T>): Promise<T | undefined> {
    this.loading = true;
    this.error = null;
    try {
      const r = await fn();
      if (label) this.notify(label);
      return r;
    } catch (e) {
      this.fail(e);
      return undefined;
    } finally {
      this.loading = false;
    }
  }

  async init() {
    await this.run(null, async () => {
      this.config = await api.getConfig();
      if (this.config.library_path) {
        await this.refreshLibrary("skill");
        await this.refreshLibrary("agent").catch(this.fail.bind(this));
        await this.refreshRegistry();
      }
      const recent = this.config.recent_projects[0];
      if (!recent) await this.refreshProjects();
      if (recent && (await api.pathExists(recent.path))) {
        this.projectPath = recent.path;
        this.target = recent.target;
        await this.refreshProject();
      }
    });
  }

  async setKind(kind: ItemKind) {
    this.view = "library";
    if (kind === this.kind) return;
    this.kind = kind;
    this.selectedId = null;
    this.checked = new Set();
    this.selectedTags = [];
    this.category = null;
    this.host = null;
    if (!this.libraries[kind] && this.config?.library_path) {
      await this.run(null, () => this.refreshLibrary(kind));
    }
  }

  async refreshLibrary(kind: ItemKind = this.kind, keepSelection = true) {
    const lib = await api.scanLibrary(kind);
    this.libraries[kind] = lib;
    void this.refreshGitStates(kind);
    if (kind !== this.kind) return;
    if (!keepSelection || !lib.skills.some((s) => s.id === this.selectedId)) {
      this.selectedId = null;
    }
    // Drop filters that no longer exist.
    this.selectedTags = this.selectedTags.filter((t) => lib.tags.includes(t));
    if (this.category !== null && this.category !== "" && !lib.categories.includes(this.category)) {
      this.category = null;
    }
    this.checked = new Set([...this.checked].filter((id) => lib.skills.some((s) => s.id === id)));
  }

  async openLibrary(load: () => Promise<Config>): Promise<boolean> {
    if (this.libraryBusy) return false;
    this.libraryBusy = true;
    try {
      const result = await this.run("Bibliothèque chargée", async () => {
        const config = await load();
        this.config = config;
        this.libraryGeneration++;
        this.libraries = { skill: null, agent: null };
        this.selectedId = null;
        this.checked = new Set();
        this.selectedTags = [];
        this.category = null;
        this.host = null;
        this.query = "";
        await this.refreshLibrary("skill", false);
        await this.refreshLibrary("agent", false);
        await this.refreshRegistry();
        await this.refreshProject();
        return true;
      });
      return result === true;
    } finally {
      this.libraryBusy = false;
    }
  }

  async setLibrary(path: string) {
    return this.openLibrary(() => api.setLibrary(path));
  }

  async cloneLibrary(url: string, parent: string, name: string) {
    return this.openLibrary(() => api.cloneLibrary(url, parent, name));
  }

  async setAgentsLibrary(path: string | null) {
    await this.run(path ? "Bibliothèque d'agents chargée" : "Dossier d'agents par défaut", async () => {
      this.config = await api.setAgentsLibrary(path);
      await this.refreshLibrary("agent", false);
      await this.refreshProject();
    });
  }

  async refreshProject() {
    if (!this.projectPath) {
      this.projectStatus = { skill: [], agent: [] };
      await this.refreshProjects();
      return;
    }
    const [skill, agent] = await Promise.all([
      api.projectStatus("skill", this.projectPath, this.target),
      api.projectStatus("agent", this.projectPath, this.target),
    ]);
    this.projectStatus = { skill, agent };
    // Whatever changed the current project may have changed the others too.
    await this.refreshProjects();
  }

  async setProject(path: string | null, target?: Target) {
    this.projectPath = path;
    if (target) this.target = target;
    await this.run(null, async () => {
      if (path) this.config = await api.rememberProject(path, this.target);
      await this.refreshProject();
    });
  }

  /// Remove an installed copy: from the current project, or from `where`. The library is not touched.
  async uninstall(id: string, where?: { project: string; target: Target; kind: ItemKind; state: string }): Promise<boolean> {
    const project = where?.project ?? this.projectPath;
    if (!project) return false;
    const name = project.split("/").filter(Boolean).pop() ?? project;
    const edited = ["project-modified", "conflict"].includes(where?.state ?? this.statusOf(id)?.state ?? "");
    const ok = await confirm(
      `La copie installée dans ${name} sera supprimée. ${id} reste dans la bibliothèque.` +
        (edited ? "\n\nCette copie contient des modifications faites dans le projet : elles seront perdues." : ""),
      { title: `Retirer ${id} du projet ?`, kind: edited ? "warning" : "info", okLabel: "Retirer du projet", cancelLabel: "Annuler" },
    );
    if (!ok) return false;
    const done = await this.run(`${id} retiré de ${name}`, async () => {
      await api.uninstallSkill(where?.kind ?? this.kind, id, project, where?.target ?? this.target);
      await this.refreshProject();
      return true;
    });
    return done === true;
  }

  /// Delete an item from the library itself, for every project. It goes to the Trash.
  async deleteFromLibrary(id: string): Promise<boolean> {
    const installedHere = !!this.statusOf(id);
    const ok = await confirm(
      `${id} sera déplacé dans la Corbeille et retiré de la bibliothèque, donc de tous les projets qui l’installeraient ensuite. La suppression sera partagée à la prochaine publication.` +
        (installedHere ? `\n\nLa copie installée dans ${this.projectName} n’est pas retirée. Pour enlever seulement cette copie, annulez et utilisez « Retirer du projet ».` : ""),
      { title: `Supprimer ${id} de la bibliothèque ?`, kind: "warning", okLabel: "Supprimer de la bibliothèque", cancelLabel: "Annuler" },
    );
    if (!ok) return false;
    const kind = this.kind;
    const done = await this.run(`${id} supprimé de la bibliothèque (déplacé dans la Corbeille)`, async () => {
      await api.deleteSkill(kind, id);
      if (this.selectedId === id) this.selectedId = null;
      await this.refreshLibrary(kind);
      await this.refreshRegistry();
      await this.refreshProject();
      return true;
    });
    return done === true;
  }

  replaceSkill(skill: Skill) {
    const lib = this.libraries[skill.kind];
    if (!lib) return;
    const i = lib.skills.findIndex((s) => s.id === skill.id);
    if (i >= 0) lib.skills[i] = skill;
    else lib.skills.push(skill);
    const tags = new Set(lib.skills.flatMap((s) => s.tags));
    const cats = new Set(lib.skills.map((s) => s.category).filter((c): c is string => !!c));
    lib.tags = [...tags].sort();
    lib.categories = [...cats].sort();
    void this.refreshGitStates(skill.kind);
  }

  toggleChecked(id: string) {
    const next = new Set(this.checked);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    this.checked = next;
  }
}

export const store = new AppStore();
