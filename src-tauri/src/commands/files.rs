use std::path::Path;

use tauri::{AppHandle, Manager, State};

use crate::error::Error;
use crate::vault::{Crumb, EntryDto, TempStatus};

use super::{begin_job, end_job, ensure_active, user_error, AppState, UiProgress};

#[tauri::command]
pub fn list_entries(
    state: State<'_, AppState>,
    view: String,
    folder_id: Option<String>,
) -> Result<Vec<EntryDto>, String> {
    ensure_active(&state)?;
    let slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
    let session = slot
        .session
        .as_ref()
        .ok_or_else(|| Error::Locked.user_message())?;
    session
        .list(&view, folder_id.as_deref())
        .map_err(user_error)
}

#[tauri::command]
pub fn breadcrumbs(
    state: State<'_, AppState>,
    folder_id: Option<String>,
) -> Result<Vec<Crumb>, String> {
    ensure_active(&state)?;
    let slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
    let session = slot
        .session
        .as_ref()
        .ok_or_else(|| Error::Locked.user_message())?;
    session.breadcrumbs(folder_id.as_deref()).map_err(user_error)
}

#[tauri::command]
pub async fn import_paths(
    app: AppHandle,
    paths: Vec<String>,
    parent_id: Option<String>,
    folders: bool,
) -> Result<u32, String> {
    ensure_active(&app.state::<AppState>())?;
    let cancel = begin_job(&app.state::<AppState>())?;
    let app2 = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let state = app2.state::<AppState>();
        let mut progress = UiProgress::new(app2.clone(), cancel);
        let mut slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
        let session = slot
            .session
            .as_mut()
            .ok_or_else(|| Error::Locked.user_message())?;
        let mut count = 0u32;
        for path in paths {
            if folders {
                let report = session
                    .import_tree(Path::new(&path), parent_id.as_deref(), &mut progress)
                    .map_err(user_error)?;
                count += u32::try_from(report.entries.len()).unwrap_or(u32::MAX);
            } else {
                session
                    .import_file(Path::new(&path), parent_id.as_deref(), &mut progress)
                    .map_err(user_error)?;
                count += 1;
            }
        }
        Ok(count)
    })
    .await
    .map_err(|_| "Opération interrompue.".to_string())?;
    end_job(&app.state::<AppState>());
    result
}

#[tauri::command]
pub fn create_folder(
    state: State<'_, AppState>,
    parent_id: Option<String>,
    name: String,
) -> Result<EntryDto, String> {
    ensure_active(&state)?;
    let mut slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
    let session = slot
        .session
        .as_mut()
        .ok_or_else(|| Error::Locked.user_message())?;
    session
        .create_folder(parent_id.as_deref(), &name)
        .map_err(user_error)
}

#[tauri::command]
pub fn rename_entry(state: State<'_, AppState>, id: String, name: String) -> Result<EntryDto, String> {
    ensure_active(&state)?;
    let mut slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
    let session = slot
        .session
        .as_mut()
        .ok_or_else(|| Error::Locked.user_message())?;
    session.rename_entry(&id, &name).map_err(user_error)
}

#[tauri::command]
pub fn set_comment(
    state: State<'_, AppState>,
    id: String,
    comment: String,
) -> Result<EntryDto, String> {
    ensure_active(&state)?;
    let mut slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
    let session = slot
        .session
        .as_mut()
        .ok_or_else(|| Error::Locked.user_message())?;
    session.set_comment(&id, &comment).map_err(user_error)
}

#[tauri::command]
pub fn set_favorite(
    state: State<'_, AppState>,
    id: String,
    favorite: bool,
) -> Result<EntryDto, String> {
    ensure_active(&state)?;
    let mut slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
    let session = slot
        .session
        .as_mut()
        .ok_or_else(|| Error::Locked.user_message())?;
    session.set_favorite(&id, favorite).map_err(user_error)
}

#[tauri::command]
pub fn move_entry(
    state: State<'_, AppState>,
    id: String,
    parent_id: Option<String>,
) -> Result<EntryDto, String> {
    ensure_active(&state)?;
    let mut slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
    let session = slot
        .session
        .as_mut()
        .ok_or_else(|| Error::Locked.user_message())?;
    session
        .move_entry(&id, parent_id.as_deref())
        .map_err(user_error)
}

#[tauri::command]
pub fn trash_entries(state: State<'_, AppState>, ids: Vec<String>) -> Result<(), String> {
    ensure_active(&state)?;
    let mut slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
    let session = slot
        .session
        .as_mut()
        .ok_or_else(|| Error::Locked.user_message())?;
    session.trash_entries(&ids).map_err(user_error)
}

#[tauri::command]
pub fn restore_entries(state: State<'_, AppState>, ids: Vec<String>) -> Result<(), String> {
    ensure_active(&state)?;
    let mut slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
    let session = slot
        .session
        .as_mut()
        .ok_or_else(|| Error::Locked.user_message())?;
    session.restore_entries(&ids).map_err(user_error)
}

#[tauri::command]
pub fn delete_permanent(state: State<'_, AppState>, ids: Vec<String>) -> Result<(), String> {
    ensure_active(&state)?;
    let mut slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
    let session = slot
        .session
        .as_mut()
        .ok_or_else(|| Error::Locked.user_message())?;
    session.delete_permanent(&ids).map_err(user_error)
}

#[tauri::command]
pub async fn export_entries(
    app: AppHandle,
    ids: Vec<String>,
    destination: String,
) -> Result<u32, String> {
    ensure_active(&app.state::<AppState>())?;
    let cancel = begin_job(&app.state::<AppState>())?;
    let app2 = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let state = app2.state::<AppState>();
        let mut progress = UiProgress::new(app2.clone(), cancel);
        let slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
        let session = slot
            .session
            .as_ref()
            .ok_or_else(|| Error::Locked.user_message())?;
        session
            .export_entries(&ids, Path::new(&destination), &mut progress)
            .map_err(user_error)
    })
    .await
    .map_err(|_| "Opération interrompue.".to_string())?;
    end_job(&app.state::<AppState>());
    result
}

#[tauri::command]
pub async fn prepare_temp(app: AppHandle, id: String) -> Result<String, String> {
    ensure_active(&app.state::<AppState>())?;
    let cancel = begin_job(&app.state::<AppState>())?;
    let app2 = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let state = app2.state::<AppState>();
        let mut progress = UiProgress::new(app2.clone(), cancel);
        let mut slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
        let session = slot
            .session
            .as_mut()
            .ok_or_else(|| Error::Locked.user_message())?;
        let path = session.prepare_temp(&id, &mut progress).map_err(user_error)?;
        Ok(path.to_string_lossy().into_owned())
    })
    .await
    .map_err(|_| "Opération interrompue.".to_string())?;
    end_job(&app.state::<AppState>());
    result
}

#[tauri::command]
pub fn launch_temp(state: State<'_, AppState>, id: String) -> Result<(), String> {
    ensure_active(&state)?;
    let slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
    let session = slot
        .session
        .as_ref()
        .ok_or_else(|| Error::Locked.user_message())?;
    session.launch_temp(&id).map_err(user_error)
}

#[tauri::command]
pub fn temp_status(state: State<'_, AppState>) -> Result<Vec<TempStatus>, String> {
    ensure_active(&state)?;
    let slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
    let session = slot
        .session
        .as_ref()
        .ok_or_else(|| Error::Locked.user_message())?;
    session.temp_status().map_err(user_error)
}

#[tauri::command]
pub async fn reimport_temp(app: AppHandle, id: String) -> Result<EntryDto, String> {
    ensure_active(&app.state::<AppState>())?;
    let cancel = begin_job(&app.state::<AppState>())?;
    let app2 = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let state = app2.state::<AppState>();
        let mut progress = UiProgress::new(app2.clone(), cancel);
        let mut slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
        let session = slot
            .session
            .as_mut()
            .ok_or_else(|| Error::Locked.user_message())?;
        session.reimport_temp(&id, &mut progress).map_err(user_error)
    })
    .await
    .map_err(|_| "Opération interrompue.".to_string())?;
    end_job(&app.state::<AppState>());
    result
}

#[tauri::command]
pub fn discard_temp(state: State<'_, AppState>, id: String) -> Result<(), String> {
    ensure_active(&state)?;
    let mut slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
    let session = slot
        .session
        .as_mut()
        .ok_or_else(|| Error::Locked.user_message())?;
    session.discard_temp(&id);
    Ok(())
}
