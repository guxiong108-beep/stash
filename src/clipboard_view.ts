import { clipApi, formatClipPreview, type ClipItem } from "./clip";
import { convertFileSrc } from "@tauri-apps/api/core";

const listEl = () => document.getElementById("clip-list") as HTMLUListElement;
const hintEl = () => document.getElementById("hint") as HTMLElement;

function escapeHtml(s: string): string {
  const d = document.createElement("div");
  d.textContent = s;
  return d.innerHTML;
}

function rowHtml(item: ClipItem): string {
  const pin = item.pinned ? "📌" : "📍";
  const thumb =
    item.kind === "image" && item.thumb_path
      ? `<img class="clip-row__thumb" src="${convertFileSrc(item.thumb_path)}" />`
      : "";
  return `<li class="clip-row" data-id="${item.id}">
    ${thumb}
    <span class="clip-row__text">${escapeHtml(formatClipPreview(item))}</span>
    <span class="clip-row__actions">
      <button data-act="pin" title="置顶">${pin}</button>
      <button data-act="del" title="删除">🗑</button>
    </span>
  </li>`;
}

export async function renderClipboard(query = ""): Promise<void> {
  const items = query ? await clipApi.search(query) : await clipApi.list();
  const ul = listEl();
  ul.innerHTML = items.map(rowHtml).join("");
  hintEl().style.display = items.length ? "none" : "block";
}

/** Wire click handlers once. */
export function bindClipboardActions(): void {
  listEl().addEventListener("click", async (e) => {
    const btn = (e.target as HTMLElement).closest("button[data-act]");
    if (!btn) return;
    const row = btn.closest(".clip-row") as HTMLElement;
    const id = Number(row.dataset.id);
    const act = (btn as HTMLElement).dataset.act;
    if (act === "del") await clipApi.remove(id);
    if (act === "pin") {
      const pinned = btn.textContent?.includes("📌") ?? false;
      await clipApi.setPinned(id, !pinned);
    }
    await renderClipboard();
  });
}
