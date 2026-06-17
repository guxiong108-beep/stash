import { describe, it, expect } from "vitest";
import { formatClipPreview, type ClipItem } from "../src/clip";

function textItem(text: string): ClipItem {
  return {
    id: 1, kind: "text", text, image_path: null, thumb_path: null,
    source_app: null, pinned: false, created_at: 0,
  };
}

describe("formatClipPreview", () => {
  it("returns text trimmed to one line", () => {
    const it1 = textItem("hello\nworld\nfoo");
    expect(formatClipPreview(it1)).toBe("hello world foo");
  });

  it("truncates long text with an ellipsis", () => {
    const long = "a".repeat(120);
    const out = formatClipPreview(textItem(long));
    expect(out.length).toBeLessThanOrEqual(81); // 80 + ellipsis
    expect(out.endsWith("…")).toBe(true);
  });

  it("labels image items", () => {
    const img: ClipItem = {
      id: 2, kind: "image", text: null, image_path: "x.png", thumb_path: "t.png",
      source_app: null, pinned: false, created_at: 0,
    };
    expect(formatClipPreview(img)).toBe("[图片]");
  });
});
