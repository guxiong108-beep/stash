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
