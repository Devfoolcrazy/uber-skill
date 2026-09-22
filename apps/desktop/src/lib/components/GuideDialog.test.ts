import { render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import GuideDialog from "./GuideDialog.svelte";

describe("embedded guide", () => {
  it("renders the guide with its sections and no raw markdown", () => {
    render(GuideDialog, { onclose: vi.fn() });
    expect(screen.getByRole("heading", { name: "Mode d'emploi" })).toBeTruthy();
    for (const section of ["Les trois lieux", "Lire les indicateurs", "Parcours courant", "Installer", "Projets", "Supprimer"]) {
      expect(screen.getByRole("heading", { name: section })).toBeTruthy();
    }
    expect(screen.getByRole("dialog", { hidden: true }).textContent).not.toMatch(/^#|\|---/m);
  });
});
