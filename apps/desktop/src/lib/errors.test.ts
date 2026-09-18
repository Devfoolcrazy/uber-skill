import { describe, expect, it } from "vitest";
import { errorText } from "./errors";

describe("errorText", () => {
  it("translates a known code and appends the untranslated details", () => {
    expect(errorText({ code: "git-clone-failed", message: "clone failed", details: "fatal: repository not found" }))
      .toBe("Le clonage n’a pas abouti. Vérifiez l’URL, le réseau et vos accès Git.\nfatal: repository not found");
  });

  it("translates block reasons carried by an error", () => {
    expect(errorText({ code: "blocked.diverged", message: "diverged", details: null })).toContain("divergent");
  });

  it("falls back to the English message for an unknown code", () => {
    expect(errorText({ code: "brand-new", message: "something new", details: null })).toBe("something new");
  });

  it("still accepts strings and exceptions", () => {
    expect(errorText("plain")).toBe("plain");
    expect(errorText(new Error("boom"))).toBe("boom");
  });
});
