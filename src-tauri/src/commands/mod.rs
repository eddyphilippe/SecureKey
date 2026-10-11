mod files;
mod session;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tauri::{AppHandle, Emitter};

use crate::crypto::Progress;
use crate::error::Error;
use crate::platform::{self, diag};
use crate::vault::VaultSession;

pub struct AppState {
    pub inner: Mutex<SessionSlot>,
    pub jobs: Mutex<HashMap<String, Arc<AtomicBool>>>,
    pub volumes: Mutex<Vec<std::path::PathBuf>>,
}

pub struct SessionSlot {
    pub epoch: u64,
    pub session: Option<VaultSession>,
}

struct UiProgress {
    app: AppHandle,
    cancel: Arc<AtomicBool>,
    last: Instant,
    started: bool,
}

impl UiProgress {
    fn new(app: AppHandle, cancel: Arc<AtomicBool>) -> Self {
        Self {
            app,
            cancel,
            last: Instant::now(),
            started: false,
        }
    }
}

impl Progress for UiProgress {
    fn report(&mut self, done: u64, total: Option<u64>) {
        let finished = total.is_some_and(|total| done >= total);
        if self.started && !finished && self.last.elapsed().as_millis() < 80 {
            return;
        }
        self.started = true;
        self.last = Instant::now();
        let _ = self.app.emit(
            "job-progress",
            serde_json::json!({ "done": done, "total": total }),
        );
    }

    fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

fn user_error(error: Error) -> String {
    let code = match &error {
        Error::AuthFailed => "err.auth",
        Error::Damaged => "err.damaged",
        Error::Cancelled => "err.cancelled",
        Error::ReadOnly => "err.readonly",
        Error::Io(_) => "err.io",
        _ => "err",
    };
    diag(code);
    error.user_message()
}

fn begin_job(state: &AppState) -> Result<Arc<AtomicBool>, String> {
    let mut jobs = state.jobs.lock().map_err(|_| "Erreur interne.".to_string())?;
    if !jobs.is_empty() {
        return Err(Error::Busy.user_message());
    }
    let flag = Arc::new(AtomicBool::new(false));
    jobs.insert("current".to_string(), flag.clone());
    Ok(flag)
}

fn end_job(state: &AppState) {
    if let Ok(mut jobs) = state.jobs.lock() {
        jobs.clear();
    }
}

fn ensure_active(state: &AppState) -> Result<(), String> {
    let mut slot = state.inner.lock().map_err(|_| "Erreur interne.".to_string())?;
    let expired = slot.session.as_ref().is_some_and(|session| session.is_expired());
    if expired {
        slot.session.take();
        slot.epoch = slot.epoch.saturating_add(1);
        diag("session.autolock");
        return Err(Error::Locked.user_message());
    }
    if let Some(session) = slot.session.as_mut() {
        session.touch();
    }
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            inner: Mutex::new(SessionSlot {
                epoch: 0,
                session: None,
            }),
            jobs: Mutex::new(HashMap::new()),
            volumes: Mutex::new(Vec::new()),
        })
        .setup(|app| {
            platform::install_volume_watch(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            session::discover,
            session::create_vault,
            session::open_vault,
            session::lock,
            session::session_info,
            session::change_password,
            session::set_auto_lock,
            session::verify_vault,
            session::backup_vault_cmd,
            session::cancel_job,
            files::list_entries,
            files::breadcrumbs,
            files::import_paths,
            files::create_folder,
            files::rename_entry,
            files::set_comment,
            files::set_favorite,
            files::move_entry,
            files::trash_entries,
            files::restore_entries,
            files::delete_permanent,
            files::export_entries,
            files::prepare_temp,
            files::launch_temp,
            files::temp_status,
            files::reimport_temp,
            files::discard_temp,
        ])
        .run(tauri::generate_context!())
        .expect("SecureZip n'a pas pu démarrer");
}
