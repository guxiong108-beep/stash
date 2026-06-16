import { describe, it, expect } from "vitest";
import { resolveTheme, applyTheme } from "../src/theme";

describe("resolveTheme", () => {
  it("defaults to warm for undefined or unknown input", () => {
    expect(resolveTheme(undefined)).toBe("warm");
    expect(resolveTheme("nonsense")).toBe("warm");
  });

  it("returns light when input is light", () => {
    expect(resolveTheme("light")).toBe("light");
  });
});

describe("applyTheme", () => {
  it("sets the data-theme attribute on the element", () => {
    const el = document.createElement("div");
    applyTheme("warm", el);
    expect(el.getAttribute("data-theme")).toBe("warm");
    applyTheme("light", el);
    expect(el.getAttribute("data-theme")).toBe("light");
  });
});
