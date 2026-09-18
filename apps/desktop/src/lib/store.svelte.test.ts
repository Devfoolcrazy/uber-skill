import { beforeEach, describe, expect, it } from "vitest";
import type { Config } from "./api";
import { store } from "./store.svelte";
import { bridge, libraryView, skill } from "../test/bridge";

const config = (library_path: string): Config => ({ library_path, agents_path: null, recent_projects: [], editor_command: null });

beforeEach(() => {
  store.config = config("/old");
  store.kind = "skill";
  store.libraries = { skill: null, agent: null };
  store.query = "";
  store.selectedTags = [];
  store.category = null;
  store.host = null;
  store.selectedId = null;
  store.checked = new Set();
  store.projectPath = null;
  store.target = { kind: "claude-code" };
  store.gitDialog = null;
  store.error = null;
});

describe("filtered", () => {
  beforeEach(() => {
    store.libraries.skill = libraryView([
      skill("review-pr", { tags: ["git", "quality"], category: "review", description: "Relit une pull request" }),
      skill("commit-message", { tags: ["git"], description: "Rédige un message de commit" }),
      skill("mcp-helper", { hosts: ["claude-code"], description: "Utilise des outils MCP pour git" }),
    ]);
  });

  it("requires every selected tag", () => {
    store.selectedTags = ["git", "quality"];
    expect(store.filtered.map((s) => s.id)).toEqual(["review-pr"]);
  });

  it("keeps host-agnostic skills when filtering on a host, and only those for « any »", () => {
    store.host = "cursor";
    expect(store.filtered.map((s) => s.id)).toEqual(["commit-message", "review-pr"]);
    store.host = "claude-code";
    expect(store.filtered.map((s) => s.id)).toContain("mcp-helper");
    store.host = "any";
    expect(store.filtered.map((s) => s.id)).not.toContain("mcp-helper");
  });

  it("ranks an id match above a tag match above a description match", () => {
    store.query = "git";
    expect(store.filtered.map((s) => s.id)).toEqual(["commit-message", "review-pr", "mcp-helper"]);
    store.query = "commit";
    expect(store.filtered[0].id).toBe("commit-message");
  });

  it("requires every search token to match", () => {
    store.query = "git pull";
    expect(store.filtered.map((s) => s.id)).toEqual(["review-pr"]);
  });
});

describe("requestInstall", () => {
  it("does nothing without a project or with a target that cannot host the kind", () => {
    store.requestInstall(["one"]);
    expect(store.gitDialog).toBeNull();
    store.projectPath = "/project";
    store.kind = "agent";
    store.target = { kind: "cursor" };
    store.requestInstall(["reviewer"]);
    expect(store.gitDialog).toBeNull();
  });

  it("opens the verification dialog with the current kind, project and target", () => {
    store.projectPath = "/project";
    store.requestInstall(["one", "two"]);
    expect(store.gitDialog).toEqual({
      install: { kind: "skill", ids: ["one", "two"], project: "/project", target: { kind: "claude-code" } },
    });
  });
});

describe("openLibrary", () => {
  it("keeps the previous library and selection when opening fails", async () => {
    bridge({ set_library: () => { throw "Choisissez la racine du dépôt Git : /repo"; } });
    const previous = libraryView([skill("one")]);
    store.libraries.skill = previous;
    store.selectedId = "one";
    const generation = store.libraryGeneration;

    expect(await store.setLibrary("/repo/skills")).toBe(false);

    expect(store.config?.library_path).toBe("/old");
    expect(store.libraries.skill).toEqual(previous);
    expect(store.selectedId).toBe("one");
    expect(store.libraryGeneration).toBe(generation);
    expect(store.error).toContain("racine");
    expect(store.libraryBusy).toBe(false);
  });

  it("resets selection and filters, reloads both kinds and remounts the editor", async () => {
    const { calls } = bridge({
      clone_library: () => config("/new"),
      scan_library: ({ kind }) => libraryView([skill("fresh")], { kind: kind as "skill" | "agent" }),
    });
    store.libraries.skill = libraryView([skill("one", { tags: ["git"] })]);
    store.selectedId = "one";
    store.checked = new Set(["one"]);
    store.selectedTags = ["git"];
    store.query = "one";
    const generation = store.libraryGeneration;

    expect(await store.cloneLibrary("https://example.com/lib.git", "/parent", "lib")).toBe(true);

    expect(calls("clone_library")).toEqual([{ url: "https://example.com/lib.git", parent: "/parent", name: "lib" }]);
    expect(calls("scan_library").map((a) => a.kind)).toEqual(["skill", "agent"]);
    expect(store.config?.library_path).toBe("/new");
    expect(store.selectedId).toBeNull();
    expect(store.checked.size).toBe(0);
    expect(store.selectedTags).toEqual([]);
    expect(store.query).toBe("");
    expect(store.libraryGeneration).toBe(generation + 1);
  });
});
