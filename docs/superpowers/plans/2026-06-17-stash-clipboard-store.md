# Stash 剪贴板数据层 实现计划（计划 ②a）

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在已有的 SQLite 存储之上，建立剪贴板条目的数据层——插入（文本/图片）、去重、按日期列出、200 条上限淘汰（置顶豁免）、置顶/删除/清空/搜索，以及图片落盘 + 缩略图，全程 TDD。

**Architecture:** 新增 `src-tauri/src/clipboard.rs` 模块，提供操作 `rusqlite::Connection` 的函数与 `ClipItem` 模型；不引入新的连接管理，复用地基的 `storage::Store`。新增数据库迁移 V2（给 `clipboard_items` 加 `hash` 列用于图片去重），并把 `run_migrations` 重构为按 `user_version` 增量应用。图片用 `image` crate 解码与生成缩略图，`sha2` 算内容哈希。

**Tech Stack:** Rust、rusqlite、image crate、sha2、anyhow；测试用 tempfile。

---

## 环境前提（子代理必读）

- Windows。命令用 PowerShell 工具。`cargo` 已装但**不在新 shell 的默认 PATH**，每条 cargo 命令前都要注入：
  ```
  $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test --manifest-path src-tauri/Cargo.toml clipboard
  ```
- 不要运行 `npm run tauri dev`（GUI 无法在此验证）。本计划纯 Rust，只用 `cargo test` / `cargo build`。
- 仓库根：`C:\Users\guxio\Claude\Projects\CVTOOL`。当前分支会在执行时另建（不要自行 push）。
- git 身份若报错，用 `git -c user.name="guxiong" -c user.email="guxiong108@gmail.com" commit ...`。

## 已有上下文（地基已完成）

- `src-tauri/src/storage.rs`：`Store { pub conn: Connection }`，`Store::open(&Path)` 建目录+开库+WAL+busy_timeout+迁移。`clipboard_items` 表列：`id, kind, content, image_path, thumb_path, source_app, pinned, created_at`（无 `hash` 列）。`MIGRATION_V1` 末尾 `PRAGMA user_version = 1;`。`run_migrations` 当前直接 `execute_batch(MIGRATION_V1)`。
- `src-tauri/src/lib.rs` 顶部已有 `mod storage; mod config;`。

## 文件结构

| 文件 | 职责 |
|------|------|
| `src-tauri/Cargo.toml` | 加 `image`、`sha2` 依赖 |
| `src-tauri/src/storage.rs` | 重构 `run_migrations` 为按版本增量；加 `MIGRATION_V2`（hash 列） |
| `src-tauri/src/clipboard.rs` | 新建：`ClipItem` 模型 + 所有剪贴板数据操作 + 图片落盘 |
| `src-tauri/src/lib.rs` | 加 `mod clipboard;` |

---

## Task 1: 添加 image / sha2 依赖

**Files:**
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: 加依赖**

在 `[dependencies]` 段追加：
```toml
image = "0.25"
sha2 = "0.10"
```

- [ ] **Step 2: 验证可编译**

Run: `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo build --manifest-path src-tauri/Cargo.toml`
Expected: 编译成功（首次会编译 image/sha2，耗时稍长），结尾 `Finished`。

- [ ] **Step 3: 提交**
```
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "build: add image and sha2 deps for clipboard store"
```

---

## Task 2: 按版本增量迁移 + hash 列（迁移 V2）

**Files:**
- Modify: `src-tauri/src/storage.rs`

- [ ] **Step 1: 写失败的测试**

在 `src-tauri/src/storage.rs` 的 `#[cfg(test)] mod tests` 内追加两个测试：
```rust
    #[test]
    fn fresh_db_is_at_version_2_with_hash_column() {
        let dir = tempdir().unwrap();
        let store = Store::open(&dir.path().join("stash.db")).unwrap();
        let version: i64 = store
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, 2);
        let cols: Vec<String> = store
            .conn
            .prepare("PRAGMA table_info(clipboard_items)")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(cols.contains(&"hash".to_string()));
    }

    #[test]
    fn migrations_are_idempotent_on_reopen() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("stash.db");
        Store::open(&db).unwrap();
        // Reopening must not error (migrations already applied).
        let store = Store::open(&db).unwrap();
        let version: i64 = store
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, 2);
    }
```

- [ ] **Step 2: 运行测试，确认失败**

Run: `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test --manifest-path src-tauri/Cargo.toml storage`
Expected: `fresh_db_is_at_version_2_with_hash_column` 失败——`assert_eq!(version, 2)` 实际为 1，且无 `hash` 列。

- [ ] **Step 3: 加 MIGRATION_V2 并重构 run_migrations**

在 `storage.rs` 中 `const MIGRATION_V1` 之后新增：
```rust
const MIGRATION_V2: &str = "
ALTER TABLE clipboard_items ADD COLUMN hash TEXT;
PRAGMA user_version = 2;
";
```

把现有的 `run_migrations` 方法替换为按版本增量应用：
```rust
    fn run_migrations(&self) -> anyhow::Result<()> {
        let version: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version < 1 {
            self.conn.execute_batch(MIGRATION_V1)?;
        }
        if version < 2 {
            self.conn.execute_batch(MIGRATION_V2)?;
        }
        Ok(())
    }
```

- [ ] **Step 4: 运行测试，确认通过**

Run: `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test --manifest-path src-tauri/Cargo.toml storage`
Expected: storage 下所有测试通过（含原有 2 个 + 新增 2 个）。

- [ ] **Step 5: 提交**
```
git add src-tauri/src/storage.rs
git commit -m "feat(storage): version-gated migrations, add hash column (v2)"
```

---

## Task 3: clipboard 模块 + 插入文本（去重）+ 按日期列出

**Files:**
- Create: `src-tauri/src/clipboard.rs`
- Modify: `src-tauri/src/lib.rs`（加 `mod clipboard;`）

- [ ] **Step 1: 声明模块**

在 `src-tauri/src/lib.rs` 的 `mod config;` 下加一行：
```rust
mod clipboard;
```

- [ ] **Step 2: 写模块骨架与失败的测试**

新建 `src-tauri/src/clipboard.rs`：
```rust
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ClipItem {
    pub id: i64,
    pub kind: String, // "text" | "image"
    pub text: Option<String>,
    pub image_path: Option<String>,
    pub thumb_path: Option<String>,
    pub source_app: Option<String>,
    pub pinned: bool,
    pub created_at: i64, // unix millis
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Store;
    use tempfile::tempdir;

    fn open() -> (tempfile::TempDir, Store) {
        let dir = tempdir().unwrap();
        let store = Store::open(&dir.path().join("stash.db")).unwrap();
        (dir, store)
    }

    #[test]
    fn insert_text_then_list_returns_it() {
        let (_d, store) = open();
        let id = insert_text(&store.conn, "hello", Some("notepad")).unwrap();
        assert!(id > 0);
        let items = list_recent(&store.conn, 50).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, "text");
        assert_eq!(items[0].text.as_deref(), Some("hello"));
        assert_eq!(items[0].source_app.as_deref(), Some("notepad"));
        assert!(!items[0].pinned);
    }

    #[test]
    fn duplicate_text_is_deduped_and_bumped_to_top() {
        let (_d, store) = open();
        let id1 = insert_text(&store.conn, "aaa", None).unwrap();
        insert_text(&store.conn, "bbb", None).unwrap();
        let id_again = insert_text(&store.conn, "aaa", None).unwrap();
        assert_eq!(id1, id_again, "same text must reuse the same row");
        let items = list_recent(&store.conn, 50).unwrap();
        assert_eq!(items.len(), 2, "no duplicate row created");
        assert_eq!(items[0].text.as_deref(), Some("aaa"), "re-inserted item is newest");
    }

    #[test]
    fn list_orders_pinned_first_then_recent() {
        let (_d, store) = open();
        let old = insert_text(&store.conn, "old", None).unwrap();
        insert_text(&store.conn, "new", None).unwrap();
        set_pinned(&store.conn, old, true).unwrap();
        let items = list_recent(&store.conn, 50).unwrap();
        assert_eq!(items[0].text.as_deref(), Some("old"), "pinned floats to top");
    }
}
```
（注：本 Task 的 Step 4 会同时实现 `insert_text`、`list_recent` 和 `set_pinned`，所以三个测试在本 Task 内全部转绿——`set_pinned` 不留到后面。）

- [ ] **Step 3: 运行测试，确认失败**

Run: `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test --manifest-path src-tauri/Cargo.toml clipboard`
Expected: 编译失败——`insert_text` / `list_recent` / `set_pinned` 未实现。

- [ ] **Step 4: 实现 insert_text / list_recent（及占位 set_pinned）**

在 `clipboard.rs` 的 `ClipItem` 之后、`#[cfg(test)]` 之前插入：
```rust
fn now_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn row_to_item(row: &rusqlite::Row) -> rusqlite::Result<ClipItem> {
    Ok(ClipItem {
        id: row.get(0)?,
        kind: row.get(1)?,
        text: row.get(2)?,
        image_path: row.get(3)?,
        thumb_path: row.get(4)?,
        source_app: row.get(5)?,
        pinned: row.get::<_, i64>(6)? != 0,
        created_at: row.get(7)?,
    })
}

const SELECT_COLS: &str =
    "id, kind, content, image_path, thumb_path, source_app, pinned, created_at";

pub fn insert_text(
    conn: &Connection,
    text: &str,
    source_app: Option<&str>,
) -> rusqlite::Result<i64> {
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM clipboard_items WHERE kind='text' AND content=?1 LIMIT 1",
            rusqlite::params![text],
            |r| r.get(0),
        )
        .optional()?;
    let now = now_millis();
    if let Some(id) = existing {
        conn.execute(
            "UPDATE clipboard_items SET created_at=?1 WHERE id=?2",
            rusqlite::params![now, id],
        )?;
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO clipboard_items (kind, content, source_app, pinned, created_at)
         VALUES ('text', ?1, ?2, 0, ?3)",
        rusqlite::params![text, source_app, now],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list_recent(conn: &Connection, limit: i64) -> rusqlite::Result<Vec<ClipItem>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM clipboard_items
         ORDER BY pinned DESC, created_at DESC LIMIT ?1"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params![limit], row_to_item)?;
    rows.collect()
}

pub fn set_pinned(conn: &Connection, id: i64, pinned: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE clipboard_items SET pinned=?1 WHERE id=?2",
        rusqlite::params![pinned as i64, id],
    )?;
    Ok(())
}
```

- [ ] **Step 5: 运行测试，确认通过**

Run: `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test --manifest-path src-tauri/Cargo.toml clipboard`
Expected: 3 个测试全部通过（`set_pinned` 已实现，故排序测试也过）。

- [ ] **Step 6: 提交**
```
git add src-tauri/src/clipboard.rs src-tauri/src/lib.rs
git commit -m "feat(clipboard): insert_text with dedupe, list_recent, set_pinned"
```

---

## Task 4: 上限淘汰（置顶豁免）

**Files:**
- Modify: `src-tauri/src/clipboard.rs`

- [ ] **Step 1: 写失败的测试**

在 `clipboard.rs` 的 `mod tests` 内追加：
```rust
    #[test]
    fn enforce_cap_evicts_oldest_unpinned_beyond_max() {
        let (_d, store) = open();
        for i in 0..5 {
            insert_text(&store.conn, &format!("item{i}"), None).unwrap();
        }
        enforce_cap(&store.conn, 3).unwrap();
        let items = list_recent(&store.conn, 50).unwrap();
        assert_eq!(items.len(), 3, "keeps only the 3 newest");
        assert_eq!(items[0].text.as_deref(), Some("item4"));
        assert_eq!(items[2].text.as_deref(), Some("item2"));
    }

    #[test]
    fn enforce_cap_never_evicts_pinned() {
        let (_d, store) = open();
        let keep = insert_text(&store.conn, "pinned-old", None).unwrap();
        for i in 0..5 {
            insert_text(&store.conn, &format!("item{i}"), None).unwrap();
        }
        set_pinned(&store.conn, keep, true).unwrap();
        enforce_cap(&store.conn, 3).unwrap();
        let items = list_recent(&store.conn, 50).unwrap();
        assert!(
            items.iter().any(|i| i.text.as_deref() == Some("pinned-old")),
            "pinned item survives eviction"
        );
    }
```

- [ ] **Step 2: 运行测试，确认失败**

Run: `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test --manifest-path src-tauri/Cargo.toml clipboard`
Expected: 编译失败——`enforce_cap` 未定义。

- [ ] **Step 3: 实现 enforce_cap**

在 `clipboard.rs` 的 `set_pinned` 之后插入：
```rust
/// Keep at most `max` items by (pinned, recency). Pinned items are never
/// deleted, so the actual row count may exceed `max` if many are pinned.
pub fn enforce_cap(conn: &Connection, max: i64) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM clipboard_items
         WHERE pinned=0 AND id NOT IN (
             SELECT id FROM clipboard_items
             ORDER BY pinned DESC, created_at DESC LIMIT ?1
         )",
        rusqlite::params![max],
    )?;
    Ok(())
}
```

- [ ] **Step 4: 运行测试，确认通过**

Run: `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test --manifest-path src-tauri/Cargo.toml clipboard`
Expected: 全部通过。

- [ ] **Step 5: 提交**
```
git add src-tauri/src/clipboard.rs
git commit -m "feat(clipboard): enforce_cap evicting oldest unpinned"
```

---

## Task 5: 删除 / 清空

**Files:**
- Modify: `src-tauri/src/clipboard.rs`

- [ ] **Step 1: 写失败的测试**

在 `mod tests` 内追加：
```rust
    #[test]
    fn delete_removes_one_item() {
        let (_d, store) = open();
        let a = insert_text(&store.conn, "a", None).unwrap();
        insert_text(&store.conn, "b", None).unwrap();
        delete(&store.conn, a).unwrap();
        let items = list_recent(&store.conn, 50).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].text.as_deref(), Some("b"));
    }

    #[test]
    fn clear_removes_everything() {
        let (_d, store) = open();
        insert_text(&store.conn, "a", None).unwrap();
        insert_text(&store.conn, "b", None).unwrap();
        clear(&store.conn).unwrap();
        assert_eq!(list_recent(&store.conn, 50).unwrap().len(), 0);
    }
```

- [ ] **Step 2: 运行测试，确认失败**

Run: `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test --manifest-path src-tauri/Cargo.toml clipboard`
Expected: 编译失败——`delete` / `clear` 未定义。

- [ ] **Step 3: 实现 delete / clear**

在 `enforce_cap` 之后插入：
```rust
pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM clipboard_items WHERE id=?1",
        rusqlite::params![id],
    )?;
    Ok(())
}

pub fn clear(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM clipboard_items", [])?;
    Ok(())
}
```

- [ ] **Step 4: 运行测试，确认通过**

Run: `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test --manifest-path src-tauri/Cargo.toml clipboard`
Expected: 全部通过。

- [ ] **Step 5: 提交**
```
git add src-tauri/src/clipboard.rs
git commit -m "feat(clipboard): delete and clear"
```

---

## Task 6: 历史内搜索（文本）

**Files:**
- Modify: `src-tauri/src/clipboard.rs`

- [ ] **Step 1: 写失败的测试**

在 `mod tests` 内追加：
```rust
    #[test]
    fn search_matches_substring_case_insensitive() {
        let (_d, store) = open();
        insert_text(&store.conn, "Hello World", None).unwrap();
        insert_text(&store.conn, "goodbye", None).unwrap();
        let hits = search(&store.conn, "world", 50).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].text.as_deref(), Some("Hello World"));
    }

    #[test]
    fn search_empty_query_returns_nothing_matching_only_text() {
        let (_d, store) = open();
        insert_text(&store.conn, "abc", None).unwrap();
        let hits = search(&store.conn, "zzz", 50).unwrap();
        assert_eq!(hits.len(), 0);
    }
```

- [ ] **Step 2: 运行测试，确认失败**

Run: `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test --manifest-path src-tauri/Cargo.toml clipboard`
Expected: 编译失败——`search` 未定义。

- [ ] **Step 3: 实现 search**

在 `clear` 之后插入（`LIKE` 在 SQLite 下对 ASCII 默认不区分大小写）：
```rust
pub fn search(conn: &Connection, query: &str, limit: i64) -> rusqlite::Result<Vec<ClipItem>> {
    let like = format!("%{}%", query);
    let sql = format!(
        "SELECT {SELECT_COLS} FROM clipboard_items
         WHERE kind='text' AND content LIKE ?1
         ORDER BY pinned DESC, created_at DESC LIMIT ?2"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params![like, limit], row_to_item)?;
    rows.collect()
}
```

- [ ] **Step 4: 运行测试，确认通过**

Run: `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test --manifest-path src-tauri/Cargo.toml clipboard`
Expected: 全部通过。

- [ ] **Step 5: 提交**
```
git add src-tauri/src/clipboard.rs
git commit -m "feat(clipboard): substring search over text items"
```

---

## Task 7: 图片落盘 + 缩略图 + 插入图片（按哈希去重）

**Files:**
- Modify: `src-tauri/src/clipboard.rs`

- [ ] **Step 1: 写失败的测试**

在 `mod tests` 内追加（含一个生成微型 PNG 字节的辅助函数）：
```rust
    fn tiny_png() -> Vec<u8> {
        use image::{ImageFormat, RgbImage};
        let img = RgbImage::from_pixel(4, 4, image::Rgb([10, 20, 30]));
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    #[test]
    fn save_image_bytes_writes_original_and_thumb() {
        let dir = tempdir().unwrap();
        let images = dir.path().join("images");
        let thumbs = dir.path().join("thumbs");
        let (image_path, thumb_path, hash) =
            save_image_bytes(&images, &thumbs, &tiny_png()).unwrap();
        assert!(std::path::Path::new(&image_path).exists());
        assert!(std::path::Path::new(&thumb_path).exists());
        assert_eq!(hash.len(), 64, "sha256 hex is 64 chars");
    }

    #[test]
    fn insert_image_then_list_returns_image_item_and_dedupes() {
        let (d, store) = open();
        let images = d.path().join("images");
        let thumbs = d.path().join("thumbs");
        let (ip, tp, hash) = save_image_bytes(&images, &thumbs, &tiny_png()).unwrap();
        let id1 = insert_image(&store.conn, &ip, &tp, &hash, None).unwrap();
        let id2 = insert_image(&store.conn, &ip, &tp, &hash, None).unwrap();
        assert_eq!(id1, id2, "same image hash dedupes to one row");
        let items = list_recent(&store.conn, 50).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, "image");
        assert_eq!(items[0].image_path.as_deref(), Some(ip.as_str()));
        assert_eq!(items[0].thumb_path.as_deref(), Some(tp.as_str()));
        assert!(items[0].text.is_none());
    }
```

- [ ] **Step 2: 运行测试，确认失败**

Run: `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test --manifest-path src-tauri/Cargo.toml clipboard`
Expected: 编译失败——`save_image_bytes` / `insert_image` 未定义。

- [ ] **Step 3: 实现图片落盘与 insert_image**

在文件顶部 `use` 区补充：
```rust
use std::path::Path;
```
在 `search` 之后插入：
```rust
/// Decode `bytes`, write the original as PNG to `images_dir/<sha256>.png` and a
/// 160px thumbnail to `thumbs_dir/<sha256>.png`. Returns (image_path, thumb_path, hash).
pub fn save_image_bytes(
    images_dir: &Path,
    thumbs_dir: &Path,
    bytes: &[u8],
) -> anyhow::Result<(String, String, String)> {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    let hash: String = digest.iter().map(|b| format!("{:02x}", b)).collect();
    std::fs::create_dir_all(images_dir)?;
    std::fs::create_dir_all(thumbs_dir)?;
    let img = image::load_from_memory(bytes)?;
    let image_path = images_dir.join(format!("{hash}.png"));
    let thumb_path = thumbs_dir.join(format!("{hash}.png"));
    img.save(&image_path)?;
    img.thumbnail(160, 160).save(&thumb_path)?;
    Ok((
        image_path.to_string_lossy().into_owned(),
        thumb_path.to_string_lossy().into_owned(),
        hash,
    ))
}

pub fn insert_image(
    conn: &Connection,
    image_path: &str,
    thumb_path: &str,
    hash: &str,
    source_app: Option<&str>,
) -> rusqlite::Result<i64> {
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM clipboard_items WHERE kind='image' AND hash=?1 LIMIT 1",
            rusqlite::params![hash],
            |r| r.get(0),
        )
        .optional()?;
    let now = now_millis();
    if let Some(id) = existing {
        conn.execute(
            "UPDATE clipboard_items SET created_at=?1 WHERE id=?2",
            rusqlite::params![now, id],
        )?;
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO clipboard_items
         (kind, content, image_path, thumb_path, hash, source_app, pinned, created_at)
         VALUES ('image', NULL, ?1, ?2, ?3, ?4, 0, ?5)",
        rusqlite::params![image_path, thumb_path, hash, source_app, now],
    )?;
    Ok(conn.last_insert_rowid())
}
```

- [ ] **Step 4: 运行测试，确认通过**

Run: `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test --manifest-path src-tauri/Cargo.toml clipboard`
Expected: clipboard 下全部测试通过。

- [ ] **Step 5: 全量回归**

Run: `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"; cargo test --manifest-path src-tauri/Cargo.toml`
Expected: storage + config + clipboard 全部通过，无 `error`。

- [ ] **Step 6: 提交**
```
git add src-tauri/src/clipboard.rs
git commit -m "feat(clipboard): image storage with thumbnails and hash dedupe"
```

---

## 完成标准（本计划）

- `cargo test --manifest-path src-tauri/Cargo.toml` 全绿（storage 4 + config 2 + clipboard 约 11 个用例）。
- `clipboard_items` 表升级到 v2（含 `hash` 列），迁移按 `user_version` 增量、可重复打开。
- `clipboard.rs` 提供完整数据层：`insert_text`/`insert_image`/`list_recent`/`search`/`set_pinned`/`delete`/`clear`/`enforce_cap`/`save_image_bytes`，均有测试覆盖。

## 交接给计划 ②b

- 数据层就绪后，②b 负责：系统剪贴板监听（捕获文本/图片字节 → `save_image_bytes` + `insert_image` / `insert_text` → `enforce_cap`）、Tauri 命令（`list_recent`/`search`/`set_pinned`/`delete`/`clear` 暴露给前端）、事件推送刷新、以及命令栏内的剪贴板历史 UI（多选、置顶、图片缩略图）。
- `ClipItem` 已 `Serialize`，可直接作为命令返回类型给前端。
