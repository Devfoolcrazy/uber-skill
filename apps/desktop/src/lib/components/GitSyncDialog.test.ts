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
  get_registry: () => null,
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
    await fireEvent.click(button("Mettre à jour la bibliothèque puis installer"));

    await waitFor(() => expect(onclose).toHaveBeenCalled());
    expect(calls("update_library_git")).toEqual([{ snapshot: "behind" }]);
    expect(calls("install_skills")).toEqual([{ plan: fresh, project: "/project" }]);
    expect(store.checked.size).toBe(0);
  });

  it("installs without showing the dialog when there is nothing to decide", async () => {
    const fresh = plan(syncStatus(), {
      items: [{ id: "one", source: "/lib/skills/one", hash: "hash-one", source_state: "local-draft" }],
    });
    const { calls } = bridge({ ...refreshes, prepare_install: () => fresh, install_skills: () => [] });
    const onclose = vi.fn();
    const { container } = render(GitSyncDialog, { request, onclose });

    await waitFor(() => expect(onclose).toHaveBeenCalled());
    expect(calls("install_skills")).toEqual([{ plan: fresh, project: "/project" }]);
    expect(container.querySelector("dialog")?.hasAttribute("open")).toBe(false);
    expect(store.toast).toContain("1 élément(s) installé(s) dans project (dont 1 avec des modifications non publiées)");
  });

  it("still asks when the library is up to date but something needs attention", async () => {
    store.editorDirty = true;
    const { calls } = bridge({ ...refreshes, prepare_install: () => plan(syncStatus()), install_skills: () => [] });
    const { container } = render(GitSyncDialog, { request, onclose: vi.fn() });

    await screen.findByText(/derniers changements distants/);
    await waitFor(() => expect(container.querySelector("dialog")?.hasAttribute("open")).toBe(true));
    expect(calls("install_skills")).toEqual([]);
    expect(screen.queryByRole("button", { name: /Mettre à jour/, hidden: true })).toBeNull();
    expect(button("Installer dans le projet").classList.contains("primary")).toBe(true);
  });

  it("never falls back to a local install when the update fails", async () => {
    const { calls } = bridge({
      ...refreshes,
      prepare_install: () => plan(syncStatus({ behind: 1 })),
      update_library_git: () => { throw { code: "git-fast-forward-failed", message: "fast-forward failed", details: "error: local changes would be overwritten" }; },
      install_skills: () => [],
    });
    const onclose = vi.fn();
    render(GitSyncDialog, { request, onclose });

    await screen.findByText(/1 commit\(s\) distant\(s\)/);
    await fireEvent.click(button("Mettre à jour la bibliothèque puis installer"));

    const alert = (await screen.findByRole("alert", { hidden: true })).textContent;
    expect(alert).toContain("Mise à jour interrompue.");
    expect(alert).toContain("local changes would be overwritten");
    expect(calls("install_skills")).toEqual([]);
    expect(onclose).not.toHaveBeenCalled();
  });

  it("offers only an explicit local install when freshness cannot be verified", async () => {
    const offline = plan(syncStatus({ verified: false, fetch_error: { code: "git-failed", message: "git failed", details: "Could not resolve host" } }), {
      items: [{ id: "one", source: "/lib/skills/one", hash: "hash-one", source_state: "unverified" }],
    });
    const { calls } = bridge({ ...refreshes, prepare_install: () => offline, install_skills: () => [] });
    const onclose = vi.fn();
    render(GitSyncDialog, { request, onclose });

    await screen.findByText(/Could not resolve host/);
    expect(screen.getByText(/Fraîcheur non vérifiée/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Mettre à jour/, hidden: true })).toBeNull();
    await fireEvent.click(button("Installer sans vérification"));

    await waitFor(() => expect(onclose).toHaveBeenCalled());
    expect(calls("install_skills")).toEqual([{ plan: offline, project: "/project" }]);
  });

  it("requires accepting host warnings before any install", async () => {
    bridge({
      ...refreshes,
      prepare_install: () => plan(syncStatus({ behind: 1 }), { warnings: [{ id: "one", hosts: ["claude-code"], target: "Cursor" }] }),
    });
    render(GitSyncDialog, { request, onclose: vi.fn() });

    await screen.findByText("one est déclaré pour claude-code, mais la cible est Cursor.");
    expect(button("Installer la version actuelle de la bibliothèque").disabled).toBe(true);
    expect(button("Mettre à jour la bibliothèque puis installer").disabled).toBe(true);
    await fireEvent.click(screen.getByRole("checkbox", { hidden: true }));
    expect(button("Installer la version actuelle de la bibliothèque").disabled).toBe(false);
    expect(button("Mettre à jour la bibliothèque puis installer").disabled).toBe(false);
  });

  it("blocks the update while the editor holds unsaved changes", async () => {
    store.editorDirty = true;
    bridge({ ...refreshes, prepare_install: () => plan(syncStatus({ behind: 1 })) });
    render(GitSyncDialog, { request, onclose: vi.fn() });

    await screen.findByText(/1 commit\(s\) distant\(s\)/);
    expect(button("Mettre à jour la bibliothèque puis installer").disabled).toBe(true);
    expect(button("Installer la version actuelle de la bibliothèque").disabled).toBe(false);
  });
});

describe("fetch without install", () => {
  it("disables the update when histories diverge", async () => {
    bridge({ check_library_git: () => syncStatus({ ahead: 1, behind: 1, blocked: "diverged" }) });
    render(GitSyncDialog, { onclose: vi.fn() });

    await screen.findByText(/Les historiques local et distant divergent/);
    expect(button("Mettre à jour la bibliothèque").disabled).toBe(true);
    expect(screen.queryByRole("button", { name: /Installer/, hidden: true })).toBeNull();
  });
});
