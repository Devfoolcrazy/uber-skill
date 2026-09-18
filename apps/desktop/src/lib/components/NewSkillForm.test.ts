import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { store } from "$lib/store.svelte";
import { bridge, libraryView, registryView, skill } from "../../test/bridge";
import NewSkillForm from "./NewSkillForm.svelte";

beforeEach(() => {
  store.kind = "skill";
  store.libraries = { skill: libraryView([]), agent: null };
  store.config = { library_path: "/lib", agents_path: null, recent_projects: [], editor_command: null };
  store.registry = registryView();
  store.selectedId = null;
  store.editRequest = null;
  store.error = null;
});

describe("creation from the built-in template", () => {
  it("creates with registry values, selects the draft and asks for the editor", async () => {
    const { calls } = bridge({
      create_skill: () => skill("review-pr", { category: "review", tags: ["git"] }),
      get_registry: () => registryView(),
    });
    const onclose = vi.fn();
    render(NewSkillForm, { onclose });

    await fireEvent.input(screen.getByPlaceholderText(/identifiant/), { target: { value: "review-pr" } });
    await fireEvent.input(screen.getByPlaceholderText("description"), { target: { value: "Relit une PR" } });
    await fireEvent.change(screen.getByLabelText("Catégorie"), { target: { value: "review" } });
    await fireEvent.click(screen.getByRole("button", { name: "git" }));
    await fireEvent.click(screen.getByRole("button", { name: "Créer et éditer" }));

    await waitFor(() => expect(onclose).toHaveBeenCalled());
    expect(calls("create_skill")).toEqual([
      { kind: "skill", id: "review-pr", description: "Relit une PR", category: "review", tags: ["git"], hosts: [] },
    ]);
    expect(store.selectedId).toBe("review-pr");
    expect(store.editRequest).toBe("review-pr");
    expect(store.toast).toContain("pas encore publié");
  });

  it("keeps the form open and explains when the name is already taken", async () => {
    bridge({ create_skill: () => { throw { code: "already-exists", message: "skill already exists: notes", details: "notes" }; } });
    const onclose = vi.fn();
    render(NewSkillForm, { onclose });

    await fireEvent.input(screen.getByPlaceholderText(/identifiant/), { target: { value: "notes" } });
    await fireEvent.click(screen.getByRole("button", { name: "Créer et éditer" }));

    await waitFor(() => expect(store.error).toBe("Un élément porte déjà ce nom.\nnotes"));
    expect(onclose).not.toHaveBeenCalled();
    expect(store.editRequest).toBeNull();
  });

  it("refuses an invalid identifier before calling anything", async () => {
    const { calls } = bridge({});
    render(NewSkillForm, { onclose: vi.fn() });
    await fireEvent.input(screen.getByPlaceholderText(/identifiant/), { target: { value: "Review PR" } });
    expect((screen.getByRole("button", { name: "Créer et éditer" }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByText(/Minuscules, chiffres et tirets/)).toBeTruthy();
    expect(calls("create_skill")).toEqual([]);
  });
});
