use std::path::Path;
use std::sync::atomic::Ordering;

use tauri::{AppHandle, Manager, State};
use zeroize::Zeroizing;

use crate::error::Error;
use crate::platform::{self, diag};
use crate::vault::{self, backup_vault, CreateOptions, SessionInfo, VerifyReport};

use super::{begin_job, end_job, ensure_active, user_error, AppState, UiProgress};

#[tauri::command]
pub fn discover() -> Option<String> {
    vault::discover()
}

#[tauri::command]
pub fn classify_directory(directory: String) -> String {
    match vault::classify_directory(Path::new(&directory)) {
        vault::DirectoryKind::Vault => "vault",
        vault::DirectoryKind::Blank => "blank",
        vault::DirectoryKind::Occupied => "occupied",
    }
    .to_string()
}

#[tauri::command]
pub async fn create_vault(
    app: AppHandle,
    directory: String,
    name: String,
    password: String,
    confirm: String,
) -> Result<SessionInfo, String> {
    let password = Zeroizing::new(password.into_bytes());
    let confirm = Zeroizing::new(confirm.into_bytes());
    let created = tauri::async_runtime::spawn_blocking(move || {
        vault::create_vault(
            Path::new(&directory),
            &name,
            &password,
            &confirm,
            CreateOptions::recommended(),
        )
    })
    .await
    .map_err(|_| "Opération interrompue.".to_string())?
    .map_err(user_error)?;
    let info = created.info();
    let state = app.state::<AppState>();
    let mut slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
    slot.session = Some(created);
    slot.epoch = slot.epoch.saturating_add(1);
    platform::refresh_volume_watch(&state.volumes, Some(Path::new(&info.vault_path)));
    Ok(info)
}

#[tauri::command]
pub async fn open_vault(
    app: AppHandle,
    directory: String,
    password: String,
    read_only: bool,
) -> Result<SessionInfo, String> {
    let password = Zeroizing::new(password.into_bytes());
    let opened = tauri::async_runtime::spawn_blocking(move || {
        vault::open_vault(Path::new(&directory), &password, read_only)
    })
    .await
    .map_err(|_| "Opération interrompue.".to_string())?
    .map_err(user_error)?;
    let info = opened.info();
    let state = app.state::<AppState>();
    let mut slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
    slot.session = Some(opened);
    slot.epoch = slot.epoch.saturating_add(1);
    platform::refresh_volume_watch(&state.volumes, Some(Path::new(&info.vault_path)));
    Ok(info)
}

#[tauri::command]
pub async fn lock(app: AppHandle) -> Result<(), String> {
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app2.state::<AppState>();
        if let Ok(jobs) = state.jobs.lock() {
            for flag in jobs.values() {
                flag.store(true, Ordering::Relaxed);
            }
        }
        let mut slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
        slot.session.take();
        slot.epoch = slot.epoch.saturating_add(1);
        platform::refresh_volume_watch(&state.volumes, None);
        diag("session.lock");
        Ok(())
    })
    .await
    .map_err(|_| "Opération interrompue.".to_string())?
}

#[tauri::command]
pub fn session_info(state: State<'_, AppState>) -> Result<Option<SessionInfo>, String> {
    ensure_active(&state)?;
    let slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
    Ok(slot.session.as_ref().map(vault::VaultSession::info))
}

#[tauri::command]
pub async fn change_password(
    app: AppHandle,
    current: String,
    next: String,
    confirm: String,
) -> Result<(), String> {
    ensure_active(&app.state::<AppState>())?;
    let current = Zeroizing::new(current.into_bytes());
    let next = Zeroizing::new(next.into_bytes());
    let confirm = Zeroizing::new(confirm.into_bytes());
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app2.state::<AppState>();
        let mut slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
        let session = slot
            .session
            .as_mut()
            .ok_or_else(|| Error::Locked.user_message())?;
        session
            .change_password(&current, &next, &confirm)
            .map_err(user_error)
    })
    .await
    .map_err(|_| "Opération interrompue.".to_string())?
}

#[tauri::command]
pub fn set_auto_lock(state: State<'_, AppState>, seconds: u32) -> Result<(), String> {
    ensure_active(&state)?;
    let mut slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
    let session = slot
        .session
        .as_mut()
        .ok_or_else(|| Error::Locked.user_message())?;
    session.set_auto_lock(seconds).map_err(user_error)
}

#[tauri::command]
pub async fn verify_vault(app: AppHandle) -> Result<VerifyReport, String> {
    ensure_active(&app.state::<AppState>())?;
    let cancel = begin_job(&app.state::<AppState>())?;
    let app2 = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let state = app2.state::<AppState>();
        let slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
        let session = slot
            .session
            .as_ref()
            .ok_or_else(|| Error::Locked.user_message())?;
        let _ = cancel;
        session.verify().map_err(user_error)
    })
    .await
    .map_err(|_| "Opération interrompue.".to_string())?;
    end_job(&app.state::<AppState>());
    result
}

#[tauri::command(rename = "backup_vault")]
pub async fn backup_vault_cmd(
    app: AppHandle,
    destination: String,
) -> Result<String, String> {
    ensure_active(&app.state::<AppState>())?;
    let cancel = begin_job(&app.state::<AppState>())?;
    let app2 = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let state = app2.state::<AppState>();
        let slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
        let session = slot
            .session
            .as_ref()
            .ok_or_else(|| Error::Locked.user_message())?;
        let mut progress = UiProgress::new(app2.clone(), cancel);
        let path = backup_vault(session.paths(), Path::new(&destination), &mut progress)
            .map_err(user_error)?;
        Ok(path.to_string_lossy().into_owned())
    })
    .await
    .map_err(|_| "Opération interrompue.".to_string())?;
    end_job(&app.state::<AppState>());
    result
}

#[tauri::command]
pub fn cancel_job(state: State<'_, AppState>) -> Result<(), String> {
    let jobs = state.jobs.lock().map_err(|_| "Erreur interne.".to_string())?;
    for flag in jobs.values() {
        flag.store(true, Ordering::Relaxed);
    }
    diag("job.cancel");
    Ok(())
}
