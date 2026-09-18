import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { InstallRequest } from "$lib/api";
import { store } from "$lib/store.svelte";
import { bridge, libraryView, plan, skill, syncStatus } from "../../test/bridge";
import GitSyncDialog from "./GitSyncDialog.svelte";

const request: InstallRequest = { kind: "skill", ids: ["one"], project: "/project", target: { kind: "claude-code" } };
const button = (name: string) => screen.getByRole("button", { name, hidden: true }) as HTMLButtonElement;
const refreshes = {
  scan_library: () => libraryView([skill("one")]),
  project_status: () => [],
};

beforeEach(() => {
  store.projectPath = "/project";
  store.editorDirty = false;
  store.checked = new Set(["one"]);
});

describe("install verification", () => {
  it("updates, prepares again and installs the refreshed plan", async () => {
    const behind = plan(syncStatus({ behind: 2, snapshot: "behind" }));
    const fresh = plan(syncStatus({ snapshot: "fresh" }), {
      items: [{ id: "one", source: "/lib/skills/one", hash: "hash-new", source_state: "published" }],
    });
    const prepared = [behind, fresh];
    const { calls } = bridge({
      ...refreshes,
      prepare_install: () => prepared.shift(),
      update_library_git: () => syncStatus({ snapshot: "fresh" }),
      install_skills: () => [],
    });
    const onclose = vi.fn();
    render(GitSyncDialog, { request, onclose });

    await screen.findByText(/2 commit\(s\) distant\(s\)/);
    await fireEvent.click(button("Mettre à jour puis installer"));

    await waitFor(() => expect(onclose).toHaveBeenCalled());
    expect(calls("update_library_git")).toEqual([{ snapshot: "behind" }]);
    expect(calls("install_skills")).toEqual([{ plan: fresh, project: "/project" }]);
    expect(store.checked.size).toBe(0);
  });

  it("never falls back to a local install when the update fails", async () => {
    const { calls } = bridge({
      ...refreshes,
      prepare_install: () => plan(syncStatus({ behind: 1 })),
      update_library_git: () => { throw "Mise à jour interrompue."; },
      install_skills: () => [],
    });
    const onclose = vi.fn();
    render(GitSyncDialog, { request, onclose });

    await screen.findByText(/1 commit\(s\) distant\(s\)/);
    await fireEvent.click(button("Mettre à jour puis installer"));

    expect((await screen.findByRole("alert", { hidden: true })).textContent).toContain("Mise à jour interrompue.");
    expect(calls("install_skills")).toEqual([]);
    expect(onclose).not.toHaveBeenCalled();
  });

  it("offers only an explicit local install when freshness cannot be verified", async () => {
    const offline = plan(syncStatus({ verified: false, fetch_error: "Could not resolve host" }), {
      items: [{ id: "one", source: "/lib/skills/one", hash: "hash-one", source_state: "unverified" }],
    });
    const { calls } = bridge({ ...refreshes, prepare_install: () => offline, install_skills: () => [] });
    const onclose = vi.fn();
    render(GitSyncDialog, { request, onclose });

    await screen.findByText("Could not resolve host");
    expect(screen.getByText(/Fraîcheur non vérifiée/)).toBeTruthy();
    expect(button("Mettre à jour puis installer").disabled).toBe(true);
    await fireEvent.click(button("Installer la version locale"));

    await waitFor(() => expect(onclose).toHaveBeenCalled());
    expect(calls("install_skills")).toEqual([{ plan: offline, project: "/project" }]);
  });

  it("requires accepting host warnings before any install", async () => {
    bridge({
      ...refreshes,
      prepare_install: () => plan(syncStatus({ behind: 1 }), { warnings: ["one dépend de claude-code"] }),
    });
    render(GitSyncDialog, { request, onclose: vi.fn() });

    await screen.findByText("one dépend de claude-code");
    expect(button("Installer la version locale").disabled).toBe(true);
    expect(button("Mettre à jour puis installer").disabled).toBe(true);
    await fireEvent.click(screen.getByRole("checkbox", { hidden: true }));
    expect(button("Installer la version locale").disabled).toBe(false);
    expect(button("Mettre à jour puis installer").disabled).toBe(false);
  });

  it("blocks the update while the editor holds unsaved changes", async () => {
    store.editorDirty = true;
    bridge({ ...refreshes, prepare_install: () => plan(syncStatus({ behind: 1 })) });
    render(GitSyncDialog, { request, onclose: vi.fn() });

    await screen.findByText(/1 commit\(s\) distant\(s\)/);
    expect(button("Mettre à jour puis installer").disabled).toBe(true);
    expect(button("Installer la version locale").disabled).toBe(false);
  });
});

describe("fetch without install", () => {
  it("disables the update when histories diverge", async () => {
    bridge({ check_library_git: () => syncStatus({ ahead: 1, behind: 1, blocked: "Les historiques local et distant divergent." }) });
    render(GitSyncDialog, { onclose: vi.fn() });

    await screen.findByText("Les historiques local et distant divergent.");
    expect(button("Mettre à jour la bibliothèque").disabled).toBe(true);
    expect(screen.queryByRole("button", { name: "Installer la version locale", hidden: true })).toBeNull();
  });
});
