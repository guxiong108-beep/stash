# Stash 剪贴板监听 + 历史 UI 实现计划（计划 ②b）

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 Stash 真正记录系统剪贴板（文本 + 图片）并在命令栏里可见——后台监听写入数据层、Tauri 命令暴露查询/管理、前端「剪贴板」标签展示历史（缩略图、搜索、置顶、删除、清空），复制后实时刷新。

**Architecture:** 三层。(1) Rust 命令层 `commands.rs`：薄封装已测的 `clipboard` 数据层，经 `State<Mutex<Store>>` 暴露给前端。(2) Rust 监听层 `clipmon.rs`：用 `clipboard-rs` 的 watcher 在后台线程监听变化，读文本/图片→落库（图片走 `save_image_bytes`+`insert_image`，文本走 `insert_text`）→`enforce_cap(max_clipboard)`→`emit("clip://changed")`。(3) 前端：剪贴板视图渲染 + 模式标签 +监听刷新事件。

**Tech Stack:** Rust、clipboard-rs（剪贴板监听/读取）、已有 rusqlite/image/sha2 数据层、Tauri 2 command/event、Vanilla TS + vitest。

---

## 环境前提（子代理必读）

- Windows。命令用 PowerShell 工具。`cargo` 不在新 shell 默认 PATH,每条 cargo 命令前注入:
  ```
  $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH";
  ```
- Rust 测试用 `--lib`(避免 stale-binary 文件锁):`cargo test --manifest-path src-tauri/Cargo.toml --lib`。
- `npm` 正常可用。前端纯逻辑测试 `npm test`(vitest)。
- 涉及 GUI 行为(窗口、复制实时刷新、缩略图显示)的验证标注为**【手动验证·留用户】**——子代理不必也无法在此环境点按,只需保证编译/`cargo check`/`npm run build` 通过,并写清楚让用户怎么验。
- 仓库根 `C:\Users\guxio\Claude\Projects\CVTOOL`；执行时另建分支,勿自行 push。git 身份报错用 `git -c user.name="guxiong" -c user.email="guxiong108@gmail.com" commit ...`。

## 已有上下文

- `clipboard` 数据层(`src-tauri/src/clipboard.rs`)已就绪并全测:`ClipItem`(serde)、`insert_text(conn,&str,Option<&str>)->i64`、`insert_image(conn,image_path,thumb_path,hash,source_app)->i64`、`save_image_bytes(&Path,&Path,&[u8])->(image,thumb,hash)`、`list_recent(conn,limit)->Vec<ClipItem>`、`search(conn,&str,limit)`、`set_pinned(conn,id,bool)`、`delete(conn,id)`、`clear(conn)`、`enforce_cap(conn,max)`。
- `storage::Store{pub conn}`,在 `lib.rs` setup 里以 `app.manage(Mutex<Store>)` 注册。
- `lib.rs` 有 `fn stash_dir(app:&AppHandle)->anyhow::Result<PathBuf>`(=`%APPDATA%\Stash`),`get_config` 命令,`mod storage/config/clipboard`。
- `config::Config{theme,hotkey_main,hotkey_paste,max_clipboard}`,默认 max_clipboard=200。
- 前端:`index.html` 命令栏外壳(`.command-bar`>`.command-bar__search`+`.command-bar__body`>`.command-bar__hint`)、`src/main.ts`(启动 applyTheme)、`src/theme.ts`、`src/styles.css`(双主题变量含 `--row-sel`)。

## 文件结构

| 文件 | 职责 |
|------|------|
| `src-tauri/Cargo.toml` | 加 `clipboard-rs` |
| `src-tauri/src/commands.rs`(新) | Tauri 命令:clip_list/clip_search/clip_set_pinned/clip_delete/clip_clear |
| `src-tauri/src/paths.rs`(新) | 共享路径助手 stash_dir/images_dir/thumbs_dir/db_path |
| `src-tauri/src/clipmon.rs`(新) | 剪贴板监听:watcher 线程 + 落库 + enforce_cap + emit |
| `src-tauri/src/lib.rs` | 声明新模块、注册命令、setup 里启动监听 |
| `src/clip.ts`(新) | 前端:ClipItem 类型、命令封装、`formatClipPreview` 纯函数 |
| `tests/clip.test.ts`(新) | `formatClipPreview` 的 vitest |
| `src/clipboard_view.ts`(新) | 渲染剪贴板列表 + 绑定操作 |
| `src/main.ts` | 接线:标签、打开时加载、搜索、事件刷新 |
| `index.html` / `src/styles.css` | 模式标签栏 + 列表样式 |

---

## Task 1: 添加 clipboard-rs 依赖

**Files:** Modify `src-tauri/Cargo.toml`

- [ ] **Step 1: 加依赖** —— 在 `[dependencies]` 追加：
```toml
clipboard-rs = "0.3"
```
- [ ] **Step 2: 验证编译**
Run: `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo build --manifest-path src-tauri/Cargo.toml`
Expected: 成功(首次编译 clipboard-rs 较慢),`Finished`。若 `0.3` 解析失败,运行 `cargo add clipboard-rs --manifest-path src-tauri/Cargo.toml` 取当前版本并记录实际版本号。
- [ ] **Step 3: 提交**
```
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "build: add clipboard-rs for clipboard monitoring"
```

---

## Task 2: 共享路径助手 paths.rs

把 `lib.rs` 里私有的 `stash_dir` 提成共享模块,供监听层复用,并加 images/thumbs/db 助手。

**Files:** Create `src-tauri/src/paths.rs`; Modify `src-tauri/src/lib.rs`

- [ ] **Step 1: 写失败的测试** —— 新建 `src-tauri/src/paths.rs`：
```rust
use std::path::{Path, PathBuf};

/// `%APPDATA%\Stash` 下的子路径助手（纯路径拼接，不碰文件系统）。
pub fn images_dir(base: &Path) -> PathBuf {
    base.join("images")
}
pub fn thumbs_dir(base: &Path) -> PathBuf {
    base.join("thumbs")
}
pub fn db_path(base: &Path) -> PathBuf {
    base.join("stash.db")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn subpaths_are_under_base() {
        let base = Path::new("C:/tmp/Stash");
        assert!(images_dir(base).ends_with("images"));
        assert!(thumbs_dir(base).ends_with("thumbs"));
        assert!(db_path(base).ends_with("stash.db"));
    }
}
```
- [ ] **Step 2: 声明模块并改用助手** —— 在 `src-tauri/src/lib.rs` 顶部 `mod clipboard;` 下加 `mod paths;`。把 setup 里 `Store::open(&dir.join("stash.db"))` 改为 `Store::open(&paths::db_path(&dir))`(`dir` 即 `stash_dir` 结果)。保留 `stash_dir` 函数不变。
- [ ] **Step 3: 测试**
Run: `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test --manifest-path src-tauri/Cargo.toml --lib paths`
Expected: 1 passed。再 `cargo build --manifest-path src-tauri/Cargo.toml` 确认 lib.rs 改动编译通过。
- [ ] **Step 4: 提交**
```
git add src-tauri/src/paths.rs src-tauri/src/lib.rs
git commit -m "feat(paths): shared dir helpers for Stash data files"
```

---

## Task 3: Tauri 命令层 commands.rs

**Files:** Create `src-tauri/src/commands.rs`; Modify `src-tauri/src/lib.rs`

- [ ] **Step 1: 写命令模块** —— 新建 `src-tauri/src/commands.rs`：
```rust
use std::sync::Mutex;
use tauri::State;

use crate::clipboard::{self, ClipItem};
use crate::storage::Store;

fn with_conn<T>(
    state: &State<'_, Mutex<Store>>,
    f: impl FnOnce(&rusqlite::Connection) -> rusqlite::Result<T>,
) -> Result<T, String> {
    let store = state.lock().map_err(|e| e.to_string())?;
    f(&store.conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clip_list(state: State<'_, Mutex<Store>>, limit: i64) -> Result<Vec<ClipItem>, String> {
    with_conn(&state, |c| clipboard::list_recent(c, limit))
}

#[tauri::command]
pub fn clip_search(
    state: State<'_, Mutex<Store>>,
    query: String,
    limit: i64,
) -> Result<Vec<ClipItem>, String> {
    with_conn(&state, |c| clipboard::search(c, &query, limit))
}

#[tauri::command]
pub fn clip_set_pinned(
    state: State<'_, Mutex<Store>>,
    id: i64,
    pinned: bool,
) -> Result<(), String> {
    with_conn(&state, |c| clipboard::set_pinned(c, id, pinned))
}

#[tauri::command]
pub fn clip_delete(state: State<'_, Mutex<Store>>, id: i64) -> Result<(), String> {
    with_conn(&state, |c| clipboard::delete(c, id))
}

#[tauri::command]
pub fn clip_clear(state: State<'_, Mutex<Store>>) -> Result<(), String> {
    with_conn(&state, |c| clipboard::clear(c))
}
```
- [ ] **Step 2: 注册命令** —— 在 `src-tauri/src/lib.rs`：顶部加 `mod commands;`；把 `invoke_handler` 改为：
```rust
.invoke_handler(tauri::generate_handler![
    get_config,
    commands::clip_list,
    commands::clip_search,
    commands::clip_set_pinned,
    commands::clip_delete,
    commands::clip_clear
])
```
- [ ] **Step 3: 编译验证**
Run: `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo build --manifest-path src-tauri/Cargo.toml`
Expected: 编译成功。（命令依赖 Tauri runtime,不做单测,靠编译 + 后续前端联调验证。）
- [ ] **Step 4: 提交**
```
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(commands): expose clipboard list/search/pin/delete/clear"
```

---

## Task 4: 剪贴板监听 clipmon.rs

**【部分手动验证】** 监听依赖 `clipboard-rs` 的真实 API 与系统剪贴板。下面是**目标实现**;实现子代理需对照 `clipboard-rs` 0.3.x 实际 API 校准方法名/签名(尤其 `RustImage` 取 PNG 字节的方法,可能是 `to_png()?.get_bytes()` 或类似),以**编译通过**为准,不得保留无法编译的猜测代码。

**Files:** Create `src-tauri/src/clipmon.rs`; Modify `src-tauri/src/lib.rs`

- [ ] **Step 1: 写监听模块（目标实现）** —— 新建 `src-tauri/src/clipmon.rs`：
```rust
use std::sync::Mutex;

use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher,
    ClipboardWatcherContext, ContentFormat, RustImage,
};
use tauri::{AppHandle, Emitter, Manager};

use crate::clipboard;
use crate::config::Config;
use crate::paths;
use crate::storage::Store;

/// Event name emitted to the frontend whenever a new clipboard item is recorded.
pub const CLIP_CHANGED_EVENT: &str = "clip://changed";

struct Monitor {
    app: AppHandle,
    ctx: ClipboardContext,
}

impl Monitor {
    /// Read the current clipboard and persist it. Returns true if something was stored.
    fn capture(&self) -> anyhow::Result<bool> {
        // Resolve per-run paths and config fresh each time (cheap, avoids stale state).
        let base = self
            .app
            .path()
            .data_dir()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?
            .join("Stash");
        let cfg = Config::load(&base.join("config.json")).unwrap_or_default();

        let state = self.app.state::<Mutex<Store>>();
        let store = state.lock().map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Prefer text; fall back to image. (Most copies are text.)
        if self.ctx.has(ContentFormat::Text) {
            if let Ok(text) = self.ctx.get_text() {
                if !text.is_empty() {
                    clipboard::insert_text(&store.conn, &text, None)?;
                    clipboard::enforce_cap(&store.conn, cfg.max_clipboard as i64)?;
                    return Ok(true);
                }
            }
        }
        if self.ctx.has(ContentFormat::Image) {
            if let Ok(img) = self.ctx.get_image() {
                // Adjust to the real RustImage API: get PNG-encoded bytes.
                let png = img.to_png()?;
                let bytes = png.get_bytes();
                let (image_path, thumb_path, hash) = clipboard::save_image_bytes(
                    &paths::images_dir(&base),
                    &paths::thumbs_dir(&base),
                    bytes,
                )?;
                clipboard::insert_image(&store.conn, &image_path, &thumb_path, &hash, None)?;
                clipboard::enforce_cap(&store.conn, cfg.max_clipboard as i64)?;
                return Ok(true);
            }
        }
        Ok(false)
    }
}

impl ClipboardHandler for Monitor {
    fn on_clipboard_change(&mut self) {
        match self.capture() {
            Ok(true) => {
                let _ = self.app.emit(CLIP_CHANGED_EVENT, ());
            }
            Ok(false) => {}
            Err(e) => eprintln!("[stash] clipboard capture failed: {e}"),
        }
    }
}

/// Spawn the clipboard watcher on a background thread. Non-fatal on failure.
pub fn start(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let ctx = match ClipboardContext::new() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[stash] clipboard context init failed: {e}");
                return;
            }
        };
        let mut watcher = match ClipboardWatcherContext::new() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("[stash] clipboard watcher init failed: {e}");
                return;
            }
        };
        let handler = Monitor { app, ctx };
        watcher.add_handler(handler);
        watcher.start_watch(); // blocks this thread
    });
}
```
> 实现子代理注意:`ClipboardContext::new()` / `ClipboardWatcherContext::new()` 的返回是否 `Result`、`add_handler` 是否返回 `&mut self`、`RustImage` 取字节的确切方法(`to_png()`→buffer→`get_bytes()`),都以 `clipboard-rs` 0.3.x 文档/源码为准微调,保证 `cargo build` 通过。`Config::load` 路径用 `base.join("config.json")`(上面写法等价,若别扭就直接 `base.join("config.json")`)。

- [ ] **Step 2: 声明并启动监听** —— 在 `src-tauri/src/lib.rs` 顶部加 `mod clipmon;`；在 setup 闭包内、注册热键之后加：
```rust
            clipmon::start(&app.handle().clone());
```
- [ ] **Step 3: 编译验证**
Run: `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo build --manifest-path src-tauri/Cargo.toml`
Expected: 编译成功。若 clipboard-rs API 名称不符,据实修正至通过。
- [ ] **Step 4:【手动验证·留用户】** 文档化验证步骤(写进提交说明即可,不在此环境执行):`npm run tauri dev` → 复制一段文字 → `%APPDATA%\Stash\stash.db` 的 `clipboard_items` 新增一行(可用 `python` 查:`SELECT count(*),max(content) FROM clipboard_items`)。
- [ ] **Step 5: 提交**
```
git add src-tauri/src/clipmon.rs src-tauri/src/lib.rs
git commit -m "feat(clipmon): watch system clipboard, persist text/image, emit event"
```

---

## Task 5: 前端 clip API + 预览格式化（TDD）

**Files:** Create `src/clip.ts`, `tests/clip.test.ts`

- [ ] **Step 1: 写失败的测试** —— 新建 `tests/clip.test.ts`：
```ts
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
```
- [ ] **Step 2: 运行,确认失败**
Run: `npm test`
Expected: 无法解析 `../src/clip`。
- [ ] **Step 3: 实现 src/clip.ts**
```ts
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

export const clipApi = {
  list: (limit = 200) => invoke<ClipItem[]>("clip_list", { limit }),
  search: (query: string, limit = 200) =>
    invoke<ClipItem[]>("clip_search", { query, limit }),
  setPinned: (id: number, pinned: boolean) =>
    invoke<void>("clip_set_pinned", { id, pinned }),
  remove: (id: number) => invoke<void>("clip_delete", { id }),
  clear: () => invoke<void>("clip_clear"),
};
```
- [ ] **Step 4: 运行,确认通过**
Run: `npm test`
Expected: 全部通过(theme 3 + clip 3)。
- [ ] **Step 5: 提交**
```
git add src/clip.ts tests/clip.test.ts
git commit -m "feat(ui): clip API wrapper and preview formatter with tests"
```

---

## Task 6: 模式标签栏 + 剪贴板列表渲染

**【手动验证】** 渲染与 Tauri 调用需真实运行;子代理保证 `npm run build` 通过并写清验证步骤。

**Files:** Modify `src-tauri/tauri.conf.json`, `index.html`, `src/styles.css`; Create `src/clipboard_view.ts`; Modify `src/main.ts`

- [ ] **Step 0: 开启资源协议(缩略图才能加载)** —— `convertFileSrc` 把本地缩略图路径转成 `asset:` URL,需在 `src-tauri/tauri.conf.json` 的 `app.security` 里开启并限定范围到 Stash 数据目录。把 `app.security` 改为:
```json
"security": {
  "csp": null,
  "assetProtocol": {
    "enable": true,
    "scope": ["$APPDATA/Stash/**"]
  }
}
```
> 若运行时缩略图仍 403/不显示,在 `src-tauri/capabilities/default.json` 的 `permissions` 加 `"core:asset:default"`。`csp` 保持 `null`,否则需额外允许 `img-src asset:`。

- [ ] **Step 1: 标签栏 + 列表容器 HTML** —— 在 `index.html` 的 `.command-bar__search` 之后、`.command-bar__body` 之前插入标签栏,并把 body 改为放列表:
```html
      <div class="command-bar__tabs" id="tabs">
        <button class="tab is-active" data-mode="all">全部</button>
        <button class="tab" data-mode="apps">应用</button>
        <button class="tab" data-mode="files">最近文件</button>
        <button class="tab" data-mode="clipboard">剪贴板</button>
      </div>
      <div class="command-bar__body">
        <ul class="clip-list" id="clip-list"></ul>
        <p class="command-bar__hint" id="hint">输入关键词，开始搜索应用、最近文件与剪贴板历史</p>
      </div>
```
- [ ] **Step 2: 列表/标签样式** —— 在 `src/styles.css` 末尾追加：
```css
.command-bar__tabs {
  display: flex;
  gap: 6px;
  padding: 8px 14px;
  border-bottom: 1px solid var(--border);
}
.tab {
  padding: 3px 10px;
  border-radius: 7px;
  border: none;
  background: transparent;
  color: var(--muted);
  font-size: 12px;
  cursor: pointer;
}
.tab.is-active {
  background: var(--row-sel);
  color: var(--fg);
}
.clip-list {
  list-style: none;
  margin: 0;
  padding: 6px;
  overflow-y: auto;
  max-height: 360px;
}
.clip-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border-radius: 8px;
  font-size: 13.5px;
  cursor: default;
}
.clip-row:hover {
  background: var(--row-sel);
}
.clip-row__thumb {
  width: 28px;
  height: 28px;
  object-fit: cover;
  border-radius: 5px;
  flex: none;
}
.clip-row__text {
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.clip-row__actions {
  display: flex;
  gap: 6px;
  opacity: 0.6;
}
.clip-row__actions button {
  border: none;
  background: transparent;
  color: var(--fg);
  cursor: pointer;
  font-size: 13px;
}
```
（`styles.css` 是纯 CSS 文件——只追加上面的规则，不要写任何 `<style>` 标签。）

- [ ] **Step 3: 渲染模块 src/clipboard_view.ts**
```ts
import { clipApi, formatClipPreview, type ClipItem } from "./clip";
import { convertFileSrc } from "@tauri-apps/api/core";

const listEl = () => document.getElementById("clip-list") as HTMLUListElement;
const hintEl = () => document.getElementById("hint") as HTMLElement;

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

function escapeHtml(s: string): string {
  const d = document.createElement("div");
  d.textContent = s;
  return d.innerHTML;
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
```
- [ ] **Step 4: 接线 main.ts** —— 把 `src/main.ts` 改为(保留主题初始化,新增标签/搜索/事件刷新):
```ts
import "./styles.css";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { applyTheme, resolveTheme } from "./theme";
import { renderClipboard, bindClipboardActions } from "./clipboard_view";

interface AppConfig {
  theme: string;
  hotkey_main: string;
  hotkey_paste: string;
  max_clipboard: number;
}

let mode = "all";

function setupTabs(): void {
  const tabs = document.getElementById("tabs")!;
  tabs.addEventListener("click", (e) => {
    const btn = (e.target as HTMLElement).closest(".tab") as HTMLElement | null;
    if (!btn) return;
    tabs.querySelectorAll(".tab").forEach((t) => t.classList.remove("is-active"));
    btn.classList.add("is-active");
    mode = btn.dataset.mode ?? "all";
    refresh();
  });
}

function refresh(): void {
  const q = (document.getElementById("search") as HTMLInputElement).value;
  if (mode === "clipboard" || mode === "all") {
    void renderClipboard(q);
  } else {
    // apps / files come in plan ③; show the hint for now.
    (document.getElementById("clip-list") as HTMLElement).innerHTML = "";
    (document.getElementById("hint") as HTMLElement).style.display = "block";
  }
}

async function init(): Promise<void> {
  try {
    const cfg = await invoke<AppConfig>("get_config");
    applyTheme(resolveTheme(cfg.theme));
  } catch {
    applyTheme("warm");
  }
  setupTabs();
  bindClipboardActions();
  (document.getElementById("search") as HTMLInputElement).addEventListener("input", refresh);
  await listen("clip://changed", () => refresh());
  refresh();
}

void init();
```
- [ ] **Step 5: 构建验证**
Run: `npm run build`
Expected: tsc + vite 构建成功,无类型错误。再 `npm test` 确认 clip/theme 测试仍过。
- [ ] **Step 6:【手动验证·留用户】** 文档化:`npm run tauri dev` → Alt+Space → 点「剪贴板」标签 → 之前复制的文字/图片以列表显示;复制新内容列表实时刷新;🗑 删除、📌 置顶生效;搜索框输入可过滤。
- [ ] **Step 7: 提交**
```
git add index.html src/styles.css src/clipboard_view.ts src/main.ts
git commit -m "feat(ui): clipboard tab with history list, search, pin/delete, live refresh"
```

---

## 完成标准（本计划）

- `cargo build` 通过(含监听 + 命令)；`cargo test --lib` 仍全绿(新增 paths 1)；`npm test` 全绿(clip 3 + theme 3)；`npm run build` 干净。
- 监听线程在 setup 启动,复制文本/图片即写库并 emit `clip://changed`。
- 命令栏「剪贴板」标签展示历史(文本预览/图片缩略图),支持搜索、置顶、删除、实时刷新。
- 【留用户验收】真机 `npm run tauri dev` 走一遍上述手动验证清单。

## 交接给计划 ②c（队列粘贴）

- `ClipItem` 与 `clipApi` 已就绪,②c 在列表上加多选 + 建队列;`Alt+V` 注入下一条 + 浮窗;复用 `hotkey_paste` 配置。
- 监听已稳定记录,②c 不需再碰数据层。

## 已知延后项（不在本计划）

- **敏感内容跳过**(密码管理器的 `ExcludeClipboardContentFromMonitorProcessing`/`CanIncludeInClipboardHistory` Windows 剪贴板格式):clipboard-rs 未直接暴露,留到计划 ④ 用 Windows 原生格式检测实现;当前先全量记录。
- 「应用 / 最近文件」标签的实际功能 → 计划 ③。
