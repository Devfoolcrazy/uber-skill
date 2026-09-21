import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { store } from "$lib/store.svelte";
import { bridge, libraryView, registryView, usage } from "../../test/bridge";
import LibrarySettings from "./LibrarySettings.svelte";

const reloads = { scan_library: () => libraryView([]), project_status: () => [] };
const button = (name: string | RegExp) => screen.getByRole("button", { name, hidden: true }) as HTMLButtonElement;

beforeEach(() => {
  store.editorDirty = false;
  store.projectPath = null;
  store.registry = null;
  store.config = { library_path: "/lib", agents_path: null, recent_projects: [], editor_command: null, tracked_projects: [] };
});

describe("registry administration", () => {
  it("offers to create the registry from the values in use", async () => {
    const { calls } = bridge({
      get_registry: () => registryView({ exists: false }),
      registry_init: () => registryView(),
    });
    render(LibrarySettings, { onclose: vi.fn() });

    await fireEvent.click(await screen.findByRole("button", { name: /Créer le référentiel/, hidden: true }));

    await waitFor(() => expect(store.registry?.exists).toBe(true));
    expect(calls("registry_init")).toHaveLength(1);
  });

  it("renames a used value everywhere, then reloads the library", async () => {
    const renamed = registryView({ tags: [usage("vcs", true, ["one", "two"])] });
    const { calls } = bridge({ ...reloads, get_registry: () => registryView(), registry_rename: () => renamed });
    render(LibrarySettings, { onclose: vi.fn() });

    await fireEvent.click(await screen.findByRole("button", { name: "Renommer git", hidden: true }));
    await fireEvent.input(screen.getByLabelText("Nouveau nom pour git"), { target: { value: "vcs" } });
    await fireEvent.click(button("Renommer partout"));

    await waitFor(() => expect(calls("scan_library")).toHaveLength(2));
    expect(calls("registry_rename")).toEqual([{ facet: "tag", from: "git", to: "vcs" }]);
  });

  it("requires an explicit replacement or removal before deleting a used value", async () => {
    const { calls } = bridge({ ...reloads, get_registry: () => registryView(), registry_remove: () => registryView() });
    render(LibrarySettings, { onclose: vi.fn() });

    await fireEvent.click(await screen.findByRole("button", { name: "Supprimer git", hidden: true }));
    expect(button("Supprimer").disabled).toBe(true);
    await fireEvent.change(screen.getByLabelText("Que faire des éléments utilisant git"), { target: { value: "quality" } });
    await fireEvent.click(button("Supprimer"));
    await waitFor(() => expect(calls("registry_remove")).toHaveLength(1));
    expect(calls("registry_remove")[0]).toEqual({ facet: "tag", value: "git", replacement: "quality", strip: false });
  });

  it("deletes an unused value without touching any item", async () => {
    const { calls } = bridge({ get_registry: () => registryView(), registry_remove: () => registryView() });
    render(LibrarySettings, { onclose: vi.fn() });

    await fireEvent.click(await screen.findByRole("button", { name: "Supprimer quality", hidden: true }));
    await fireEvent.click(button("Supprimer"));

    await waitFor(() => expect(calls("registry_remove")).toHaveLength(1));
    expect(calls("registry_remove")[0]).toEqual({ facet: "tag", value: "quality", replacement: null, strip: false });
    expect(calls("scan_library")).toEqual([]);
  });

  it("accepts an unknown value or maps it onto an allowed one", async () => {
    const { calls } = bridge({
      ...reloads,
      get_registry: () => registryView(),
      registry_add: () => registryView(),
      registry_rename: () => registryView(),
    });
    render(LibrarySettings, { onclose: vi.fn() });

    const tags = await screen.findByRole("region", { name: "Tags", hidden: true });
    expect(within(tags).getByText("Valeurs inconnues")).toBeTruthy();
    await fireEvent.click(button("Ajouter legacy au référentiel"));
    await waitFor(() => expect(calls("registry_add")).toEqual([{ facet: "tag", value: "legacy" }]));

    await fireEvent.click(button("Remplacer legacy"));
    await fireEvent.change(screen.getByLabelText("Valeur autorisée remplaçant legacy"), { target: { value: "git" } });
    await fireEvent.click(button(/Remplacer dans 1 élément/));
    await waitFor(() => expect(calls("registry_rename")).toEqual([{ facet: "tag", from: "legacy", to: "git" }]));
  });

  it("does not rewrite items while the editor holds unsaved changes", async () => {
    store.editorDirty = true;
    bridge({ get_registry: () => registryView() });
    render(LibrarySettings, { onclose: vi.fn() });

    await fireEvent.click(await screen.findByRole("button", { name: "Renommer git", hidden: true }));
    expect(button("Renommer partout").disabled).toBe(true);
    await fireEvent.click(button("Renommer quality"));
    expect(button("Renommer partout").disabled).toBe(false);
  });
});
