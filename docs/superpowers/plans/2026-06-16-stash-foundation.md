# Stash 基础地基 实现计划（计划 ①）

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 搭起 Stash 的 Tauri 2 工程骨架——一个可被 `Alt+Space` 全局唤起/隐藏的无边框搜索窗，配套可测试的 SQLite 存储层、配置读写和双主题外壳。

**Architecture:** Tauri 2 应用，Rust 后端（core）+ Vanilla TypeScript 前端（Vite）。后端负责全局快捷键、窗口显隐、SQLite 持久化、配置文件读写；前端负责命令栏外壳与主题切换。后端能力以 `存储模块` / `配置模块` 形式独立成文件并用 `cargo test` 覆盖；前端纯逻辑用 `vitest` 覆盖。

**Tech Stack:** Tauri 2、Rust、rusqlite（bundled SQLite）、serde / serde_json、anyhow、tauri-plugin-global-shortcut、Vite、TypeScript、vitest + jsdom。

---

## 环境前提

- 已安装 Node.js ≥ 18、Rust（含 MSVC 工具链，`rustup default stable`）、npm。
- `rusqlite` 使用 `bundled` 特性，会从源码编译 SQLite，需要 MSVC C 编译器（随 Visual Studio Build Tools 安装）。
- 所有命令在仓库根目录 `C:\Users\guxio\Claude\Projects\CVTOOL` 下执行。

## 文件结构（本计划产出/修改）

| 文件 | 职责 |
|------|------|
| `package.json` / `vite.config.ts` / `tsconfig.json` / `index.html` | 前端工程（脚手架生成） |
| `src/main.ts` | 前端入口：启动时拉取配置并应用主题 |
| `src/theme.ts` | 主题解析与应用（纯逻辑，可测试） |
| `src/styles.css` | 双主题 CSS 变量 + 命令栏外壳样式 |
| `tests/theme.test.ts` | `theme.ts` 的 vitest 单测 |
| `src-tauri/Cargo.toml` | Rust 依赖 |
| `src-tauri/tauri.conf.json` | 窗口配置（无边框/隐藏/居中/置顶） |
| `src-tauri/capabilities/default.json` | 权限（含 global-shortcut） |
| `src-tauri/src/lib.rs` | 后端入口：插件、命令、setup（建目录、开库、注册快捷键） |
| `src-tauri/src/storage.rs` | SQLite 存储层：开库 + 迁移建表（含单测） |
| `src-tauri/src/config.rs` | 配置结构 + 默认值 + 读写（含单测） |

---

## Task 1: 脚手架 Tauri 2（vanilla-ts）

仓库根目录已有 `.git/`、`.gitignore`、`docs/`，所以先在临时子目录生成脚手架，再把内容搬到根目录，避免覆盖现有 `.gitignore`。

**Files:**
- Create: 由脚手架生成的全部工程文件

- [ ] **Step 1: 在临时目录生成脚手架**

Run:
```powershell
npm create tauri-app@latest .stash-scaffold -- --template vanilla-ts --manager npm --yes
```
Expected: 在 `.stash-scaffold/` 下生成 `package.json`、`index.html`、`src/`、`src-tauri/` 等文件，结尾打印 `Template created!`。

- [ ] **Step 2: 把脚手架内容搬到仓库根目录（保留我们已有的 .gitignore）**

Run:
```powershell
Get-ChildItem -Path .stash-scaffold -Force | Where-Object { $_.Name -ne '.gitignore' } | ForEach-Object { Move-Item -Path $_.FullName -Destination . -Force }
Add-Content -Path .gitignore -Value "`n# Tauri / frontend`n/.stash-scaffold`n"
Remove-Item -Recurse -Force .stash-scaffold
```
Expected: 仓库根目录出现 `package.json`、`src/`、`src-tauri/`；`.gitignore` 末尾追加了忽略项；`.stash-scaffold` 已删除。

- [ ] **Step 3: 安装依赖并启动验证**

Run:
```powershell
npm install
npm run tauri dev
```
Expected: 首次会编译 Rust（耗时数分钟），随后弹出一个默认 Tauri 窗口。确认窗口能正常显示后，在终端按 `Ctrl+C` 关闭。

- [ ] **Step 4: 提交**

```powershell
git add -A
git commit -m "chore: scaffold Tauri 2 vanilla-ts app"
```

---

## Task 2: 添加 Rust 依赖

**Files:**
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: 加入依赖**

在 `src-tauri/Cargo.toml` 的 `[dependencies]` 段加入（保留脚手架已有的 `tauri`、`serde`、`serde_json` 等，不要重复）：
```toml
rusqlite = { version = "0.31", features = ["bundled"] }
anyhow = "1"
tauri-plugin-global-shortcut = "2"
```
并确保 `serde` 带 derive 特性（脚手架通常已是 `serde = { version = "1", features = ["derive"] }`，若不是则改成这样）。

在文件末尾新增开发依赖：
```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: 验证可编译**

Run:
```powershell
cargo build --manifest-path src-tauri/Cargo.toml
```
Expected: 编译成功（首次会下载并编译 rusqlite/SQLite，耗时较长），结尾 `Finished`。

- [ ] **Step 3: 提交**

```powershell
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "build: add rusqlite, anyhow, global-shortcut deps"
```

---

## Task 3: 存储模块（SQLite 迁移建表）

**Files:**
- Create: `src-tauri/src/storage.rs`
- Modify: `src-tauri/src/lib.rs`（声明 `mod storage;`）

- [ ] **Step 1: 声明模块**

在 `src-tauri/src/lib.rs` 顶部（其他 `use` 之前）加一行：
```rust
mod storage;
```

- [ ] **Step 2: 写失败的测试**

新建 `src-tauri/src/storage.rs`，先只放测试和模块外壳：
```rust
use rusqlite::Connection;
use std::path::Path;

pub struct Store {
    pub conn: Connection,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn migrations_create_expected_tables() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("stash.db");
        let store = Store::open(&db).unwrap();
        let tables = store.table_names().unwrap();
        assert!(tables.contains(&"clipboard_items".to_string()));
        assert!(tables.contains(&"app_usage".to_string()));
        assert!(tables.contains(&"file_usage".to_string()));
    }

    #[test]
    fn open_creates_missing_parent_dirs() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("nested/sub/stash.db");
        assert!(Store::open(&db).is_ok());
        assert!(db.exists());
    }
}
```

- [ ] **Step 3: 运行测试，确认失败**

Run:
```powershell
cargo test --manifest-path src-tauri/Cargo.toml storage
```
Expected: 编译失败，提示 `no function or associated item named `open`` 与 `table_names`（方法未实现）。

- [ ] **Step 4: 实现 Store**

在 `src-tauri/src/storage.rs` 的 `pub struct Store` 之后、`#[cfg(test)]` 之前插入：
```rust
const MIGRATION_V1: &str = "
CREATE TABLE IF NOT EXISTS clipboard_items (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    kind        TEXT    NOT NULL,
    content     TEXT,
    image_path  TEXT,
    thumb_path  TEXT,
    source_app  TEXT,
    pinned      INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_clip_created ON clipboard_items(created_at DESC);
CREATE TABLE IF NOT EXISTS app_usage (
    path         TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    launch_count INTEGER NOT NULL DEFAULT 0,
    last_used    INTEGER
);
CREATE TABLE IF NOT EXISTS file_usage (
    path       TEXT PRIMARY KEY,
    open_count INTEGER NOT NULL DEFAULT 0,
    last_used  INTEGER
);
";

impl Store {
    pub fn open(db_path: &Path) -> anyhow::Result<Store> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        let store = Store { conn };
        store.run_migrations()?;
        Ok(store)
    }

    fn run_migrations(&self) -> anyhow::Result<()> {
        self.conn.execute_batch(MIGRATION_V1)?;
        Ok(())
    }

    pub fn table_names(&self) -> anyhow::Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
        let names = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(names)
    }
}
```

- [ ] **Step 5: 运行测试，确认通过**

Run:
```powershell
cargo test --manifest-path src-tauri/Cargo.toml storage
```
Expected: `test result: ok. 2 passed`。

- [ ] **Step 6: 提交**

```powershell
git add src-tauri/src/storage.rs src-tauri/src/lib.rs
git commit -m "feat(storage): SQLite store with schema migrations"
```

---

## Task 4: 配置模块（config.json 读写）

**Files:**
- Create: `src-tauri/src/config.rs`
- Modify: `src-tauri/src/lib.rs`（声明 `mod config;`）

- [ ] **Step 1: 声明模块**

在 `src-tauri/src/lib.rs` 顶部 `mod storage;` 下面加一行：
```rust
mod config;
```

- [ ] **Step 2: 写失败的测试**

新建 `src-tauri/src/config.rs`，先只放结构外壳与测试：
```rust
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Config {
    pub theme: String,
    pub hotkey_main: String,
    pub hotkey_paste: String,
    pub max_clipboard: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn defaults_when_file_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let cfg = Config::load(&path).unwrap();
        assert_eq!(cfg, Config::default());
        assert_eq!(cfg.theme, "warm");
        assert_eq!(cfg.max_clipboard, 200);
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("sub/config.json");
        let mut cfg = Config::default();
        cfg.theme = "light".to_string();
        cfg.max_clipboard = 150;
        cfg.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded, cfg);
    }
}
```

- [ ] **Step 3: 运行测试，确认失败**

Run:
```powershell
cargo test --manifest-path src-tauri/Cargo.toml config
```
Expected: 编译失败，提示 `Default`、`load`、`save` 未实现。

- [ ] **Step 4: 实现 Config**

在 `src-tauri/src/config.rs` 的结构体之后、`#[cfg(test)]` 之前插入：
```rust
impl Default for Config {
    fn default() -> Self {
        Config {
            theme: "warm".to_string(),
            hotkey_main: "Alt+Space".to_string(),
            hotkey_paste: "Alt+V".to_string(),
            max_clipboard: 200,
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> anyhow::Result<Config> {
        if !path.exists() {
            return Ok(Config::default());
        }
        let text = std::fs::read_to_string(path)?;
        let cfg: Config = serde_json::from_str(&text)?;
        Ok(cfg)
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(path, text)?;
        Ok(())
    }
}
```

- [ ] **Step 5: 运行测试，确认通过**

Run:
```powershell
cargo test --manifest-path src-tauri/Cargo.toml config
```
Expected: `test result: ok. 2 passed`。

- [ ] **Step 6: 提交**

```powershell
git add src-tauri/src/config.rs src-tauri/src/lib.rs
git commit -m "feat(config): config.json load/save with defaults"
```

---

## Task 5: 窗口配置（无边框/隐藏/居中/置顶）

`tauri.conf.json` 是声明式配置，难以单测，靠运行时目测验证。

**Files:**
- Modify: `src-tauri/tauri.conf.json`

- [ ] **Step 1: 改窗口与标识**

把 `src-tauri/tauri.conf.json` 里 `productName` 改为 `Stash`，`identifier` 改为 `com.stash.desktop`；把 `app.windows` 数组里那个窗口对象替换为：
```json
{
  "label": "main",
  "title": "Stash",
  "width": 720,
  "height": 460,
  "center": true,
  "decorations": false,
  "transparent": true,
  "alwaysOnTop": true,
  "skipTaskbar": true,
  "resizable": false,
  "visible": false
}
```

- [ ] **Step 2: 运行验证**

Run:
```powershell
npm run tauri dev
```
Expected: 应用启动后**不**自动弹窗（`visible:false`），任务栏也没有图标（`skipTaskbar:true`）。确认无报错后按 `Ctrl+C` 退出。（下一任务加上快捷键后才能唤起窗口。）

- [ ] **Step 3: 提交**

```powershell
git add src-tauri/tauri.conf.json
git commit -m "feat(window): frameless hidden centered always-on-top window"
```

---

## Task 6: 全局快捷键唤起/隐藏窗口

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/capabilities/default.json`

- [ ] **Step 1: 开放 global-shortcut 权限**

在 `src-tauri/capabilities/default.json` 的 `permissions` 数组里追加一项：
```json
"global-shortcut:default"
```

- [ ] **Step 2: 改写 lib.rs 入口**

把 `src-tauri/src/lib.rs` 中由脚手架生成的 `run()` 函数体替换为下面内容（保留顶部已有的 `mod storage;`、`mod config;`）。删除脚手架自带的示例 `greet` 命令（若有）。完整文件应为：
```rust
mod storage;
mod config;

use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use config::Config;
use storage::Store;

fn stash_dir(app: &tauri::AppHandle) -> anyhow::Result<PathBuf> {
    let base = app
        .path()
        .data_dir()
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok(base.join("Stash"))
}

fn toggle_main_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let visible = win.is_visible().unwrap_or(false);
        if visible {
            let _ = win.hide();
        } else {
            let _ = win.show();
            let _ = win.set_focus();
        }
    }
}

#[tauri::command]
fn get_config(app: tauri::AppHandle) -> Result<Config, String> {
    let dir = stash_dir(&app).map_err(|e| e.to_string())?;
    Config::load(&dir.join("config.json")).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        toggle_main_window(app);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![get_config])
        .setup(|app| {
            let dir = stash_dir(&app.handle())?;
            let store = Store::open(&dir.join("stash.db"))?;
            app.manage(Mutex::new(store));

            let main_hotkey = Shortcut::new(Some(Modifiers::ALT), Code::Space);
            app.global_shortcut().register(main_hotkey)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

> 注：若脚手架的 `lib.rs` 还 `.plugin(tauri_plugin_opener::init())`，可保留该行（放在 global-shortcut 插件之后）；若不需要可删除对应依赖。不要遗留对已删除 `greet` 命令的引用。

- [ ] **Step 3: 运行验证**

Run:
```powershell
npm run tauri dev
```
Expected: 启动后窗口隐藏；按 `Alt+Space` 窗口出现在屏幕中央并获得焦点；再按 `Alt+Space` 窗口隐藏。确认后 `Ctrl+C` 退出。

- [ ] **Step 4: 提交**

```powershell
git add src-tauri/src/lib.rs src-tauri/capabilities/default.json
git commit -m "feat(hotkey): Alt+Space toggles main window"
```

---

## Task 7: 前端主题模块（双主题，默认暖色）

**Files:**
- Create: `src/theme.ts`
- Create: `tests/theme.test.ts`
- Modify: `package.json`（加 vitest 脚本与依赖）
- Create: `vitest.config.ts`

- [ ] **Step 1: 安装测试依赖**

Run:
```powershell
npm add -D vitest jsdom
```
Expected: `package.json` 的 `devDependencies` 出现 `vitest` 与 `jsdom`。

- [ ] **Step 2: 加测试脚本与 vitest 配置**

在 `package.json` 的 `scripts` 里加一行：
```json
"test": "vitest run"
```
新建 `vitest.config.ts`：
```ts
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "jsdom",
  },
});
```

- [ ] **Step 3: 写失败的测试**

新建 `tests/theme.test.ts`：
```ts
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
```

- [ ] **Step 4: 运行测试，确认失败**

Run:
```powershell
npm test
```
Expected: 失败，提示无法从 `../src/theme` 解析 `resolveTheme` / `applyTheme`。

- [ ] **Step 5: 实现 theme.ts**

新建 `src/theme.ts`：
```ts
export type Theme = "light" | "warm";

export function resolveTheme(input: string | undefined): Theme {
  return input === "light" ? "light" : "warm";
}

export function applyTheme(
  theme: Theme,
  root: HTMLElement = document.documentElement,
): void {
  root.setAttribute("data-theme", theme);
}
```

- [ ] **Step 6: 运行测试，确认通过**

Run:
```powershell
npm test
```
Expected: `Test Files 1 passed`，3 个用例全过。

- [ ] **Step 7: 提交**

```powershell
git add src/theme.ts tests/theme.test.ts vitest.config.ts package.json package-lock.json
git commit -m "feat(theme): theme resolve/apply with vitest coverage"
```

---

## Task 8: 主题样式 + 启动时按配置应用主题

**Files:**
- Modify: `src/styles.css`
- Modify: `src/main.ts`

- [ ] **Step 1: 写双主题 CSS 变量与命令栏外壳**

把 `src/styles.css` 内容替换为：
```css
:root,
[data-theme="warm"] {
  --bg: rgba(38, 36, 44, 0.94);
  --fg: #efeaf0;
  --muted: rgba(239, 234, 240, 0.55);
  --row-sel: rgba(255, 123, 89, 0.16);
  --accent: linear-gradient(90deg, #ff7a59, #ff5b8a);
  --border: rgba(255, 255, 255, 0.06);
}

[data-theme="light"] {
  --bg: #ffffff;
  --fg: #1d1d1f;
  --muted: #6b6f76;
  --row-sel: #eef3ff;
  --accent: #2f6bff;
  --border: #f0f1f4;
}

* {
  box-sizing: border-box;
}

html,
body {
  margin: 0;
  background: transparent;
  font-family: "Segoe UI", system-ui, -apple-system, sans-serif;
}

.command-bar {
  border-radius: 14px;
  overflow: hidden;
  background: var(--bg);
  color: var(--fg);
  border: 1px solid var(--border);
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.45);
}

.command-bar__search {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 14px 16px;
  font-size: 15px;
  border-bottom: 1px solid var(--border);
}

.command-bar__search input {
  flex: 1;
  background: transparent;
  border: none;
  outline: none;
  color: var(--fg);
  font-size: 15px;
}

.command-bar__search input::placeholder {
  color: var(--muted);
}
```

- [ ] **Step 2: 写命令栏外壳的 HTML 入口**

把 `index.html` 的 `<body>` 内容替换为（保留原有的 `<script type="module" src="/src/main.ts"></script>`）：
```html
<div class="command-bar">
  <div class="command-bar__search">
    <span>🔍</span>
    <input id="search" placeholder="搜索应用、最近文件、剪贴板…" autofocus />
  </div>
</div>
<script type="module" src="/src/main.ts"></script>
```

- [ ] **Step 3: 启动时拉取配置并应用主题**

把 `src/main.ts` 内容替换为：
```ts
import "./styles.css";
import { invoke } from "@tauri-apps/api/core";
import { applyTheme, resolveTheme } from "./theme";

interface AppConfig {
  theme: string;
  hotkey_main: string;
  hotkey_paste: string;
  max_clipboard: number;
}

async function init(): Promise<void> {
  try {
    const cfg = await invoke<AppConfig>("get_config");
    applyTheme(resolveTheme(cfg.theme));
  } catch {
    applyTheme("warm");
  }
}

void init();
```

- [ ] **Step 4: 运行验证**

Run:
```powershell
npm run tauri dev
```
Expected: 按 `Alt+Space` 唤起窗口，呈现**暖色**命令栏（深暖灰背景、圆角、占位文字「搜索应用、最近文件、剪贴板…」），输入框可聚焦输入。确认后 `Ctrl+C` 退出。

- [ ] **Step 5: 手动验证浅色主题（可选回归）**

临时把 `%APPDATA%\Stash\config.json` 创建为：
```json
{ "theme": "light", "hotkey_main": "Alt+Space", "hotkey_paste": "Alt+V", "max_clipboard": 200 }
```
再 `npm run tauri dev`，确认命令栏变为白底浅色。验证完可删除该文件恢复默认暖色。

- [ ] **Step 6: 提交**

```powershell
git add src/styles.css src/main.ts index.html
git commit -m "feat(ui): dual-theme command bar shell, apply theme from config"
```

---

## 完成标准（本计划）

- `cargo test --manifest-path src-tauri/Cargo.toml` 全绿（storage + config 共 4 个用例）。
- `npm test` 全绿（theme 3 个用例）。
- `npm run tauri dev`：应用静默启动（无可见窗口、无任务栏图标）；`Alt+Space` 唤起/隐藏居中无边框命令栏；命令栏默认暖色、可切换浅色；数据目录 `%APPDATA%\Stash\` 下生成 `stash.db`。

## 交接给后续计划

地基跑通后，下列能力留给计划 ②③④，已在本地基中预置好支撑：
- `clipboard_items` / `app_usage` / `file_usage` 表已建好 → 计划 ② / ③ 直接写仓储与查询。
- `Mutex<Store>` 已注册为 Tauri state → 后续命令用 `tauri::State` 取用。
- `Config` 已含 `hotkey_paste` / `max_clipboard` 字段 → 计划 ②（队列粘贴、200 条上限）与计划 ④（设置页）直接消费。
- `applyTheme` / `resolveTheme` 已就绪 → 计划 ④ 设置页切换主题复用。
