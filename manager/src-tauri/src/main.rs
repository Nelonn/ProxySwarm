#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::path::PathBuf;

use tauri::{AppHandle, Manager};

const STATE_FILE: &str = "state.json";
const DRAFTS_DIR: &str = "drafts";

#[tauri::command]
fn load_app_state(app: AppHandle) -> Result<Option<String>, String> {
    read_optional_file(state_file_path(&app)?)
}

#[tauri::command]
fn save_app_state(app: AppHandle, contents: String) -> Result<(), String> {
    write_file(state_file_path(&app)?, contents)
}

#[tauri::command]
fn load_node_draft(app: AppHandle, node_id: String) -> Result<Option<String>, String> {
    read_optional_file(draft_file_path(&app, &node_id)?)
}

#[tauri::command]
fn save_node_draft(app: AppHandle, node_id: String, contents: String) -> Result<(), String> {
    write_file(draft_file_path(&app, &node_id)?, contents)
}

fn state_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    let app_dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    Ok(app_dir.join(STATE_FILE))
}

fn draft_file_path(app: &AppHandle, node_id: &str) -> Result<PathBuf, String> {
    let app_dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    Ok(app_dir.join(DRAFTS_DIR).join(format!("{node_id}.json")))
}

fn read_optional_file(path: PathBuf) -> Result<Option<String>, String> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.to_string()),
    }
}

fn write_file(path: PathBuf, contents: String) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(path, contents).map_err(|err| err.to_string())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            load_app_state,
            save_app_state,
            load_node_draft,
            save_node_draft
        ])
        .run(tauri::generate_context!())
        .expect("failed to run ProxySwarm desktop app");
}
