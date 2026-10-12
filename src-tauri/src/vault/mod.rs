//! Ouverture, verrouillage et consultation d'un coffre.
//!
//! La clé maître ne quitte pas ce module vers l'interface : les commandes
//! Tauri ne reçoivent que des descriptions de fichiers.

mod backup;
mod files;
mod header;
mod index;
mod verify;

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use subtle::ConstantTimeEq;
use uuid::Uuid;
use zeroize::Zeroize;

use crate::crypto::{
    derive_kek, random_bytes, seal_to_writer, KdfParams, MasterKey, NopProgress, DEFAULT_CHUNK_SIZE,
    INDEX_CHUNK_SIZE, MAX_CHUNK_SIZE, MIN_CHUNK_SIZE, SALT_LEN,
};
use crate::error::{Error, Result};
use crate::metadata::{is_document, is_image, validate_vault_name, Catalog, Entry, EntryKind};
use crate::platform::{diag, discover_vault, now_ms};
use crate::storage::VaultPaths;

pub use backup::backup_vault;
pub use files::{ImportReport, TempStatus};
pub use header::FORMAT_VERSION;
pub use verify::{VerifyIssue, VerifyReport};

use header::{build_meta, load_meta, save_meta, unlock_master};
use index::read_catalog;

const AUTO_LOCK_CHOICES: [u32; 5] = [0, 60, 300, 900, 1800];

pub struct CreateOptions {
    pub kdf: KdfParams,
    pub chunk_size: u32,
}

impl CreateOptions {
    pub fn recommended() -> Self {
        Self {
            kdf: KdfParams::recommended(),
            chunk_size: DEFAULT_CHUNK_SIZE,
        }
    }

    /// Réservé aux tests. L'interface crée toujours un coffre avec
    /// [`CreateOptions::recommended`].
    pub fn for_tests() -> Self {
        Self {
            kdf: KdfParams::for_tests(),
            chunk_size: 4096,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub vault_name: String,
    pub read_only: bool,
    pub auto_lock_secs: u32,
    pub vault_path: String,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryDto {
    pub id: String,
    pub parent_id: Option<String>,
    pub kind: String,
    pub name: String,
    pub size: u64,
    pub created_ms: u64,
    pub modified_ms: u64,
    pub favorite: bool,
    pub trashed: bool,
    pub comment: String,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Crumb {
    pub id: String,
    pub name: String,
}

pub(crate) struct OpenTemp {
    entry_id: String,
    path: std::path::PathBuf,
    dir: std::path::PathBuf,
    len: u64,
    modified_ms: u64,
}

pub struct VaultSession {
    pub(crate) paths: VaultPaths,
    pub(crate) vault_id: [u8; 16],
    pub(crate) master: MasterKey,
    pub(crate) catalog: Catalog,
    pub(crate) read_only: bool,
    pub(crate) write_chunk_size: u32,
    temps: Vec<OpenTemp>,
    last_activity: Instant,
}

impl Drop for VaultSession {
    fn drop(&mut self) {
        self.discard_temps();
        diag("session.drop");
    }
}

impl VaultSession {
    pub fn info(&self) -> SessionInfo {
        SessionInfo {
            vault_name: self.catalog.name.clone(),
            read_only: self.read_only,
            auto_lock_secs: self.catalog.auto_lock_secs,
            vault_path: self.paths.root().to_string_lossy().into_owned(),
        }
    }

    pub fn paths(&self) -> &VaultPaths {
        &self.paths
    }

    pub fn touch(&mut self) {
        self.last_activity = Instant::now();
    }

    pub fn is_expired(&self) -> bool {
        let seconds = self.catalog.auto_lock_secs;
        if seconds == 0 {
            return false;
        }
        self.last_activity.elapsed() >= Duration::from_secs(u64::from(seconds))
    }

    pub fn set_auto_lock(&mut self, seconds: u32) -> Result<()> {
        if !AUTO_LOCK_CHOICES.contains(&seconds) {
            return Err(Error::InvalidAutoLock);
        }
        self.mutate(|catalog| {
            catalog.auto_lock_secs = seconds;
            Ok(())
        })
    }

    pub fn change_password(&mut self, current: &[u8], next: &[u8], confirm: &[u8]) -> Result<()> {
        self.ensure_writable()?;
        check_password_pair(next, confirm)?;
        let mut meta = load_meta(&self.paths.meta())?;
        let unlocked = unlock_master(&meta, current)?;
        if !unlocked.ct_eq(&self.master) {
            return Err(Error::AuthFailed);
        }
        let params = meta.kdf_params()?;
        let salt: [u8; SALT_LEN] = random_bytes()?;
        let kek = derive_kek(next, &salt, &params)?;
        meta.kdf.salt_hex = hex::encode(salt);
        header::rewrap(&mut meta, &kek, &self.master)?;
        drop(kek);
        save_meta(&self.paths.meta(), &meta)?;
        diag("vault.password");
        Ok(())
    }

    pub fn list(&self, view: &str, folder_id: Option<&str>) -> Result<Vec<EntryDto>> {
        let mut items: Vec<&Entry> = match view {
            "folder" => {
                self.catalog.ensure_folder(folder_id)?;
                self.catalog.children(folder_id)
            }
            "documents" => self
                .catalog
                .entries
                .iter()
                .filter(|entry| {
                    entry.kind == EntryKind::File && !self.catalog.hidden(entry) && is_document(&entry.name)
                })
                .collect(),
            "images" => self
                .catalog
                .entries
                .iter()
                .filter(|entry| {
                    entry.kind == EntryKind::File && !self.catalog.hidden(entry) && is_image(&entry.name)
                })
                .collect(),
            "favorites" => self
                .catalog
                .entries
                .iter()
                .filter(|entry| entry.favorite && !self.catalog.hidden(entry))
                .collect(),
            "trash" => self
                .catalog
                .entries
                .iter()
                .filter(|entry| entry.trashed)
                .collect(),
            _ => return Err(Error::Data),
        };
        items.sort_by(|left, right| {
            let left_folder = left.kind == EntryKind::Folder;
            let right_folder = right.kind == EntryKind::Folder;
            right_folder
                .cmp(&left_folder)
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
        });
        Ok(items.into_iter().map(entry_dto).collect())
    }

    pub fn breadcrumbs(&self, folder_id: Option<&str>) -> Result<Vec<Crumb>> {
        let Some(mut current) = folder_id.map(str::to_string) else {
            return Ok(Vec::new());
        };
        let mut crumbs = Vec::new();
        let mut guard = 0;
        while guard < 10_000 {
            let entry = self.catalog.entry(&current)?;
            crumbs.push(Crumb {
                id: entry.id.clone(),
                name: entry.name.clone(),
            });
            match &entry.parent {
                Some(parent) => current = parent.clone(),
                None => break,
            }
            guard += 1;
        }
        crumbs.reverse();
        Ok(crumbs)
    }

    pub(crate) fn ensure_writable(&self) -> Result<()> {
        if self.read_only {
            Err(Error::ReadOnly)
        } else {
            Ok(())
        }
    }
}

pub fn discover() -> Option<String> {
    discover_vault().map(|path| path.to_string_lossy().into_owned())
}

/// Ce qu'un dossier choisi par l'utilisateur peut devenir.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectoryKind {
    /// `vault.meta` est présent : le coffre se déverrouille.
    Vault,
    /// Rien, ou seulement le dossier `objects` vide livré avec le programme.
    Blank,
    /// D'autres fichiers : ce n'est pas un emplacement de coffre.
    Occupied,
}

pub fn classify_directory(directory: &Path) -> DirectoryKind {
    if directory.join("vault.meta").is_file() {
        return DirectoryKind::Vault;
    }
    if !directory.exists() || is_blank_slot(directory) {
        return DirectoryKind::Blank;
    }
    DirectoryKind::Occupied
}

fn is_ignored_name(name: &std::ffi::OsStr) -> bool {
    let text = name.to_string_lossy();
    text.starts_with("._") || text == ".DS_Store"
}

fn is_blank_slot(directory: &Path) -> bool {
    let Ok(entries) = fs::read_dir(directory) else {
        return false;
    };
    for entry in entries {
        let Ok(entry) = entry else {
            return false;
        };
        if is_ignored_name(&entry.file_name()) {
            continue;
        }
        let only_empty_objects = entry.file_name() == "objects"
            && entry.path().is_dir()
            && is_blank_slot(&entry.path());
        if !only_empty_objects {
            return false;
        }
    }
    true
}

pub fn create_vault(
    directory: &Path,
    name: &str,
    password: &[u8],
    confirm: &[u8],
    options: CreateOptions,
) -> Result<VaultSession> {
    validate_vault_name(name)?;
    check_password_pair(password, confirm)?;
    if !(MIN_CHUNK_SIZE..=MAX_CHUNK_SIZE).contains(&options.chunk_size) {
        return Err(Error::Data);
    }
    prepare_directory(directory)?;
    let paths = VaultPaths::new(directory);
    let vault_uuid = Uuid::new_v4();
    let vault_id = *vault_uuid.as_bytes();
    let salt: [u8; SALT_LEN] = random_bytes()?;
    let kek = derive_kek(password, &salt, &options.kdf)?;
    let master = MasterKey::new(random_bytes()?);
    let wrapped = crate::crypto::wrap_master_key(&kek, &master, &vault_id, FORMAT_VERSION)?;
    drop(kek);
    let catalog = Catalog::new(name.to_string());
    let mut plain = catalog.to_bytes()?;
    let mut sealed = Vec::new();
    let mut cursor = std::io::Cursor::new(plain.as_slice());
    seal_to_writer(
        &master,
        &crate::crypto::index_aad(&vault_id),
        &mut cursor,
        &mut sealed,
        INDEX_CHUNK_SIZE,
        Some(u64::try_from(plain.len()).unwrap_or(u64::MAX)),
        &mut NopProgress,
    )?;
    plain.zeroize();
    crate::storage::write_atomic(&paths.index(), &sealed)?;
    sealed.zeroize();
    let meta = build_meta(vault_uuid, now_ms(), &options.kdf, &salt, &wrapped);
    save_meta(&paths.meta(), &meta)?;
    diag("vault.create");
    Ok(VaultSession {
        paths,
        vault_id,
        master,
        catalog,
        read_only: false,
        write_chunk_size: options.chunk_size,
        temps: Vec::new(),
        last_activity: Instant::now(),
    })
}

pub fn open_vault(directory: &Path, password: &[u8], read_only: bool) -> Result<VaultSession> {
    let paths = VaultPaths::new(directory);
    if !paths.meta().is_file() {
        return Err(Error::NotAVault);
    }
    let meta = load_meta(&paths.meta())?;
    let vault_id = meta.vault_id_bytes()?;
    let master = match unlock_master(&meta, password) {
        Ok(master) => master,
        Err(error) => {
            diag("vault.open.auth_failed");
            return Err(error);
        }
    };
    let catalog = match read_catalog(&master, &vault_id, &paths.index()) {
        Ok(catalog) => catalog,
        Err(error) => {
            diag("vault.open.damaged");
            return Err(error);
        }
    };
    diag("vault.open.ok");
    Ok(VaultSession {
        paths,
        vault_id,
        master,
        catalog,
        read_only,
        write_chunk_size: DEFAULT_CHUNK_SIZE,
        temps: Vec::new(),
        last_activity: Instant::now(),
    })
}

pub(crate) fn entry_dto(entry: &Entry) -> EntryDto {
    EntryDto {
        id: entry.id.clone(),
        parent_id: entry.parent.clone(),
        kind: match entry.kind {
            EntryKind::File => "file",
            EntryKind::Folder => "folder",
        }
        .to_string(),
        name: entry.name.clone(),
        size: entry.size,
        created_ms: entry.created_ms,
        modified_ms: entry.modified_ms,
        favorite: entry.favorite,
        trashed: entry.trashed,
        comment: entry.comment.clone(),
    }
}

fn check_password_pair(password: &[u8], confirm: &[u8]) -> Result<()> {
    let text = std::str::from_utf8(password).map_err(|_| Error::Data)?;
    if text.chars().count() < 8 {
        return Err(Error::PasswordTooShort);
    }
    let same_len = password.len() == confirm.len();
    let equal = if same_len {
        bool::from(password.ct_eq(confirm))
    } else {
        false
    };
    if !equal {
        return Err(Error::PasswordMismatch);
    }
    Ok(())
}

fn prepare_directory(directory: &Path) -> Result<()> {
    if directory.exists() {
        if !is_blank_slot(directory) {
            return Err(Error::AlreadyExists);
        }
    } else {
        fs::create_dir_all(directory)?;
    }
    fs::create_dir_all(directory.join("objects"))?;
    Ok(())
}
