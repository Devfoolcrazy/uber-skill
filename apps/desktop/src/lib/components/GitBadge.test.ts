import { render, screen, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it } from "vitest";
import { store } from "$lib/store.svelte";
import { bridge, libraryView, skill } from "../../test/bridge";
import GitBadge from "./GitBadge.svelte";
import SkillList from "./SkillList.svelte";

beforeEach(() => {
  store.kind = "skill";
  store.query = "";
  store.selectedTags = [];
  store.category = null;
  store.host = null;
  store.projectStatus = { skill: [], agent: [] };
  store.gitStates = { skill: {}, agent: {} };
  store.config = { library_path: "/lib", agents_path: null, recent_projects: [], editor_command: null, tracked_projects: [] };
});

describe("difference with the remote", () => {
  it("names each kind of difference and shows nothing for an identical item", () => {
    const { unmount } = render(GitBadge, { state: { modified: true, unpublished: false, outdated: true }, full: true });
    expect(screen.getByLabelText("Modifications locales non publiées").textContent).toContain("Modifié");
    expect(screen.getByLabelText(/Version plus récente sur le dépôt distant/).textContent).toContain("À récupérer");
    expect(screen.queryByLabelText(/Commit local/)).toBeNull();
    unmount();

    render(GitBadge, { state: undefined });
    expect(screen.queryByRole("img")).toBeNull();
  });

  it("decorates the list after a scan and counts what is left to publish", async () => {
    bridge({
      scan_library: () => libraryView([skill("edited"), skill("pushed"), skill("behind")]),
      library_git_states: ({ kind }) =>
        kind === "skill"
          ? { edited: { modified: true, unpublished: false, outdated: false }, behind: { modified: false, unpublished: false, outdated: true } }
          : { reviewer: { modified: false, unpublished: true, outdated: false } },
    });
    render(SkillList);
    await store.refreshLibrary("skill");
    await store.refreshGitStates("agent");

    await waitFor(() => expect(screen.getAllByRole("img")).toHaveLength(2));
    expect(screen.getByLabelText("Modifications locales non publiées")).toBeTruthy();
    // One edited skill and one agent with an unpushed commit; a remote-only change is not ours to publish.
    expect(store.unpublishedCount).toBe(2);
  });
});
