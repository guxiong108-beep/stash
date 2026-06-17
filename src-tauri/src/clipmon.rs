use std::sync::Mutex;

use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher,
    ClipboardWatcherContext, ContentFormat,
};
use clipboard_rs::common::RustImage;
use tauri::{AppHandle, Emitter, Manager};

use crate::clipboard;
use crate::config::Config;
use crate::paths;
use crate::storage::Store;

/// Event emitted to the frontend whenever a new clipboard item is recorded.
pub const CLIP_CHANGED_EVENT: &str = "clip://changed";

struct Monitor {
    app: AppHandle,
    ctx: ClipboardContext,
}

impl Monitor {
    fn capture(&self) -> anyhow::Result<bool> {
        let base = self
            .app
            .path()
            .data_dir()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?
            .join("Stash");
        let cfg = Config::load(&base.join("config.json")).unwrap_or_default();

        let state = self.app.state::<Mutex<Store>>();
        let store = state.lock().map_err(|e| anyhow::anyhow!(e.to_string()))?;

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
                let png_buf = img.to_png().map_err(|e| anyhow::anyhow!(e.to_string()))?;
                let bytes = png_buf.get_bytes();
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
        watcher.start_watch();
    });
}
