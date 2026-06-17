import { describe, it, expect } from "vitest";
import {
  formatClipPreview,
  clipTypeLabel,
  textCharCount,
  dayBucket,
  type ClipItem,
} from "../src/clip";

function textItem(text: string): ClipItem {
  return {
    id: 1, kind: "text", text, image_path: null, thumb_path: null,
    source_app: null, pinned: false, created_at: 0,
  };
}

function imageItem(): ClipItem {
  return {
    id: 2, kind: "image", text: null, image_path: "x.png", thumb_path: "t.png",
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
    expect(formatClipPreview(imageItem())).toBe("[图片]");
  });
});

describe("clipTypeLabel", () => {
  it("returns 文本 for text and 图片 for image", () => {
    expect(clipTypeLabel(textItem("hi"))).toBe("文本");
    expect(clipTypeLabel(imageItem())).toBe("图片");
  });
});

describe("textCharCount", () => {
  it("counts characters for text items", () => {
    expect(textCharCount(textItem("hello"))).toBe(5);
  });
  it("returns null for image items", () => {
    expect(textCharCount(imageItem())).toBeNull();
  });
});

describe("dayBucket", () => {
  const now = new Date(2026, 5, 17, 10, 0, 0).getTime(); // 2026-06-17 10:00 local
  it("labels same-day items 今天", () => {
    const earlierToday = new Date(2026, 5, 17, 1, 0, 0).getTime();
    expect(dayBucket(earlierToday, now)).toBe("今天");
  });
  it("labels previous-day items 昨天", () => {
    const yesterday = new Date(2026, 5, 16, 23, 0, 0).getTime();
    expect(dayBucket(yesterday, now)).toBe("昨天");
  });
  it("labels older items with an ISO date", () => {
    const older = new Date(2026, 5, 14, 12, 0, 0).getTime();
    expect(dayBucket(older, now)).toBe("2026-06-14");
  });
});
