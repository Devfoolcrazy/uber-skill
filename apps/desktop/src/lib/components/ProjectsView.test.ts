import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { InstalledSkill, ProjectOverview } from "$lib/api";
import { store } from "$lib/store.svelte";
import { bridge, skill } from "../../test/bridge";
import InstalledIn from "./InstalledIn.svelte";
import ProjectsView from "./ProjectsView.svelte";

const item = (id: string, state: InstalledSkill["state"]) => ({ id, state }) as InstalledSkill;
const claude = { kind: "claude-code" } as const;
const cursor = { kind: "cursor" } as const;
const projects: ProjectOverview[] = [
  {
    path: "/work/game", name: "game", exists: true, global: false, behind: 2,
    installs: [
      { target: claude, kind: "skill", items: [item("one", "library-updated"), item("two", "project-modified")] },
      { target: claude, kind: "agent", items: [item("reviewer", "library-updated")] },
      { target: cursor, kind: "skill", items: [item("three", "up-to-date")] },
    ],
  },
  { path: "/work/site", name: "site", exists: true, global: false, behind: 1, installs: [{ target: claude, kind: "skill", items: [item("one", "library-updated")] }] },
  { path: "/work/gone", name: "gone", exists: false, global: false, behind: 0, installs: [] },
];

beforeEach(() => {
  store.config = { library_path: "/lib", agents_path: null, recent_projects: [], editor_command: null, tracked_projects: projects.map((p) => p.path) };
  store.projectPath = "/work/game";
  store.kind = "skill";
  store.view = "projects";
  store.gitDialog = null;
  store.editorDirty = false;
  store.projects = projects;
});

describe("followed projects", () => {
  it("lists projects with what is behind, across targets and kinds", async () => {
    bridge({ projects_overview: () => projects });
    render(ProjectsView);

    const list = screen.getByLabelText("Projets suivis");
    expect(within(list).getByText("2 en retard")).toBeTruthy();
    expect(within(list).getByText("introuvable")).toBeTruthy();
    // Two targets in this project: each section says which one it is.
    const headings = (await screen.findAllByRole("heading", { level: 3 })).map((h) => h.textContent?.replace(/\s+/g, " ").trim());
    expect(headings).toEqual(["Projets suivis", "Skills · Claude Code", "Agents · Claude Code", "Skills · Cursor"]);
  });

  it("« tout mettre à jour » only takes the copies simply behind, in one reviewed batch", async () => {
    bridge({ projects_overview: () => projects });
    render(ProjectsView);

    await fireEvent.click(await screen.findByRole("button", { name: "Tout mettre à jour (2)" }));

    expect(store.gitDialog).toEqual({
      install: [
        { project: "/work/game", kind: "skill", target: claude, ids: ["one"] },
        { project: "/work/game", kind: "agent", target: claude, ids: ["reviewer"] },
      ],
    });
  });

  it("keeps a missing project listed until it is removed, and follows a new one", async () => {
    const dialog = vi.mocked((await import("@tauri-apps/plugin-dialog")).open);
    dialog.mockResolvedValue("/work/new");
    const { calls } = bridge({
      projects_overview: () => projects,
      untrack_project: () => ({ ...store.config!, tracked_projects: ["/work/game", "/work/site"] }),
      track_project: () => ({ ...store.config!, tracked_projects: [...store.config!.tracked_projects, "/work/new"] }),
    });
    render(ProjectsView);

    await fireEvent.click(screen.getByRole("button", { name: /gone/ }));
    expect(screen.getByText(/Ce dossier est introuvable/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Tout mettre à jour/ })).toBeNull();
    await fireEvent.click(screen.getByRole("button", { name: "Retirer de la liste" }));
    await waitFor(() => expect(calls("untrack_project")).toEqual([{ path: "/work/gone" }]));

    await fireEvent.click(screen.getByRole("button", { name: "+ Suivre un projet…" }));
    await waitFor(() => expect(calls("track_project")).toEqual([{ path: "/work/new" }]));
  });

  it("puts the global install first, whole, with adopt actions and no way to unfollow it", async () => {
    const global: ProjectOverview = {
      path: "/Users/demo", name: "Global", exists: true, global: true, behind: 0,
      installs: [
        { target: claude, kind: "skill", items: [item("one", "up-to-date"), item("handmade", "untracked")] },
        { target: { kind: "agents" }, kind: "skill", items: [item("two", "untracked")] },
      ],
    };
    store.globalRoot = "/Users/demo";
    store.libraries = { skill: { kind: "skill", root: "/lib", skills: [skill("one"), skill("two")], warnings: [], tags: [], categories: [] }, agent: null };
    store.projects = [global, ...projects];
    const { calls } = bridge({ projects_overview: () => store.projects, adopt_skill: () => null, scan_library: () => store.libraries.skill, project_status: () => [], get_registry: () => null, library_git_states: () => ({}) });
    render(ProjectsView);

    await fireEvent.click(screen.getByRole("button", { name: /Global/ }));
    expect(screen.getByText(/Vos dossiers personnels/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Retirer de la liste" })).toBeNull();
    expect(screen.getByRole("button", { name: "Importer handmade dans la bibliothèque" })).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Lier two à la bibliothèque" }));
    await waitFor(() => expect(calls("adopt_skill")).toEqual([{ kind: "skill", id: "two", project: "/Users/demo", target: { kind: "agents" } }]));
    store.globalRoot = null;
  });

  it("works on another project's copy without making it the current project", async () => {
    const confirm = vi.mocked((await import("@tauri-apps/plugin-dialog")).confirm);
    confirm.mockResolvedValue(true);
    const { calls } = bridge({ projects_overview: () => projects, uninstall_skill: () => null, project_status: () => [] });
    render(ProjectsView);

    await fireEvent.click(screen.getByRole("button", { name: /site/ }));
    await fireEvent.click(await screen.findByRole("button", { name: "Retirer one du projet" }));

    await waitFor(() => expect(calls("uninstall_skill")).toEqual([{ kind: "skill", id: "one", project: "/work/site", target: claude }]));
    expect(store.projectPath).toBe("/work/game");
  });
});

describe("« installé dans » on an item", () => {
  it("shows every project holding a copy and updates them all at once", async () => {
    render(InstalledIn, { skill: skill("one") });

    expect(screen.getByText("2 projets")).toBeTruthy();
    expect(screen.getByText("· 2 en retard")).toBeTruthy();
    expect(screen.getByText("projet courant")).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Mettre à jour partout (2)" }));

    expect(store.gitDialog).toEqual({
      install: [
        { project: "/work/game", kind: "skill", target: claude, ids: ["one"] },
        { project: "/work/site", kind: "skill", target: claude, ids: ["one"] },
      ],
    });
    expect(store.behindElsewhere("one")).toBe(1);
  });

  it("stays out of the way for an item installed nowhere", () => {
    const { container } = render(InstalledIn, { skill: skill("nowhere") });
    expect(container.querySelector("details")).toBeNull();
  });
});
