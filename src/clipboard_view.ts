import {
  clipApi,
  formatClipPreview,
  clipTypeLabel,
  textCharCount,
  dayBucket,
  type ClipItem,
} from "./clip";
import { convertFileSrc } from "@tauri-apps/api/core";

let items: ClipItem[] = [];
let selectedId: number | null = null;

const listEl = () => document.getElementById("clip-list") as HTMLUListElement;
const detailEl = () => document.getElementById("clip-detail") as HTMLElement;
const bodyEl = () =>
  document.querySelector(".command-bar__body") as HTMLElement;
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
      : `<span class="clip-row__thumb clip-row__thumb--text">T</span>`;
  const source = item.source_app
    ? `<span class="clip-row__source" title="来源">${escapeHtml(item.source_app)}</span>`
    : "";
  const sel = item.id === selectedId ? " is-selected" : "";
  return `<li class="clip-row${sel}" data-id="${item.id}" data-pinned="${item.pinned}">
    ${thumb}
    <span class="clip-row__text">${escapeHtml(formatClipPreview(item))}</span>
    ${source}
    <span class="clip-row__actions">
      <button data-act="pin" title="置顶">${pin}</button>
      <button data-act="del" title="删除">🗑</button>
    </span>
  </li>`;
}

function groupHeader(label: string): string {
  return `<li class="clip-group" aria-hidden="true">${label}</li>`;
}

/** Build the list HTML: a 置顶 section for pinned items, then day-bucketed groups. */
function groupedListHtml(rows: ClipItem[], nowMs: number): string {
  const pinned = rows.filter((i) => i.pinned);
  const rest = rows.filter((i) => !i.pinned);
  let html = "";
  if (pinned.length) {
    html += groupHeader("置顶") + pinned.map(rowHtml).join("");
  }
  let lastBucket = "";
  for (const item of rest) {
    const bucket = dayBucket(item.created_at, nowMs);
    if (bucket !== lastBucket) {
      html += groupHeader(bucket);
      lastBucket = bucket;
    }
    html += rowHtml(item);
  }
  return html;
}

function metaRow(label: string, value: string): string {
  return `<div class="clip-detail__metarow"><span class="clip-detail__metakey">${label}</span><span class="clip-detail__metaval">${escapeHtml(value)}</span></div>`;
}

function detailHtml(item: ClipItem | null): string {
  if (!item) {
    return `<p class="clip-detail__empty">选择左侧条目查看详情</p>`;
  }
  const preview =
    item.kind === "image" && item.image_path
      ? `<img class="clip-detail__img" src="${convertFileSrc(item.image_path)}" />`
      : `<div class="clip-detail__text">${escapeHtml(item.text ?? "")}</div>`;

  const rows: string[] = [];
  rows.push(metaRow("来源", item.source_app ?? "—"));
  if (item.kind === "image") {
    rows.push(
      `<div class="clip-detail__metarow"><span class="clip-detail__metakey">类型</span><span class="clip-detail__metaval">图片 <span class="clip-detail__dims"></span></span></div>`,
    );
  } else {
    rows.push(metaRow("类型", clipTypeLabel(item)));
    rows.push(metaRow("字数", String(textCharCount(item) ?? 0)));
  }
  rows.push(metaRow("复制时间", new Date(item.created_at).toLocaleString()));

  return `<div class="clip-detail__preview">${preview}</div>
    <div class="clip-detail__meta">${rows.join("")}</div>`;
}

function renderDetail(): void {
  const item = items.find((i) => i.id === selectedId) ?? null;
  detailEl().innerHTML = detailHtml(item);
  const img = detailEl().querySelector(
    ".clip-detail__img",
  ) as HTMLImageElement | null;
  const dims = detailEl().querySelector(".clip-detail__dims") as HTMLElement | null;
  if (img && dims) {
    const fill = () => {
      if (img.naturalWidth) dims.textContent = `(${img.naturalWidth}×${img.naturalHeight})`;
    };
    img.complete ? fill() : img.addEventListener("load", fill);
  }
}

export async function renderClipboard(query = ""): Promise<void> {
  items = query ? await clipApi.search(query) : await clipApi.list();
  if (!items.some((i) => i.id === selectedId)) {
    selectedId = items[0]?.id ?? null;
  }
  const has = items.length > 0;
  bodyEl().style.display = has ? "flex" : "none";
  hintEl().style.display = has ? "none" : "block";
  listEl().innerHTML = groupedListHtml(items, Date.now());
  renderDetail();
}

/** Select a row by id: update highlight + detail pane. */
export function selectItem(id: number): void {
  selectedId = id;
  listEl()
    .querySelectorAll(".clip-row")
    .forEach((r) =>
      r.classList.toggle(
        "is-selected",
        Number((r as HTMLElement).dataset.id) === id,
      ),
    );
  renderDetail();
}

/** Move selection up (-1) or down (+1) the list. */
export function moveSelection(delta: number): void {
  if (!items.length) return;
  const idx = items.findIndex((i) => i.id === selectedId);
  const next = Math.min(items.length - 1, Math.max(0, idx + delta));
  const target = items[next];
  if (target) {
    selectItem(target.id);
    const row = listEl().querySelector(
      `.clip-row[data-id="${target.id}"]`,
    ) as HTMLElement | null;
    row?.scrollIntoView({ block: "nearest" });
  }
}

/** Wire row click (select / pin / delete) once. */
export function bindClipboardActions(): void {
  listEl().addEventListener("click", async (e) => {
    const row = (e.target as HTMLElement).closest(".clip-row") as HTMLElement | null;
    if (!row) return;
    const id = Number(row.dataset.id);
    const btn = (e.target as HTMLElement).closest("button[data-act]");
    if (!btn) {
      selectItem(id);
      return;
    }
    const act = (btn as HTMLElement).dataset.act;
    if (act === "del") await clipApi.remove(id);
    if (act === "pin") {
      const pinned = row.dataset.pinned === "true";
      await clipApi.setPinned(id, !pinned);
    }
    await renderClipboard();
  });
}
