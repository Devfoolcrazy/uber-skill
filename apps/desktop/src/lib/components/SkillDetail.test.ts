import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { openUrl } from "@tauri-apps/plugin-opener";
import { beforeEach, describe, expect, it, vi } from "vitest";
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
  store.editRequest = null;
  store.error = null;
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

describe("a just-created draft", () => {
  it("opens straight in the editor, once", async () => {
    bridge({ read_skill_file: () => TEXT });
    store.editRequest = "one";
    render(SkillDetail);

    const editor = await waitFor(() => screen.getByRole("textbox") as HTMLTextAreaElement);
    await waitFor(() => expect(editor.value).toBe(TEXT));
    expect(store.editRequest).toBeNull();
  });
});

describe("links in the preview", () => {
  const MAIN = "---\nname: one\ndescription: One\n---\nSee [the cheatsheet](cheatsheet.md), [a chapter](chapters/ch01.md),\n[a missing file](nope.md), [outside](../other/SKILL.md) and [the site](https://example.com/doc).\n";
  const files: Record<string, string> = {
    "SKILL.md": MAIN,
    "cheatsheet.md": "# Cheatsheet\n\nBack to [chapter one](chapters/ch01.md).\n",
    "chapters/ch01.md": "# Chapter one\n\nSee the [cheatsheet](../cheatsheet.md).\n",
  };
  const open = () => {
    store.libraries = { skill: libraryView([skill("one", { files: Object.keys(files) })]), agent: null };
    bridge({ read_skill_file: ({ rel }) => files[rel as string] });
    render(SkillDetail);
  };

  it("opens a linked file of the skill in place, rendered, with a way back", async () => {
    open();
    const pathname = location.pathname;
    await fireEvent.click(await screen.findByRole("link", { name: "the cheatsheet" }));
    expect(await screen.findByRole("heading", { name: "Cheatsheet" })).toBeTruthy();
    expect(location.pathname).toBe(pathname);

    // Relative to the file being read, in both directions.
    await fireEvent.click(screen.getByRole("link", { name: "chapter one" }));
    expect(await screen.findByRole("heading", { name: "Chapter one" })).toBeTruthy();
    await fireEvent.click(screen.getByRole("link", { name: "cheatsheet" }));
    expect(await screen.findByRole("heading", { name: "Cheatsheet" })).toBeTruthy();

    await fireEvent.click(screen.getByRole("button", { name: "← SKILL.md" }));
    expect(await screen.findByRole("link", { name: "the cheatsheet" })).toBeTruthy();
  });

  it("reports a broken or escaping link instead of navigating, and sends web links to the browser", async () => {
    open();
    await fireEvent.click(await screen.findByRole("link", { name: "a missing file" }));
    expect(store.error).toBe("Lien introuvable dans ce skill : nope.md");
    await fireEvent.click(screen.getByRole("link", { name: "outside" }));
    expect(store.error).toBe("Lien introuvable dans ce skill : ../other/SKILL.md");
    expect(screen.getByRole("link", { name: "the cheatsheet" })).toBeTruthy();

    await fireEvent.click(screen.getByRole("link", { name: "the site" }));
    expect(vi.mocked(openUrl)).toHaveBeenCalledWith("https://example.com/doc");
  });
});
