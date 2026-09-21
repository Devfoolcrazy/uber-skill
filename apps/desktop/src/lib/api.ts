import { invoke } from "@tauri-apps/api/core";
import type { BlockReason, ErrorPayload } from "./errors";

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
  /// Projects followed across the library; nothing leaves this list by itself.
  tracked_projects: string[];
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
  error?: ErrorPayload;
  duplicate_of?: string;
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
  source_state?: SourceState;
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
export interface LinkFix { file: string; from: string; to: string }
export interface Issue {
  severity: Severity;
  rule: string;
  /// Stable identifier, worded in lint.ts from `args`; `message` is the English fallback.
  code: string;
  args: string[];
  message: string;
  fix?: LinkFix;
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
  blocked: BlockReason | null;
}

export interface PublicationResult {
  commit: string | null;
  pushed: boolean;
  push_error: ErrorPayload | null;
}

export type SourceState = "published" | "local-draft" | "unpublished-commit" | "unverified";
export const SOURCE_LABEL: Record<SourceState, string> = {
  published: "Version publiée",
  "local-draft": "Brouillon local non publié",
  "unpublished-commit": "Commit local non publié",
  unverified: "Fraîcheur non vérifiée",
};
export interface GitSyncStatus {
  root: string; branch: string | null; head: string | null;
  remote: string | null; remote_ref: string | null; upstream_ref: string | null; remote_head: string | null;
  ahead: number; behind: number; changed_files: string[];
  verified: boolean; fetch_error: ErrorPayload | null; blocked: BlockReason | null; snapshot: string;
}
export interface HostMismatch { id: string; hosts: string[]; target: string }
export const hostMismatchText = (w: HostMismatch) => `${w.id} est déclaré pour ${w.hosts.join(", ")}, mais la cible est ${w.target}.`;
export interface InstallationPlan {
  root: string; kind: ItemKind; target: Target;
  items: { id: string; source: string; hash: string; source_state: SourceState }[];
  git: GitSyncStatus | null; git_error: ErrorPayload | null; warnings: HostMismatch[];
}
/// Where an item stands against the tracked remote branch (absent = identical).
export interface ItemGitState { modified: boolean; unpublished: boolean; outdated: boolean }
export interface RefineProposal { id: string; original: string; proposed: string; diff: string; restored_keys: string[] }
export type Facet = "category" | "tag";
export interface ValueUsage { value: string; known: boolean; items: { kind: ItemKind; id: string }[] }
export interface RegistryView { path: string; exists: boolean; categories: ValueUsage[]; tags: ValueUsage[] }
/// One install: some items of one kind into one target of one project.
export interface InstallRequest { kind: ItemKind; ids: string[]; project: string; target: Target }
/// Several installs reviewed together, against a single check of the remote.
export interface BatchPlan {
  root: string; git: GitSyncStatus | null; git_error: ErrorPayload | null;
  jobs: { project: string; plan: InstallationPlan }[];
}
export interface TargetInstall { target: Target; kind: ItemKind; items: InstalledSkill[] }
export interface ProjectOverview { path: string; name: string; exists: boolean; installs: TargetInstall[]; behind: number }
export const targetLabel = (t: Target) => TARGETS.find((x) => x.target.kind === t.kind)?.label ?? (t.kind === "custom" ? t.path : t.kind);

export const api = {
  getConfig: () => invoke<Config>("get_config"),
  setLibrary: (path: string) => invoke<Config>("set_library", { path }),
  cloneLibrary: (url: string, parent: string, name: string) => invoke<Config>("clone_library", { url, parent, name }),
  publicationPreview: () => invoke<PublicationPreview>("git_publication_preview"),
  publishLibrary: (snapshot: string, paths: string[], message: string) =>
    invoke<PublicationResult>("publish_library", { snapshot, paths, message }),
  checkLibraryGit: () => invoke<GitSyncStatus>("check_library_git"),
  updateLibraryGit: (snapshot: string) => invoke<GitSyncStatus>("update_library_git", { snapshot }),
  prepareInstall: (requests: InstallRequest[]) => invoke<BatchPlan>("prepare_install", { requests }),
  projectsOverview: () => invoke<ProjectOverview[]>("projects_overview"),
  trackProject: (path: string) => invoke<Config>("track_project", { path }),
  untrackProject: (path: string) => invoke<Config>("untrack_project", { path }),
  applyLintFix: (kind: ItemKind, id: string, fix: LinkFix) => invoke<Skill>("apply_lint_fix", { kind, id, fix }),
  libraryGitStates: (kind: ItemKind) => invoke<Record<string, ItemGitState>>("library_git_states", { kind }),
  refinePropose: (kind: ItemKind, id: string, instruction: string) =>
    invoke<RefineProposal>("refine_propose", { kind, id, instruction }),
  refineAccept: (kind: ItemKind, proposal: RefineProposal) => invoke<Skill>("refine_accept", { kind, proposal }),
  getRegistry: () => invoke<RegistryView>("get_registry"),
  registryInit: () => invoke<RegistryView>("registry_init"),
  registryAdd: (facet: Facet, value: string) => invoke<RegistryView>("registry_add", { facet, value }),
  registryRename: (facet: Facet, from: string, to: string) => invoke<RegistryView>("registry_rename", { facet, from, to }),
  /// `replacement` re-tags the items still using the value, `strip` removes it from them.
  registryRemove: (facet: Facet, value: string, replacement: string | null, strip: boolean) =>
    invoke<RegistryView>("registry_remove", { facet, value, replacement, strip }),
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
  deleteSkill: (kind: ItemKind, id: string) => invoke<void>("delete_skill", { kind, id }),
  importSkill: (kind: ItemKind, path: string, newId: string | null) => invoke<Skill>("import_skill", { kind, path, newId }),
  lintSkill: (kind: ItemKind, id: string) => invoke<Issue[]>("lint_skill", { kind, id }),
  projectStatus: (kind: ItemKind, project: string, target: Target) =>
    invoke<InstalledSkill[]>("project_status", { kind, project, target }),
  installSkills: (plan: BatchPlan) => invoke<LockEntry[]>("install_skills", { plan }),
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
