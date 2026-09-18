import { invoke } from "@tauri-apps/api/core";
import { vi } from "vitest";
import type { GitSyncStatus, InstallationPlan, LibraryView, PublicationPreview, RegistryView, Skill, ValueUsage } from "$lib/api";

type Handler = (args: Record<string, unknown>) => unknown;

/// Answer Tauri commands from a table; an unexpected command fails the test.
export function bridge(handlers: Record<string, Handler>) {
  const mock = vi.mocked(invoke);
  mock.mockReset();
  mock.mockImplementation(async (cmd, args) => {
    const handler = handlers[cmd];
    if (!handler) throw new Error(`commande inattendue : ${cmd}`);
    return handler((args ?? {}) as Record<string, unknown>);
  });
  return {
    calls: (cmd: string) => mock.mock.calls.filter(([c]) => c === cmd).map(([, a]) => a as Record<string, unknown>),
  };
}

export function skill(id: string, over: Partial<Skill> = {}): Skill {
  return {
    kind: "skill", id, name: id, description: "", category: null, tags: [], hosts: [],
    path: `/lib/skills/${id}`, rel_path: id, hash: `hash-${id}`, files: ["SKILL.md"],
    extra: {}, body_chars: 10, modified_at: null, ...over,
  };
}

export function libraryView(skills: Skill[], over: Partial<LibraryView> = {}): LibraryView {
  return {
    kind: "skill", root: "/lib/skills", skills, warnings: [],
    tags: [...new Set(skills.flatMap((s) => s.tags))].sort(),
    categories: [...new Set(skills.flatMap((s) => (s.category ? [s.category] : [])))].sort(),
    ...over,
  };
}

export function usage(value: string, known: boolean, ids: string[] = []): ValueUsage {
  return { value, known, items: ids.map((id) => ({ kind: "skill", id })) };
}

export function registryView(over: Partial<RegistryView> = {}): RegistryView {
  return {
    path: "/lib/uber-skill.yaml", exists: true,
    categories: [usage("review", true, ["one"]), usage("writing", true)],
    tags: [usage("git", true, ["one", "two"]), usage("quality", true), usage("legacy", false, ["two"])],
    ...over,
  };
}

export function syncStatus(over: Partial<GitSyncStatus> = {}): GitSyncStatus {
  return {
    root: "/lib", branch: "main", head: "aaa", remote: "origin", remote_ref: "refs/heads/main",
    upstream_ref: "refs/remotes/origin/main", remote_head: "aaa", ahead: 0, behind: 0, changed_files: [],
    verified: true, fetch_error: null, blocked: null, snapshot: "snap-1", ...over,
  };
}

export function plan(git: GitSyncStatus | null, over: Partial<InstallationPlan> = {}): InstallationPlan {
  return {
    root: "/lib", kind: "skill", target: { kind: "claude-code" },
    items: [{ id: "one", source: "/lib/skills/one", hash: "hash-one", source_state: "published" }],
    git, git_error: null, warnings: [], ...over,
  };
}

export function publicationPreview(over: Partial<PublicationPreview> = {}): PublicationPreview {
  return {
    root: "/lib", branch: "main", head: "aaa", remote: "origin", remote_branch: "main",
    pending_count: 0, pending_commits: [], snapshot: "preview-1", blocked: null,
    files: [
      { path: "skills/one/SKILL.md", status: "M", diff: "+one" },
      { path: "skills/two/SKILL.md", status: "M", diff: "+two" },
    ],
    ...over,
  };
}
