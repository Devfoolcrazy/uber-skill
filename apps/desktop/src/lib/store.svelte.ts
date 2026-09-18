import {
  api,
  hostKind,
  targetSupports,
  type Config,
  type Facet,
  type InstalledSkill,
  type InstallRequest,
  type ItemKind,
  type LibraryView,
  type RegistryView,
  type Skill,
  type Target,
} from "./api";
import { errorText } from "./errors";

export const DRIFT_LABEL: Record<string, string> = {
  "up-to-date": "À jour",
  "library-updated": "Bibliothèque plus récente",
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
  gitDialog = $state<{ install?: InstallRequest } | null>(null);
  /// Allowed categories and tags of the library, merged with the values in use.
  registry = $state<RegistryView | null>(null);
  registryOpen = $state(false);

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
    this.gitDialog = { install: { kind: this.kind, ids, project: this.projectPath, target: this.target } };
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
    this.toastTimer = setTimeout(() => (this.toast = null), 2600);
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
      if (recent && (await api.pathExists(recent.path))) {
        this.projectPath = recent.path;
        this.target = recent.target;
        await this.refreshProject();
      }
    });
  }

  async setKind(kind: ItemKind) {
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
      return;
    }
    const [skill, agent] = await Promise.all([
      api.projectStatus("skill", this.projectPath, this.target),
      api.projectStatus("agent", this.projectPath, this.target),
    ]);
    this.projectStatus = { skill, agent };
  }

  async setProject(path: string | null, target?: Target) {
    this.projectPath = path;
    if (target) this.target = target;
    await this.run(null, async () => {
      if (path) this.config = await api.rememberProject(path, this.target);
      await this.refreshProject();
    });
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
  }

  toggleChecked(id: string) {
    const next = new Set(this.checked);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    this.checked = next;
  }
}

export const store = new AppStore();
