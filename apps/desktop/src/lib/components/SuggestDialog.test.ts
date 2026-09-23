import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ProjectOverview, SuggestContext, Suggestions } from "$lib/api";
import { store } from "$lib/store.svelte";
import { bridge } from "../../test/bridge";
import SuggestDialog from "./SuggestDialog.svelte";

const project: ProjectOverview = {
  path: "/work/game", name: "game", exists: true, global: false, behind: 0,
  installs: [{ target: { kind: "claude-code" }, kind: "skill", items: [] }],
};
const context: SuggestContext = {
  project: "/work/game", empty: false, tree: ["src/", "src/main.rs", "Cargo.toml"], tree_truncated: false,
  excerpts: [{ path: "README.md", text: "# Game", truncated: false }, { path: "Cargo.toml", text: "[package]", truncated: true }],
  installed: ["skill:seo"], catalogue_items: 27,
};
const answer: Suggestions = {
  summary: "Un jeu en Rust.",
  suggestions: [
    { kind: "skill", id: "diagnosing-bugs", reason: "Cargo.toml présent.", confidence: "high", installed: false },
    { kind: "agent", id: "reviewer", reason: "Relecture des diffs.", confidence: "medium", installed: false },
    { kind: "skill", id: "seo", reason: "Déjà là.", confidence: "low", installed: true },
  ],
  unknown: ["made-up"],
};
const button = (name: string | RegExp) => screen.getByRole("button", { name, hidden: true }) as HTMLButtonElement;

beforeEach(() => {
  store.gitDialog = null;
});

describe("skill suggestions for a project", () => {
  it("says what leaves the machine, then turns the checked suggestions into one install review", async () => {
    const { calls } = bridge({ suggest_context: () => context, suggest_items: () => answer });
    const onclose = vi.fn();
    render(SuggestDialog, { project, onclose });

    const disclosure = await screen.findByText(/Claude reçoit l’index de la bibliothèque \(27 éléments\)/);
    expect(disclosure.textContent).toContain("3 entrées");
    expect(disclosure.textContent).toContain("README.md, Cargo.toml");
    expect(disclosure.textContent).toContain("déjà installé (1)");
    expect(disclosure.textContent).toContain("Aucun fichier source");

    await fireEvent.input(screen.getByLabelText(/Type de projet/), { target: { value: "jeu" } });
    await fireEvent.click(button("Demander à Claude"));

    await screen.findByText("Un jeu en Rust.");
    expect(calls("suggest_items")).toEqual([{ project: "/work/game", profile: { kind: "jeu", stack: null, goal: null } }]);
    const boxes = screen.getAllByRole("checkbox", { hidden: true }) as HTMLInputElement[];
    expect(boxes.map((b) => b.checked)).toEqual([true, true, false]);
    expect(screen.getByText("confiance élevée")).toBeTruthy();
    expect(screen.getByText(/absent de la bibliothèque : made-up/)).toBeTruthy();
    expect(button(/Installer la sélection/).textContent).toContain("(2)");

    await fireEvent.click(screen.getByLabelText("Installer reviewer"));
    expect(button(/Installer la sélection/).textContent).toContain("(1)");
    await fireEvent.click(button(/Installer la sélection/));

    expect(onclose).toHaveBeenCalled();
    expect(store.gitDialog).toEqual({ install: [{ project: "/work/game", kind: "skill", target: { kind: "claude-code" }, ids: ["diagnosing-bugs"] }] });
  });

  it("drops agents when the chosen target has no folder for them", async () => {
    bridge({ suggest_context: () => context, suggest_items: () => answer });
    render(SuggestDialog, { project, onclose: vi.fn() });
    await screen.findByText(/Claude reçoit/);
    await fireEvent.change(screen.getByLabelText("Installer dans"), { target: { value: "agents" } });
    await fireEvent.click(button("Demander à Claude"));
    await screen.findByText("Un jeu en Rust.");

    expect((screen.getByLabelText("Installer reviewer") as HTMLInputElement).disabled).toBe(true);
    expect(screen.getByText("pas pour cette cible")).toBeTruthy();
    expect(button(/Installer la sélection/).textContent).toContain("(1)");
    await fireEvent.click(button(/Installer la sélection/));
    expect(store.gitDialog).toEqual({ install: [{ project: "/work/game", kind: "skill", target: { kind: "agents" }, ids: ["diagnosing-bugs"] }] });
  });

  it("asks for a description when the folder is empty and explains a missing Claude CLI", async () => {
    bridge({
      suggest_context: () => ({ ...context, empty: true, tree: [], excerpts: [], installed: [] }),
      suggest_items: () => { throw { code: "refine.claude-not-found", message: "not found", details: null }; },
    });
    render(SuggestDialog, { project, onclose: vi.fn() });

    expect((await screen.findByText(/semble vide ou tout neuf/)).textContent).toBeTruthy();
    await fireEvent.click(button("Demander à Claude"));
    expect((await screen.findByRole("alert", { hidden: true })).textContent).toContain("La commande « claude » est introuvable");
    expect(button("Demander à Claude").disabled).toBe(false);
  });

  it("discards the answer of an abandoned request", async () => {
    let reply: (s: Suggestions) => void = () => {};
    bridge({ suggest_context: () => context, suggest_items: () => new Promise<Suggestions>((resolve) => (reply = resolve)) });
    render(SuggestDialog, { project, onclose: vi.fn() });
    await screen.findByText(/Claude reçoit/);

    await fireEvent.click(button("Demander à Claude"));
    await fireEvent.click(await screen.findByRole("button", { name: "Abandonner", hidden: true }));
    reply(answer);
    await Promise.resolve();

    expect(screen.queryByText("Un jeu en Rust.")).toBeNull();
    await waitFor(() => expect(button("Demander à Claude")).toBeTruthy());
  });
});
