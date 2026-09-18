import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it } from "vitest";
import { store } from "$lib/store.svelte";
import { registryView } from "../../test/bridge";
import MetaFields from "./MetaFields.svelte";

beforeEach(() => {
  store.registry = registryView();
});

describe("with a registry", () => {
  it("offers allowed values only, and keeps unknown ones already on the item visible", () => {
    render(MetaFields, { category: "old-cat", tags: "git, legacy" });

    const options = [...(screen.getByLabelText("Catégorie") as HTMLSelectElement).options].map((o) => o.text);
    expect(options).toEqual(["Aucune", "review", "writing", "old-cat (inconnue)"]);
    expect(screen.getByRole("button", { name: "git" }).getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByRole("button", { name: "quality" }).getAttribute("aria-pressed")).toBe("false");
    expect(screen.getByRole("button", { name: "legacy (inconnu)" })).toBeTruthy();
  });

  it("toggles tags and lets an unknown tag be removed but not added back", async () => {
    const props = $state({ category: "", tags: "git, brand-new" });
    render(MetaFields, {
      get category() { return props.category; }, set category(v) { props.category = v; },
      get tags() { return props.tags; }, set tags(v) { props.tags = v; },
    });

    await fireEvent.click(screen.getByRole("button", { name: "quality" }));
    expect(props.tags).toBe("brand-new, git, quality");
    await fireEvent.click(screen.getByRole("button", { name: "brand-new (inconnu)" }));
    expect(props.tags).toBe("git, quality");
    expect(screen.queryByRole("button", { name: /brand-new/ })).toBeNull();
  });
});

describe("without a registry", () => {
  it("falls back to free text", () => {
    store.registry = registryView({ exists: false });
    render(MetaFields, { category: "anything", tags: "a, b" });
    expect((screen.getByLabelText("Catégorie") as HTMLInputElement).value).toBe("anything");
    expect((screen.getByLabelText("Tags") as HTMLInputElement).value).toBe("a, b");
  });
});
