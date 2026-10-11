//! Catalogue en clair, uniquement manipulé en mémoire après déverrouillage.
//! Sur la clé, il est stocké chiffré dans `index.enc`.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroize;

use crate::error::{Error, Result};

pub const CATALOG_VERSION: u32 = 1;
pub const MAX_NAME_CHARS: usize = 255;
pub const MAX_VAULT_NAME_CHARS: usize = 80;

const RESERVED_WINDOWS: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

const DOCUMENT_EXT: &[&str] = &[
    "pdf", "doc", "docx", "txt", "md", "rtf", "odt", "xls", "xlsx", "ppt", "pptx", "csv", "json",
];

const IMAGE_EXT: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "heic", "tif", "tiff", "bmp", "svg",
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    File,
    Folder,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub parent: Option<String>,
    pub kind: EntryKind,
    pub name: String,
    pub object: Option<String>,
    pub size: u64,
    pub created_ms: u64,
    pub modified_ms: u64,
    pub favorite: bool,
    pub trashed: bool,
    pub comment: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub v: u32,
    pub name: String,
    pub auto_lock_secs: u32,
    pub entries: Vec<Entry>,
}

impl Drop for Catalog {
    fn drop(&mut self) {
        wipe_string(&mut self.name);
        for entry in &mut self.entries {
            wipe_string(&mut entry.name);
            wipe_string(&mut entry.comment);
        }
    }
}

impl Catalog {
    pub fn new(name: String) -> Self {
        Self {
            v: CATALOG_VERSION,
            name,
            auto_lock_secs: 300,
            entries: Vec::new(),
        }
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut bytes = serde_json::to_vec_pretty(self).map_err(|_| Error::Data)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let catalog: Self = serde_json::from_slice(bytes).map_err(|_| Error::Damaged)?;
        if catalog.v != CATALOG_VERSION {
            return Err(Error::UnsupportedFormat {
                found: u16::try_from(catalog.v).unwrap_or(u16::MAX),
            });
        }
        catalog.validate_structure()?;
        Ok(catalog)
    }

    pub fn validate_structure(&self) -> Result<()> {
        validate_vault_name(&self.name)?;
        let mut ids = Vec::new();
        for entry in &self.entries {
            if !ids.iter().any(|id| id == &entry.id) {
                ids.push(entry.id.clone());
            } else {
                return Err(Error::Damaged);
            }
            Uuid::parse_str(&entry.id).map_err(|_| Error::Damaged)?;
            validate_entry_name(&entry.name)?;
            match entry.kind {
                EntryKind::File => {
                    let object = entry.object.as_deref().ok_or(Error::Damaged)?;
                    if crate::storage::parse_object_name(object).is_none() {
                        return Err(Error::Damaged);
                    }
                }
                EntryKind::Folder => {
                    if entry.object.is_some() || entry.size != 0 {
                        return Err(Error::Damaged);
                    }
                }
            }
        }
        for entry in &self.entries {
            if let Some(parent) = &entry.parent {
                let parent_entry = self
                    .entries
                    .iter()
                    .find(|candidate| &candidate.id == parent)
                    .ok_or(Error::Damaged)?;
                if parent_entry.kind != EntryKind::Folder {
                    return Err(Error::Damaged);
                }
            }
            if self.creates_cycle(&entry.id) {
                return Err(Error::Damaged);
            }
        }
        Ok(())
    }

    pub fn entry(&self, id: &str) -> Result<&Entry> {
        self.entries
            .iter()
            .find(|entry| entry.id == id)
            .ok_or(Error::NotFound)
    }

    pub fn entry_mut(&mut self, id: &str) -> Result<&mut Entry> {
        self.entries
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or(Error::NotFound)
    }

    pub fn name_taken(&self, parent: Option<&str>, name: &str, except: Option<&str>) -> bool {
        self.entries.iter().any(|entry| {
            entry.parent.as_deref() == parent
                && entry.name == name
                && except != Some(entry.id.as_str())
                && !entry.trashed
        })
    }

    pub fn ensure_available_name(&self, parent: Option<&str>, name: &str) -> Result<()> {
        validate_entry_name(name)?;
        self.ensure_folder(parent)?;
        if self.name_taken(parent, name, None) {
            return Err(Error::NameConflict);
        }
        Ok(())
    }

    pub fn ensure_folder(&self, parent: Option<&str>) -> Result<()> {
        if let Some(parent_id) = parent {
            let parent_entry = self.entry(parent_id)?;
            if parent_entry.kind != EntryKind::Folder || parent_entry.trashed {
                return Err(Error::NotFound);
            }
        }
        Ok(())
    }

    pub fn add_folder(
        &mut self,
        parent: Option<String>,
        name: String,
        now_ms: u64,
    ) -> Result<Entry> {
        self.ensure_available_name(parent.as_deref(), &name)?;
        let entry = Entry {
            id: Uuid::new_v4().to_string(),
            parent,
            kind: EntryKind::Folder,
            name,
            object: None,
            size: 0,
            created_ms: now_ms,
            modified_ms: now_ms,
            favorite: false,
            trashed: false,
            comment: String::new(),
        };
        self.entries.push(entry.clone());
        Ok(entry)
    }

    pub fn add_file(
        &mut self,
        parent: Option<String>,
        name: String,
        object: String,
        size: u64,
        now_ms: u64,
    ) -> Result<Entry> {
        self.ensure_available_name(parent.as_deref(), &name)?;
        let entry = Entry {
            id: Uuid::new_v4().to_string(),
            parent,
            kind: EntryKind::File,
            name,
            object: Some(object),
            size,
            created_ms: now_ms,
            modified_ms: now_ms,
            favorite: false,
            trashed: false,
            comment: String::new(),
        };
        self.entries.push(entry.clone());
        Ok(entry)
    }

    pub fn rename(&mut self, id: &str, name: String, now_ms: u64) -> Result<()> {
        validate_entry_name(&name)?;
        let parent = self.entry(id)?.parent.clone();
        if self.name_taken(parent.as_deref(), &name, Some(id)) {
            return Err(Error::NameConflict);
        }
        let entry = self.entry_mut(id)?;
        entry.name = name;
        entry.modified_ms = now_ms;
        Ok(())
    }

    pub fn set_comment(&mut self, id: &str, comment: String) -> Result<()> {
        if comment.chars().count() > 2000 {
            return Err(Error::InvalidName);
        }
        if comment.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
            return Err(Error::InvalidName);
        }
        self.entry_mut(id)?.comment = comment;
        Ok(())
    }

    pub fn set_favorite(&mut self, id: &str, favorite: bool) -> Result<()> {
        self.entry_mut(id)?.favorite = favorite;
        Ok(())
    }

    pub fn move_entry(&mut self, id: &str, new_parent: Option<String>, now_ms: u64) -> Result<()> {
        self.ensure_folder(new_parent.as_deref())?;
        if new_parent.as_deref() == Some(id) {
            return Err(Error::InvalidName);
        }
        if let Some(parent_id) = new_parent.as_deref() {
            if self.is_descendant(id, parent_id) {
                return Err(Error::InvalidName);
            }
        }
        let name = self.entry(id)?.name.clone();
        if self.name_taken(new_parent.as_deref(), &name, Some(id)) {
            return Err(Error::NameConflict);
        }
        let entry = self.entry_mut(id)?;
        entry.parent = new_parent;
        entry.modified_ms = now_ms;
        Ok(())
    }

    pub fn trash(&mut self, id: &str, now_ms: u64) -> Result<()> {
        let entry = self.entry_mut(id)?;
        entry.trashed = true;
        entry.modified_ms = now_ms;
        Ok(())
    }

    pub fn restore(&mut self, id: &str, now_ms: u64) -> Result<()> {
        let parent = self.entry(id)?.parent.clone();
        let name = self.entry(id)?.name.clone();
        if self.name_taken(parent.as_deref(), &name, Some(id)) {
            return Err(Error::NameConflict);
        }
        let entry = self.entry_mut(id)?;
        entry.trashed = false;
        entry.modified_ms = now_ms;
        Ok(())
    }

    /// Retire des entrées et renvoie les noms d'objets à supprimer ensuite.
    pub fn delete_permanent(&mut self, id: &str) -> Result<Vec<String>> {
        let mut remove_ids = vec![id.to_string()];
        let mut cursor = 0;
        while cursor < remove_ids.len() {
            let current = remove_ids[cursor].clone();
            for entry in &self.entries {
                if entry.parent.as_deref() == Some(current.as_str()) {
                    remove_ids.push(entry.id.clone());
                }
            }
            cursor += 1;
        }
        let mut objects = Vec::new();
        self.entries.retain(|entry| {
            if remove_ids.iter().any(|candidate| candidate == &entry.id) {
                if let Some(object) = &entry.object {
                    objects.push(object.clone());
                }
                false
            } else {
                true
            }
        });
        Ok(objects)
    }

    pub fn hidden(&self, entry: &Entry) -> bool {
        if entry.trashed {
            return true;
        }
        let mut parent = entry.parent.as_deref();
        while let Some(parent_id) = parent {
            let Ok(parent_entry) = self.entry(parent_id) else {
                return true;
            };
            if parent_entry.trashed {
                return true;
            }
            parent = parent_entry.parent.as_deref();
        }
        false
    }

    pub fn children(&self, parent: Option<&str>) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.parent.as_deref() == parent && !self.hidden(entry))
            .collect()
    }

    fn creates_cycle(&self, id: &str) -> bool {
        let mut seen = Vec::new();
        let mut current = Some(id);
        while let Some(next) = current {
            if seen.iter().any(|item| item == &next) {
                return true;
            }
            seen.push(next);
            current = self
                .entries
                .iter()
                .find(|entry| entry.id == next)
                .and_then(|entry| entry.parent.as_deref());
        }
        false
    }

    fn is_descendant(&self, ancestor: &str, maybe_child: &str) -> bool {
        let mut current = Some(maybe_child);
        while let Some(next) = current {
            if next == ancestor {
                return true;
            }
            current = self
                .entries
                .iter()
                .find(|entry| entry.id == next)
                .and_then(|entry| entry.parent.as_deref());
        }
        false
    }
}

pub fn validate_vault_name(name: &str) -> Result<()> {
    let count = name.chars().count();
    if count == 0 || count > MAX_VAULT_NAME_CHARS {
        return Err(Error::InvalidName);
    }
    if name.chars().any(|c| c.is_control()) {
        return Err(Error::InvalidName);
    }
    Ok(())
}

pub fn validate_entry_name(name: &str) -> Result<()> {
    let count = name.chars().count();
    if count == 0 || count > MAX_NAME_CHARS {
        return Err(Error::InvalidName);
    }
    if name != name.trim() || name.ends_with('.') {
        return Err(Error::InvalidName);
    }
    if name.chars().any(|c| {
        c.is_control() || matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
    }) {
        return Err(Error::InvalidName);
    }
    let upper = name.to_ascii_uppercase();
    let stem = upper.split('.').next().unwrap_or(upper.as_str());
    if RESERVED_WINDOWS.contains(&stem) {
        return Err(Error::InvalidName);
    }
    Ok(())
}

pub fn extension_of(name: &str) -> Option<&str> {
    let dot = name.rfind('.')?;
    if dot == 0 || dot + 1 == name.len() {
        return None;
    }
    Some(&name[dot + 1..])
}

pub fn is_document(name: &str) -> bool {
    extension_of(name)
        .map(|ext| DOCUMENT_EXT.iter().any(|candidate| candidate.eq_ignore_ascii_case(ext)))
        .unwrap_or(false)
}

pub fn is_image(name: &str) -> bool {
    extension_of(name)
        .map(|ext| IMAGE_EXT.iter().any(|candidate| candidate.eq_ignore_ascii_case(ext)))
        .unwrap_or(false)
}

pub fn file_name_from_path(path: &std::path::Path) -> Result<String> {
    let raw = path.file_name().ok_or(Error::InvalidName)?;
    let name = raw.to_str().ok_or(Error::InvalidName)?;
    validate_entry_name(name)?;
    Ok(name.to_string())
}

fn wipe_string(value: &mut String) {
    let mut bytes = std::mem::take(value).into_bytes();
    bytes.zeroize();
}
