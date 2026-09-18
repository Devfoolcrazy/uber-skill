import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import type { RefineProposal } from "$lib/api";
import { bridge, skill } from "../../test/bridge";
import RefineDialog from "./RefineDialog.svelte";

const proposal: RefineProposal = {
  id: "one", original: "old", proposed: "new",
  diff: "--- one/SKILL.md\n+++ one/SKILL.md\n@@ -1 +1 @@\n-old\n+new\n", restored_keys: ["name"],
};
const button = (name: string | RegExp) => screen.getByRole("button", { name, hidden: true }) as HTMLButtonElement;
const ask = async (text: string) => {
  await fireEvent.input(screen.getByLabelText("Que doit améliorer Claude ?"), { target: { value: text } });
  await fireEvent.click(button("Proposer"));
};

describe("assisted refinement", () => {
  it("shows the proposal as a diff and writes nothing until accepted", async () => {
    const { calls } = bridge({ refine_propose: () => proposal, refine_accept: () => skill("one", { hash: "refined" }) });
    const onaccepted = vi.fn();
    const onclose = vi.fn();
    render(RefineDialog, { skill: skill("one"), onaccepted, onclose });
    expect(button("Proposer").disabled).toBe(true);

    await ask("Clarifie les étapes");
    const diff = await screen.findByLabelText("Différences proposées");
    expect(diff.textContent).toContain("+new");
    expect(screen.getByText(/rétablies : name/)).toBeTruthy();
    expect(calls("refine_propose")).toEqual([{ kind: "skill", id: "one", instruction: "Clarifie les étapes" }]);
    expect(calls("refine_accept")).toEqual([]);

    await fireEvent.click(button("Accepter"));
    await waitFor(() => expect(onclose).toHaveBeenCalled());
    expect(calls("refine_accept")).toEqual([{ kind: "skill", proposal }]);
    expect(onaccepted).toHaveBeenCalledWith(expect.objectContaining({ hash: "refined" }));
  });

  it("rejecting keeps the file as it was", async () => {
    const { calls } = bridge({ refine_propose: () => proposal });
    const onaccepted = vi.fn();
    const onclose = vi.fn();
    render(RefineDialog, { skill: skill("one"), onaccepted, onclose });

    await ask("Clarifie");
    await screen.findByLabelText("Différences proposées");
    await fireEvent.click(button("Rejeter"));

    expect(onclose).toHaveBeenCalled();
    expect(onaccepted).not.toHaveBeenCalled();
    expect(calls("refine_accept")).toEqual([]);
  });

  it("explains a missing Claude CLI and lets the request be changed", async () => {
    bridge({ refine_propose: () => { throw { code: "refine.claude-not-found", message: "not found", details: null }; } });
    render(RefineDialog, { skill: skill("one"), onaccepted: vi.fn(), onclose: vi.fn() });

    await ask("Clarifie");
    expect((await screen.findByRole("alert", { hidden: true })).textContent).toContain("La commande « claude » est introuvable");
    expect(button("Proposer").disabled).toBe(false);
  });

  it("discards the answer of an abandoned request", async () => {
    let answer: (p: RefineProposal) => void = () => {};
    bridge({ refine_propose: () => new Promise<RefineProposal>((resolve) => (answer = resolve)) });
    render(RefineDialog, { skill: skill("one"), onaccepted: vi.fn(), onclose: vi.fn() });

    await ask("Clarifie");
    await fireEvent.click(await screen.findByRole("button", { name: "Abandonner", hidden: true }));
    answer(proposal);
    await Promise.resolve();

    expect(screen.queryByLabelText("Différences proposées")).toBeNull();
    expect(button("Proposer")).toBeTruthy();
  });

  it("cannot accept an empty proposal", async () => {
    bridge({ refine_propose: () => ({ ...proposal, diff: "", proposed: "old", restored_keys: [] }) });
    render(RefineDialog, { skill: skill("one"), onaccepted: vi.fn(), onclose: vi.fn() });

    await ask("Clarifie");
    await screen.findByText("Claude ne propose aucun changement.");
    expect(button("Accepter").disabled).toBe(true);
  });
});
