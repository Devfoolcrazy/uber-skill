import { api, hostKind, type Config, type InstalledSkill, type LibraryView, type Skill, type Target } from "./api";

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
  library = $state<LibraryView | null>(null);
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
  projectStatus = $state<InstalledSkill[]>([]);
  showProject = $state(true);

  private toastTimer: ReturnType<typeof setTimeout> | null = null;

  get skills(): Skill[] {
    return this.library?.skills ?? [];
  }

  get selected(): Skill | null {
    return this.skills.find((s) => s.id === this.selectedId) ?? null;
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

  statusOf(id: string): InstalledSkill | undefined {
    return this.projectStatus.find((s) => s.id === id);
  }

  notify(msg: string) {
    this.toast = msg;
    if (this.toastTimer) clearTimeout(this.toastTimer);
    this.toastTimer = setTimeout(() => (this.toast = null), 2600);
  }

  fail(e: unknown) {
    this.error = typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
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
      if (this.config.library_path) await this.refreshLibrary();
      const recent = this.config.recent_projects[0];
      if (recent && (await api.pathExists(recent.path))) {
        this.projectPath = recent.path;
        this.target = recent.target;
        await this.refreshProject();
      }
    });
  }

  async refreshLibrary(keepSelection = true) {
    const lib = await api.scanLibrary();
    this.library = lib;
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

  async setLibrary(path: string) {
    await this.run("Bibliothèque chargée", async () => {
      this.config = await api.setLibrary(path);
      await this.refreshLibrary(false);
      await this.refreshProject();
    });
  }

  async refreshProject() {
    if (!this.projectPath) {
      this.projectStatus = [];
      return;
    }
    this.projectStatus = await api.projectStatus(this.projectPath, this.target);
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
    if (!this.library) return;
    const i = this.library.skills.findIndex((s) => s.id === skill.id);
    if (i >= 0) this.library.skills[i] = skill;
    else this.library.skills.push(skill);
    const tags = new Set(this.library.skills.flatMap((s) => s.tags));
    const cats = new Set(this.library.skills.map((s) => s.category).filter((c): c is string => !!c));
    this.library.tags = [...tags].sort();
    this.library.categories = [...cats].sort();
  }

  toggleChecked(id: string) {
    const next = new Set(this.checked);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    this.checked = next;
  }
}

export const store = new AppStore();
