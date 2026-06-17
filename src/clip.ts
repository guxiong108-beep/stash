import { invoke } from "@tauri-apps/api/core";

export interface ClipItem {
  id: number;
  kind: "text" | "image";
  text: string | null;
  image_path: string | null;
  thumb_path: string | null;
  source_app: string | null;
  pinned: boolean;
  created_at: number;
}

/** One-line, length-capped preview for a clipboard row. */
export function formatClipPreview(item: ClipItem): string {
  if (item.kind === "image") return "[图片]";
  const oneLine = (item.text ?? "").replace(/\s+/g, " ").trim();
  return oneLine.length > 80 ? oneLine.slice(0, 80) + "…" : oneLine;
}

/** Chinese type label for the detail pane. */
export function clipTypeLabel(item: ClipItem): string {
  return item.kind === "image" ? "图片" : "文本";
}

/** Character count for text items; null for images. */
export function textCharCount(item: ClipItem): number | null {
  return item.kind === "text" ? (item.text ?? "").length : null;
}

export const clipApi = {
  list: (limit = 200) => invoke<ClipItem[]>("clip_list", { limit }),
  search: (query: string, limit = 200) =>
    invoke<ClipItem[]>("clip_search", { query, limit }),
  setPinned: (id: number, pinned: boolean) =>
    invoke<void>("clip_set_pinned", { id, pinned }),
  remove: (id: number) => invoke<void>("clip_delete", { id }),
  clear: () => invoke<void>("clip_clear"),
};
