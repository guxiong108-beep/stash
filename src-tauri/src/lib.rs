mod storage;
mod config;
mod clipboard;
mod paths;

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
        .plugin(tauri_plugin_opener::init())
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
            let handle = app.handle().clone();
            let dir = stash_dir(&handle)
                .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            let store = Store::open(&paths::db_path(&dir))
                .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            app.manage(Mutex::new(store));

            // A hotkey conflict (e.g. another app owns Alt+Space) must not crash
            // startup — log and keep running so the app stays usable.
            let main_hotkey = Shortcut::new(Some(Modifiers::ALT), Code::Space);
            if let Err(e) = app.global_shortcut().register(main_hotkey) {
                eprintln!("[stash] failed to register global hotkey Alt+Space: {e}");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
