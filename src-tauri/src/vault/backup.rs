use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::crypto::Progress;
use crate::error::{Error, Result};
use crate::platform::diag;
use crate::storage::VaultPaths;

/// Copie le coffre vers un nouveau dossier.
///
/// `vault.meta` est écrit en dernier : une sauvegarde interrompue n'a pas
/// d'en-tête et n'est donc pas un coffre ouvrable. Le dossier incomplet est
/// supprimé si l'opération est annulée.
pub fn backup_vault(source: &VaultPaths, destination_parent: &Path, progress: &mut dyn Progress) -> Result<PathBuf> {
    if !source.meta().is_file() || !source.index().is_file() {
        return Err(Error::NotAVault);
    }
    if !destination_parent.is_dir() {
        return Err(Error::Data);
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let mut dest_root = destination_parent.join(format!("NomadVault-sauvegarde-{stamp}"));
    if dest_root.exists() {
        let suffix = hex::encode(crate::crypto::random_bytes::<4>()?);
        dest_root = destination_parent.join(format!("NomadVault-sauvegarde-{stamp}-{suffix}"));
    }
    fs::create_dir(&dest_root)?;
    let dest = VaultPaths::new(&dest_root);
    let outcome = (|| -> Result<()> {
        fs::create_dir(dest.objects_dir())?;
        if let Ok(entries) = fs::read_dir(source.objects_dir()) {
            for entry in entries {
                let entry = entry?;
                if !entry.file_type()?.is_file() {
                    continue;
                }
                let name = entry.file_name();
                let Some(name) = name.to_str() else {
                    continue;
                };
                if name.starts_with('.') {
                    continue;
                }
                if progress.is_cancelled() {
                    return Err(Error::Cancelled);
                }
                copy_file(&entry.path(), &dest.object(name), progress)?;
            }
        }
        copy_file(&source.index(), &dest.index(), progress)?;
        if source.index_backup().is_file() {
            copy_file(&source.index_backup(), &dest.index_backup(), progress)?;
        }
        copy_file(&source.meta(), &dest.meta(), progress)?;
        Ok(())
    })();
    if let Err(error) = outcome {
        let _ = fs::remove_dir_all(&dest_root);
        return Err(error);
    }
    diag("vault.backup");
    Ok(dest_root)
}

fn copy_file(source: &Path, dest: &Path, progress: &mut dyn Progress) -> Result<()> {
    let mut input = File::open(source)?;
    let mut output = File::create(dest)?;
    let total = input.metadata()?.len();
    let mut buf = vec![0u8; 1024 * 1024];
    let mut done = 0u64;
    loop {
        if progress.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let n = read_retry(&mut input, &mut buf)?;
        if n == 0 {
            break;
        }
        output.write_all(&buf[..n])?;
        done += n as u64;
        progress.report(done, Some(total));
    }
    output.sync_all()?;
    if output.metadata()?.len() != total {
        return Err(Error::Damaged);
    }
    Ok(())
}

fn read_retry(input: &mut File, buf: &mut [u8]) -> io::Result<usize> {
    loop {
        match input.read(buf) {
            Ok(n) => return Ok(n),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
}
