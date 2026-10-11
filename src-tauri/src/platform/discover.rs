use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Error, Result};

/// Cherche un coffre voisin de l'exécutable, pour un lancement depuis la clé.
///
/// La recherche remonte au plus six dossiers. Le premier `Vault/vault.meta`
/// rencontré gagne, ce qui correspond à la disposition Mac, Windows et Linux.
pub fn discover_vault() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(found) = walk_for_vault(&exe, 6) {
            return Some(found);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        if cwd.join("vault.meta").is_file() {
            return Some(cwd);
        }
        let nested = cwd.join("Vault");
        if nested.join("vault.meta").is_file() {
            return Some(nested);
        }
    }
    None
}

fn walk_for_vault(start: &Path, max_depth: usize) -> Option<PathBuf> {
    let mut current = start.parent();
    for _ in 0..max_depth {
        let dir = current?;
        let nested = dir.join("Vault");
        if nested.join("vault.meta").is_file() {
            return Some(nested);
        }
        if dir.join("vault.meta").is_file() {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    None
}

pub fn open_with_associated_app(path: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let status = Command::new("open").arg(path).status()?;
        if status.success() {
            Ok(())
        } else {
            Err(Error::Data)
        }
    }
    #[cfg(target_os = "windows")]
    {
        windows_open(path)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let status = Command::new("xdg-open").arg(path).status()?;
        if status.success() {
            Ok(())
        } else {
            Err(Error::Data)
        }
    }
}

#[cfg(target_os = "windows")]
fn windows_open(path: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let operation: Vec<u16> = std::ffi::OsStr::new("open")
        .encode_wide()
        .chain(Some(0))
        .collect();
    const SW_SHOWNORMAL: i32 = 1;

    #[link(name = "shell32")]
    unsafe extern "system" {
        fn ShellExecuteW(
            hwnd: *mut core::ffi::c_void,
            operation: *const u16,
            file: *const u16,
            parameters: *const u16,
            directory: *const u16,
            show_cmd: i32,
        ) -> isize;
    }

    // SAFETY: chaînes UTF-16 terminées par NUL, valides pendant l'appel.
    let code = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            wide.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if code > 32 {
        Ok(())
    } else {
        Err(Error::Data)
    }
}
