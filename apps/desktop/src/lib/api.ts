import { invoke } from "@tauri-apps/api/core";

export type Target =
  | { kind: "claude-code" }
  | { kind: "agents" }
  | { kind: "cursor" }
  | { kind: "copilot" }
  | { kind: "custom"; path: string };

export const TARGETS: { target: Target; label: string; dir: string }[] = [
  { target: { kind: "claude-code" }, label: "Claude Code", dir: ".claude/skills" },
  { target: { kind: "agents" }, label: "Agents (Codex, Amp, Copilot CLI)", dir: ".agents/skills" },
  { target: { kind: "cursor" }, label: "Cursor", dir: ".cursor/skills" },
  { target: { kind: "copilot" }, label: "GitHub Copilot", dir: ".github/skills" },
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
export function targetDir(t: Target): string {
  return t.kind === "custom" ? t.path : TARGETS.find((x) => x.target.kind === t.kind)!.dir;
}

export interface RecentProject {
  path: string;
  target: Target;
}
export interface Config {
  library_path: string | null;
  recent_projects: RecentProject[];
  editor_command: string | null;
}
export interface Skill {
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

export const api = {
  getConfig: () => invoke<Config>("get_config"),
  setLibrary: (path: string) => invoke<Config>("set_library", { path }),
  setEditor: (command: string | null) => invoke<Config>("set_editor", { command }),
  rememberProject: (path: string, target: Target) => invoke<Config>("remember_project", { path, target }),
  scanLibrary: () => invoke<LibraryView>("scan_library"),
  getSkill: (id: string) => invoke<Skill>("get_skill", { id }),
  readSkillFile: (id: string, rel: string) => invoke<string>("read_skill_file", { id, rel }),
  writeSkillFile: (id: string, rel: string, text: string) => invoke<Skill>("write_skill_file", { id, rel, text }),
  updateMeta: (
    id: string,
    patch: { tags?: string[]; category?: string | null; set_category: boolean; hosts?: string[]; description?: string },
  ) => invoke<Skill>("update_meta", { id, patch }),
  createSkill: (id: string, description: string, category: string | null, tags: string[], hosts: string[]) =>
    invoke<Skill>("create_skill", { id, description, category, tags, hosts }),
  checkHosts: (ids: string[], target: Target) => invoke<string[]>("check_hosts", { ids, target }),
  deleteSkill: (id: string) => invoke<void>("delete_skill", { id }),
  importSkill: (path: string, newId: string | null) => invoke<Skill>("import_skill", { path, newId }),
  lintSkill: (id: string) => invoke<Issue[]>("lint_skill", { id }),
  projectStatus: (project: string, target: Target) =>
    invoke<InstalledSkill[]>("project_status", { project, target }),
  installSkills: (ids: string[], project: string, target: Target) =>
    invoke<LockEntry[]>("install_skills", { ids, project, target }),
  uninstallSkill: (id: string, project: string, target: Target) =>
    invoke<void>("uninstall_skill", { id, project, target }),
  diffInstalled: (id: string, project: string, target: Target) =>
    invoke<FileDiff[]>("diff_installed", { id, project, target }),
  syncSkill: (id: string, direction: "pull" | "push", project: string, target: Target) =>
    invoke<void>("sync_skill", { id, direction, project, target }),
  adoptSkill: (id: string, project: string, target: Target) =>
    invoke<LockEntry>("adopt_skill", { id, project, target }),
  openInEditor: (path: string) => invoke<void>("open_in_editor", { path }),
  pathExists: (path: string) => invoke<boolean>("path_exists", { path }),
};

export function splitFrontmatter(text: string): { front: string; body: string } {
  const m = text.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?([\s\S]*)$/);
  if (!m) return { front: "", body: text };
  return { front: m[1], body: m[2] };
}
