import { invoke } from "@tauri-apps/api/core";

export type ItemKind = "skill" | "agent";
export const KIND_LABEL: Record<ItemKind, { one: string; many: string }> = {
  skill: { one: "skill", many: "skills" },
  agent: { one: "agent", many: "agents" },
};

export type Target =
  | { kind: "claude-code" }
  | { kind: "agents" }
  | { kind: "cursor" }
  | { kind: "copilot" }
  | { kind: "custom"; path: string };

export const TARGETS: { target: Target; label: string; dirs: Record<ItemKind, string | null> }[] = [
  { target: { kind: "claude-code" }, label: "Claude Code", dirs: { skill: ".claude/skills", agent: ".claude/agents" } },
  { target: { kind: "agents" }, label: "Agents (Codex, Amp, Copilot CLI)", dirs: { skill: ".agents/skills", agent: null } },
  { target: { kind: "cursor" }, label: "Cursor", dirs: { skill: ".cursor/skills", agent: null } },
  { target: { kind: "copilot" }, label: "GitHub Copilot", dirs: { skill: ".github/skills", agent: null } },
];

/// Known `hosts` values and the target they map to (mirrors Target::parse in Rust).
export const HOST_ALIASES: Record<string, Target["kind"]> = {
  claude: "claude-code", "claude-code": "claude-code", claudecode: "claude-code",
  agents: "agents", codex: "agents", amp: "agents", "copilot-cli": "agents",
  cursor: "cursor",
  copilot: "copilot", github: "copilot", "github-copilot": "copilot",
};
export const KNOWN_HOSTS = ["claude-code", "codex", "cursor", "copilot"];
export function hostKind(host: string): Target["kind"] | null {
  return HOST_ALIASES[host.toLowerCase()] ?? null;
}
export function targetAcceptsHosts(t: Target, hosts: string[]): boolean {
  if (hosts.length === 0 || t.kind === "custom") return true;
  return hosts.some((h) => hostKind(h) === t.kind);
}

export function targetKey(t: Target): string {
  return t.kind === "custom" ? `custom:${t.path}` : t.kind;
}
export function targetDirFor(kind: ItemKind, t: Target): string | null {
  if (t.kind === "custom") return t.path;
  return TARGETS.find((x) => x.target.kind === t.kind)?.dirs[kind] ?? null;
}
export function targetSupports(kind: ItemKind, t: Target): boolean {
  return targetDirFor(kind, t) !== null;
}

export interface RecentProject {
  path: string;
  target: Target;
}
export interface Config {
  library_path: string | null;
  agents_path: string | null;
  recent_projects: RecentProject[];
  editor_command: string | null;
}
export interface Skill {
  kind: ItemKind;
  id: string;
  name: string;
  description: string;
  category: string | null;
  tags: string[];
  hosts: string[];
  path: string;
  rel_path: string;
  hash: string;
  files: string[];
  extra: Record<string, string>;
  body_chars: number;
  modified_at: string | null;
}
export interface ScanWarning {
  path: string;
  message: string;
}
export interface LibraryView {
  kind: ItemKind;
  root: string;
  skills: Skill[];
  warnings: ScanWarning[];
  tags: string[];
  categories: string[];
}
export type DriftState =
  | "up-to-date"
  | "library-updated"
  | "project-modified"
  | "conflict"
  | "untracked"
  | "missing"
  | "source-missing";
export interface LockEntry {
  id: string;
  source: string;
  hash: string;
  installed_at: string;
}
export interface InstalledSkill {
  id: string;
  state: DriftState;
  path: string;
  lock: LockEntry | null;
  installed_hash: string | null;
  library_hash: string | null;
  description: string | null;
}
export interface FileDiff {
  file: string;
  kind: "added" | "removed" | "modified" | "binary";
  unified: string;
}
export type Severity = "info" | "warning" | "error";
export interface Issue {
  severity: Severity;
  rule: string;
  message: string;
}

export interface PublicationPreview {
  root: string;
  branch: string | null;
  head: string | null;
  remote: string | null;
  remote_branch: string | null;
  pending_count: number;
  pending_commits: string[];
  files: { path: string; status: string; diff: string }[];
  snapshot: string;
  blocked: string | null;
}

export interface PublicationResult {
  commit: string | null;
  pushed: boolean;
  push_error: string | null;
}

export const api = {
  getConfig: () => invoke<Config>("get_config"),
  setLibrary: (path: string) => invoke<Config>("set_library", { path }),
  cloneLibrary: (url: string, parent: string, name: string) => invoke<Config>("clone_library", { url, parent, name }),
  publicationPreview: () => invoke<PublicationPreview>("git_publication_preview"),
  publishLibrary: (snapshot: string, paths: string[], message: string) =>
    invoke<PublicationResult>("publish_library", { snapshot, paths, message }),
  setAgentsLibrary: (path: string | null) => invoke<Config>("set_agents_library", { path }),
  setEditor: (command: string | null) => invoke<Config>("set_editor", { command }),
  rememberProject: (path: string, target: Target) => invoke<Config>("remember_project", { path, target }),
  scanLibrary: (kind: ItemKind) => invoke<LibraryView>("scan_library", { kind }),
  getSkill: (kind: ItemKind, id: string) => invoke<Skill>("get_skill", { kind, id }),
  readSkillFile: (kind: ItemKind, id: string, rel: string) => invoke<string>("read_skill_file", { kind, id, rel }),
  writeSkillFile: (kind: ItemKind, id: string, rel: string, text: string) =>
    invoke<Skill>("write_skill_file", { kind, id, rel, text }),
  updateMeta: (
    kind: ItemKind,
    id: string,
    patch: { tags?: string[]; category?: string | null; set_category: boolean; hosts?: string[]; description?: string },
  ) => invoke<Skill>("update_meta", { kind, id, patch }),
  createSkill: (kind: ItemKind, id: string, description: string, category: string | null, tags: string[], hosts: string[]) =>
    invoke<Skill>("create_skill", { kind, id, description, category, tags, hosts }),
  checkHosts: (kind: ItemKind, ids: string[], target: Target) => invoke<string[]>("check_hosts", { kind, ids, target }),
  deleteSkill: (kind: ItemKind, id: string) => invoke<void>("delete_skill", { kind, id }),
  importSkill: (kind: ItemKind, path: string, newId: string | null) => invoke<Skill>("import_skill", { kind, path, newId }),
  lintSkill: (kind: ItemKind, id: string) => invoke<Issue[]>("lint_skill", { kind, id }),
  projectStatus: (kind: ItemKind, project: string, target: Target) =>
    invoke<InstalledSkill[]>("project_status", { kind, project, target }),
  installSkills: (kind: ItemKind, ids: string[], project: string, target: Target) =>
    invoke<LockEntry[]>("install_skills", { kind, ids, project, target }),
  uninstallSkill: (kind: ItemKind, id: string, project: string, target: Target) =>
    invoke<void>("uninstall_skill", { kind, id, project, target }),
  diffInstalled: (kind: ItemKind, id: string, project: string, target: Target) =>
    invoke<FileDiff[]>("diff_installed", { kind, id, project, target }),
  syncSkill: (kind: ItemKind, id: string, direction: "pull" | "push", project: string, target: Target) =>
    invoke<void>("sync_skill", { kind, id, direction, project, target }),
  adoptSkill: (kind: ItemKind, id: string, project: string, target: Target) =>
    invoke<LockEntry>("adopt_skill", { kind, id, project, target }),
  openInEditor: (path: string) => invoke<void>("open_in_editor", { path }),
  pathExists: (path: string) => invoke<boolean>("path_exists", { path }),
};

export function splitFrontmatter(text: string): { front: string; body: string } {
  const m = text.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?([\s\S]*)$/);
  if (!m) return { front: "", body: text };
  return { front: m[1], body: m[2] };
}
