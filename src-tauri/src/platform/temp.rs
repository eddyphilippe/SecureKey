//! Fichiers temporaires hors de la clé USB.
//!
//! L'écrasement avant suppression est une précaution. Sur un SSD, le contrôleur
//! peut conserver d'anciennes copies : cet effacement n'est pas une garantie.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::crypto::random_bytes;
use crate::error::Result;

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

pub fn mtime_ms(path: &Path) -> Result<u64> {
    let modified = fs::metadata(path)?.modified()?;
    Ok(modified
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(0))
        .unwrap_or(0))
}

pub fn secure_temp_dir(vault_id: &str) -> Result<PathBuf> {
    let suffix = hex::encode(random_bytes::<8>()?);
    let dir = std::env::temp_dir().join(format!("securekey-{vault_id}-{suffix}"));
    fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
    Ok(dir)
}

/// Écrase puis supprime un fichier temporaire.
///
/// Sur un SSD, l'effacement sécurisé n'est pas toujours possible : le
/// contrôleur peut réécrire ailleurs et garder l'ancien bloc.
pub fn wipe_file(path: &Path) -> io::Result<()> {
    let metadata = fs::metadata(path)?;
    let len = metadata.len();
    let mut file = OpenOptions::new().write(true).open(path)?;
    let mut remaining = len;
    let zeros = vec![0u8; 64 * 1024];
    while remaining > 0 {
        let take = usize::try_from(remaining.min(zeros.len() as u64)).unwrap_or(zeros.len());
        file.write_all(&zeros[..take])?;
        remaining -= take as u64;
    }
    file.sync_all()?;
    drop(file);
    fs::remove_file(path)
}

pub fn wipe_tree(dir: &Path) {
    if !dir.exists() {
        return;
    }
    let _ = wipe_children(dir);
    let _ = fs::remove_dir_all(dir);
}

fn wipe_children(dir: &Path) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            let _ = wipe_children(&path);
            let _ = fs::remove_dir(&path);
        } else if entry.file_type()?.is_file() {
            let _ = wipe_file(&path);
        }
    }
    Ok(())
}

