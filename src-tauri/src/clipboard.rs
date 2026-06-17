use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;

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
}
