import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { store } from "$lib/store.svelte";
import { bridge, publicationPreview } from "../../test/bridge";
import PublishLibrary from "./PublishLibrary.svelte";

const button = (name: string) => screen.getByRole("button", { name, hidden: true }) as HTMLButtonElement;

beforeEach(() => {
  store.editorDirty = false;
});

describe("publication", () => {
  it("sends only the checked files with the reviewed snapshot and the edited message", async () => {
    const { calls } = bridge({
      git_publication_preview: () => publicationPreview(),
      publish_library: () => ({ commit: "0123456789abcdef", pushed: true, push_error: null }),
    });
    render(PublishLibrary, { onclose: vi.fn() });

    const include = await screen.findByLabelText("Inclure skills/two/SKILL.md");
    expect(button("Envoyer les commits").disabled).toBe(true);
    await fireEvent.click(include);
    await fireEvent.input(screen.getByLabelText("Message de commit"), { target: { value: "Clarifie two" } });
    await fireEvent.click(button("Commit et push"));

    await screen.findByText(/Publication réussie — commit 01234567/);
    expect(calls("publish_library")).toEqual([
      { snapshot: "preview-1", paths: ["skills/two/SKILL.md"], message: "Clarifie two" },
    ]);
  });

  it("retries a failed push without creating another commit", async () => {
    const previews = [
      publicationPreview(),
      publicationPreview({ files: [], pending_count: 1, pending_commits: ["0123456 Clarifie one"], snapshot: "preview-2" }),
      publicationPreview({ files: [], snapshot: "preview-3" }),
    ];
    const results = [
      { commit: "0123456789abcdef", pushed: false, push_error: { code: "git-failed", message: "git failed", details: "Could not resolve host" } },
      { commit: null, pushed: true, push_error: null },
    ];
    const { calls } = bridge({
      git_publication_preview: () => previews.shift(),
      publish_library: () => results.shift(),
    });
    render(PublishLibrary, { onclose: vi.fn() });

    await fireEvent.click(await screen.findByLabelText("Inclure skills/one/SKILL.md"));
    await fireEvent.click(button("Commit et push"));
    await screen.findByText(/a été créé localement, mais son envoi a échoué/);
    await screen.findByText(/1 commit\(s\) local\(aux\) seront également envoyés/);

    await fireEvent.click(button("Envoyer les commits"));

    await waitFor(() => expect(calls("publish_library")).toHaveLength(2));
    expect(calls("publish_library")[1]).toMatchObject({ snapshot: "preview-2", paths: [] });
    await screen.findByText(/Publication réussie/);
  });

  it("refuses to publish a blocked repository and an empty commit message", async () => {
    bridge({ git_publication_preview: () => publicationPreview({ blocked: "detached-head" }) });
    const { unmount } = render(PublishLibrary, { onclose: vi.fn() });
    await fireEvent.click(await screen.findByLabelText("Inclure skills/one/SKILL.md"));
    expect(button("Commit et push").disabled).toBe(true);
    unmount();

    bridge({ git_publication_preview: () => publicationPreview() });
    render(PublishLibrary, { onclose: vi.fn() });
    await fireEvent.click(await screen.findByLabelText("Inclure skills/one/SKILL.md"));
    expect(button("Commit et push").disabled).toBe(false);
    await fireEvent.input(screen.getByLabelText("Message de commit"), { target: { value: "   " } });
    expect(button("Commit et push").disabled).toBe(true);
  });

  it("warns that unsaved editor changes are not part of the publication", async () => {
    store.editorDirty = true;
    bridge({ git_publication_preview: () => publicationPreview() });
    render(PublishLibrary, { onclose: vi.fn() });
    expect(await screen.findByText(/modifications non enregistrées/)).toBeTruthy();
  });
});
