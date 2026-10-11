//! Écritures qui ne remplacent un fichier utile qu'une fois le temporaire synchronisé.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::crypto::random_bytes;
use crate::error::Result;

pub fn write_atomic(dest: &Path, data: &[u8]) -> Result<()> {
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let tmp = temporary_sibling(dest)?;
    let write_result = (|| -> Result<()> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        file.write_all(data)?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(error);
    }
    if let Err(error) = replace_file(&tmp, dest) {
        let _ = fs::remove_file(&tmp);
        return Err(error.into());
    }
    if let Some(parent) = dest.parent() {
        let _ = fsync_dir(parent);
    }
    Ok(())
}

pub fn temporary_sibling(dest: &Path) -> Result<PathBuf> {
    let name = dest
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "blob".to_string());
    let suffix = hex::encode(random_bytes::<8>()?);
    Ok(dest.with_file_name(format!(".{name}.tmp-{suffix}")))
}

pub fn replace_file(from: &Path, to: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        windows_replace(from, to)
    }
    #[cfg(not(windows))]
    {
        fs::rename(from, to)
    }
}

pub fn fsync_dir(path: &Path) -> io::Result<()> {
    let dir = File::open(path)?;
    dir.sync_all()
}

/// `MoveFileExW` peut remplacer un fichier existant. `std::fs::rename` ne le
/// fait pas sur Windows, ce qui casserait la validation atomique de l'index.
#[cfg(windows)]
fn windows_replace(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    let from_wide: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to_wide: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    const MOVEFILE_REPLACE_EXISTING: u32 = 1;
    const MOVEFILE_WRITE_THROUGH: u32 = 8;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(existing: *const u16, new_name: *const u16, flags: u32) -> i32;
    }

    // SAFETY: les deux chemins sont des chaînes UTF-16 terminées par NUL,
    // construites ci-dessus, et vivent le temps de l'appel.
    let ok = unsafe {
        MoveFileExW(
            from_wide.as_ptr(),
            to_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
