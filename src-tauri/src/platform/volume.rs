//! Ferme l'application si la clé qui porte le programme ou le coffre disparaît.
//!
//! La liste des volumes vient du système. On ne touche pas aux fichiers de la
//! clé : les lire après un retrait peut bloquer le processus.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, TryLockError};
use std::time::Duration;

use tauri::{AppHandle, Manager};

use super::diag;
use crate::commands::AppState;

pub fn refresh(volumes: &Mutex<Vec<PathBuf>>, vault: Option<&Path>) {
    let mut next = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        push_removable(&mut next, &exe);
    }
    if let Some(vault) = vault {
        push_removable(&mut next, vault);
    }
    if let Ok(mut slot) = volumes.lock() {
        *slot = next;
    }
}

pub fn install(app: AppHandle) {
    refresh(&app.state::<AppState>().volumes, None);
    std::thread::Builder::new()
        .name("volume-watch".into())
        .spawn(move || loop {
            if let Some(state) = app.try_state::<AppState>() {
                if watched_volume_missing(&state.volumes) {
                    close_now(&app);
                }
            }
            std::thread::sleep(Duration::from_millis(400));
        })
        .ok();
}

fn close_now(app: &AppHandle) {
    diag("volume.removed");
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(jobs) = state.jobs.try_lock() {
            for flag in jobs.values() {
                flag.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
        if let Ok(mut slot) = state.inner.try_lock() {
            slot.session.take();
        }
    }
    std::process::exit(0);
}

fn watched_volume_missing(volumes: &Mutex<Vec<PathBuf>>) -> bool {
    let slot = match volumes.try_lock() {
        Ok(slot) => slot,
        Err(TryLockError::WouldBlock | TryLockError::Poisoned(_)) => return false,
    };
    if slot.is_empty() {
        return false;
    }
    let mounted = current_mounts();
    if mounted.is_empty() {
        return false;
    }
    slot.iter().any(|anchor| !mounted.iter().any(|mount| mount == anchor))
}

fn push_removable(list: &mut Vec<PathBuf>, path: &Path) {
    let mounts = current_mounts();
    let Some(owner) = owning_mount(path, &mounts) else {
        return;
    };
    if is_removable_mount(&owner) && !list.iter().any(|mount| mount == &owner) {
        list.push(owner);
    }
}

pub(crate) fn owning_mount(path: &Path, mounts: &[PathBuf]) -> Option<PathBuf> {
    mounts
        .iter()
        .filter(|mount| is_prefix(mount, path))
        .max_by_key(|mount| mount.components().count())
        .cloned()
}

fn is_prefix(mount: &Path, path: &Path) -> bool {
    let mount: Vec<_> = mount.components().collect();
    let path: Vec<_> = path.components().collect();
    !mount.is_empty() && path.starts_with(&mount)
}

#[cfg(target_os = "macos")]
fn is_removable_mount(mount: &Path) -> bool {
    is_prefix(Path::new("/Volumes"), mount) && mount.components().count() > 2
}

#[cfg(all(unix, not(target_os = "macos")))]
fn is_removable_mount(mount: &Path) -> bool {
    ["/media", "/mnt", "/run/media", "/Volumes"]
        .iter()
        .any(|prefix| is_prefix(Path::new(prefix), mount) && mount != Path::new(prefix))
}

#[cfg(windows)]
fn is_removable_mount(mount: &Path) -> bool {
    match (drive_letter(mount), system_drive()) {
        (Some(drive), Some(system)) => drive != system,
        _ => false,
    }
}

#[cfg(target_os = "macos")]
fn current_mounts() -> Vec<PathBuf> {
    unsafe {
        let mut buf: *mut libc::statfs = std::ptr::null_mut();
        let count = libc::getmntinfo(&mut buf, libc::MNT_NOWAIT);
        if count <= 0 || buf.is_null() {
            return Vec::new();
        }
        let entries = std::slice::from_raw_parts(buf, count as usize);
        entries
            .iter()
            .filter_map(|entry| c_string(&entry.f_mntonname))
            .map(PathBuf::from)
            .collect()
    }
}

#[cfg(target_os = "macos")]
fn c_string(bytes: &[libc::c_char]) -> Option<String> {
    let raw = bytes.iter().map(|byte| *byte as u8).take_while(|byte| *byte != 0).collect::<Vec<_>>();
    String::from_utf8(raw).ok()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn current_mounts() -> Vec<PathBuf> {
    let text = std::fs::read_to_string("/proc/mounts").unwrap_or_default();
    text.lines()
        .filter_map(|line| line.split_whitespace().nth(1))
        .map(|mount| PathBuf::from(unescape_mount(mount)))
        .collect()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn unescape_mount(field: &str) -> String {
    field
        .replace("\\040", " ")
        .replace("\\011", "\t")
        .replace("\\012", "\n")
        .replace("\\134", "\\")
}

#[cfg(windows)]
fn current_mounts() -> Vec<PathBuf> {
    let mask = unsafe { GetLogicalDrives() };
    (0..26)
        .filter(|bit| mask & (1 << bit) != 0)
        .map(|bit| {
            let letter = (b'A' + bit) as char;
            PathBuf::from(format!("{letter}:\\"))
        })
        .collect()
}

#[cfg(windows)]
fn drive_letter(path: &Path) -> Option<char> {
    let text = path.to_string_lossy();
    let mut chars = text.chars();
    let letter = chars.next()?;
    if chars.next() == Some(':') && letter.is_ascii_alphabetic() {
        Some(letter.to_ascii_uppercase())
    } else {
        None
    }
}

#[cfg(windows)]
fn system_drive() -> Option<char> {
    std::env::var("SystemDrive")
        .ok()
        .as_deref()
        .and_then(|drive| drive_letter(Path::new(drive)))
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn GetLogicalDrives() -> u32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_longest_mount_point_owns_the_path() {
        let mounts = vec![
            PathBuf::from("/"),
            PathBuf::from("/Volumes"),
            PathBuf::from("/Volumes/Sauvegarde"),
            PathBuf::from("/Volumes/Autre"),
        ];
        let path = Path::new("/Volumes/Sauvegarde/SecureKey.app/Contents/MacOS/SecureKey");
        assert_eq!(
            owning_mount(path, &mounts).unwrap(),
            PathBuf::from("/Volumes/Sauvegarde")
        );
    }

    #[test]
    fn a_similar_name_is_not_the_same_volume() {
        let mounts = vec![PathBuf::from("/Volumes/Sauvegarde")];
        let path = Path::new("/Volumes/Sauvegarde-copie/Vault");
        assert!(owning_mount(path, &mounts).is_none());
    }

    #[test]
    fn the_internal_disk_is_not_treated_as_a_key() {
        #[cfg(target_os = "macos")]
        {
            assert!(!is_removable_mount(Path::new("/")));
            assert!(!is_removable_mount(Path::new("/System/Volumes/Data")));
            assert!(is_removable_mount(Path::new("/Volumes/Sauvegarde")));
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            assert!(!is_removable_mount(Path::new("/")));
            assert!(!is_removable_mount(Path::new("/home")));
            assert!(is_removable_mount(Path::new("/media/eddy/Sauvegarde")));
        }
        #[cfg(windows)]
        {
            assert!(!is_removable_mount(Path::new(r"C:\")));
            assert!(is_removable_mount(Path::new(r"E:\Sauvegarde")));
        }
    }
}
