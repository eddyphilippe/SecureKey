//! Journal technique volontairement pauvre.
//!
//! N'y passez jamais un mot de passe, une clé, un nom de fichier réel ou un
//! contenu. Seuls des codes fixes sont acceptés.

use std::sync::Mutex;

static CAPTURE: Mutex<Option<Vec<String>>> = Mutex::new(None);

pub fn diag(code: &str) {
    eprintln!("securekey {code}");
    if let Ok(mut slot) = CAPTURE.lock() {
        if let Some(lines) = slot.as_mut() {
            lines.push(code.to_string());
        }
    }
}

pub fn start_capture() {
    if let Ok(mut slot) = CAPTURE.lock() {
        *slot = Some(Vec::new());
    }
}

pub fn stop_capture() -> String {
    CAPTURE
        .lock()
        .ok()
        .and_then(|mut slot| slot.take())
        .unwrap_or_default()
        .join("\n")
}
