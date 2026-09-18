import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it } from "vitest";
import type { InstalledSkill } from "$lib/api";
import { store } from "$lib/store.svelte";
import { bridge, libraryView, skill } from "../../test/bridge";
import SkillDetail from "./SkillDetail.svelte";

const TEXT = "---\nname: one\ndescription: One\n---\nBody\n";
const installedAs = (state: InstalledSkill["state"]) => [{ id: "one", state } as InstalledSkill];

beforeEach(() => {
  store.kind = "skill";
  store.libraries = { skill: libraryView([skill("one")]), agent: null };
  store.selectedId = "one";
  store.projectPath = "/work/game";
  store.target = { kind: "claude-code" };
  store.projectStatus = { skill: installedAs("up-to-date"), agent: [] };
  store.gitDialog = null;
  store.toast = null;
  store.registry = null;
});

describe("project copy after an edit", () => {
  it("offers to update the project copy as soon as a saved change leaves it behind", async () => {
    bridge({
      read_skill_file: () => TEXT,
      write_skill_file: () => skill("one", { hash: "hash-edited" }),
      project_status: ({ kind }) => (kind === "skill" ? installedAs("library-updated") : []),
    });
    render(SkillDetail);
    expect(screen.queryByRole("button", { name: "Mettre à jour la copie du projet" })).toBeNull();

    await fireEvent.click(screen.getByRole("button", { name: /Éditer/ }));
    const editor = await waitFor(() => screen.getByRole("textbox") as HTMLTextAreaElement);
    await waitFor(() => expect(editor.value).toBe(TEXT));
    await fireEvent.input(editor, { target: { value: TEXT + "More\n" } });
    await fireEvent.click(screen.getByRole("button", { name: /Enregistrer/ }));

    await fireEvent.click(await screen.findByRole("button", { name: "Mettre à jour la copie du projet" }));
    expect(store.toast).toContain("La copie dans game est en retard");
    expect(store.gitDialog).toEqual({
      install: { kind: "skill", ids: ["one"], project: "/work/game", target: { kind: "claude-code" } },
    });
  });
});
