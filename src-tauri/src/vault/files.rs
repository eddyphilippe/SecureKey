use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::crypto::{object_aad, open_from_file, random_bytes, seal_to_writer, Progress};
use crate::error::{Error, Result};
use crate::metadata::{file_name_from_path, EntryKind};
use crate::platform::{diag, mtime_ms, now_ms, open_with_associated_app, secure_temp_dir, wipe_tree};
use crate::storage::{object_name, replace_file, temporary_sibling};

use super::{entry_dto, EntryDto, VaultSession};

pub struct ImportReport {
    pub entries: Vec<EntryDto>,
    pub skipped: u32,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TempStatus {
    pub entry_id: String,
    pub name: String,
    pub changed: bool,
}

impl VaultSession {
    pub fn import_file(
        &mut self,
        source: &Path,
        parent: Option<&str>,
        progress: &mut dyn Progress,
    ) -> Result<EntryDto> {
        self.ensure_writable()?;
        let metadata = fs::symlink_metadata(source)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(Error::Data);
        }
        let name = file_name_from_path(source)?;
        self.import_reader(source, &name, metadata.len(), parent, progress)
    }

    pub fn import_tree(
        &mut self,
        source: &Path,
        parent: Option<&str>,
        progress: &mut dyn Progress,
    ) -> Result<ImportReport> {
        self.ensure_writable()?;
        let mut report = ImportReport {
            entries: Vec::new(),
            skipped: 0,
        };
        self.import_tree_inner(source, parent, true, progress, &mut report)?;
        Ok(report)
    }

    pub fn create_folder(&mut self, parent: Option<&str>, name: &str) -> Result<EntryDto> {
        let parent = parent.map(str::to_string);
        let name = name.to_string();
        self.mutate(|catalog| {
            let entry = catalog.add_folder(parent, name, now_ms())?;
            Ok(entry_dto(&entry))
        })
    }

    pub fn rename_entry(&mut self, id: &str, name: &str) -> Result<EntryDto> {
        let name = name.to_string();
        let id = id.to_string();
        self.mutate(move |catalog| {
            catalog.rename(&id, name, now_ms())?;
            Ok(entry_dto(catalog.entry(&id)?))
        })
    }

    pub fn set_comment(&mut self, id: &str, comment: &str) -> Result<EntryDto> {
        let id = id.to_string();
        let comment = comment.to_string();
        self.mutate(move |catalog| {
            catalog.set_comment(&id, comment)?;
            Ok(entry_dto(catalog.entry(&id)?))
        })
    }

    pub fn set_favorite(&mut self, id: &str, favorite: bool) -> Result<EntryDto> {
        let id = id.to_string();
        self.mutate(move |catalog| {
            catalog.set_favorite(&id, favorite)?;
            Ok(entry_dto(catalog.entry(&id)?))
        })
    }

    pub fn move_entry(&mut self, id: &str, parent: Option<&str>) -> Result<EntryDto> {
        let id = id.to_string();
        let parent = parent.map(str::to_string);
        self.mutate(move |catalog| {
            catalog.move_entry(&id, parent, now_ms())?;
            Ok(entry_dto(catalog.entry(&id)?))
        })
    }

    pub fn trash_entries(&mut self, ids: &[String]) -> Result<()> {
        let ids = ids.to_vec();
        self.mutate(move |catalog| {
            let now = now_ms();
            for id in &ids {
                catalog.trash(id, now)?;
            }
            Ok(())
        })
    }

    pub fn restore_entries(&mut self, ids: &[String]) -> Result<()> {
        let ids = ids.to_vec();
        self.mutate(move |catalog| {
            let now = now_ms();
            for id in &ids {
                catalog.restore(id, now)?;
            }
            Ok(())
        })
    }

    pub fn delete_permanent(&mut self, ids: &[String]) -> Result<()> {
        self.ensure_writable()?;
        let ids = ids.to_vec();
        let objects = self.mutate(|catalog| {
            let mut objects = Vec::new();
            for id in &ids {
                if catalog.entries.iter().any(|entry| &entry.id == id) {
                    objects.extend(catalog.delete_permanent(id)?);
                }
            }
            Ok(objects)
        })?;
        for object in objects {
            let path = self.paths.object(&object);
            if path.exists() {
                let _ = fs::remove_file(path);
            }
        }
        diag("entry.delete");
        Ok(())
    }

    pub fn export_entries(
        &self,
        ids: &[String],
        destination: &Path,
        progress: &mut dyn Progress,
    ) -> Result<u32> {
        if !destination.is_dir() {
            return Err(Error::Data);
        }
        let mut count = 0u32;
        for id in ids {
            count += self.export_one(id, destination, progress)?;
        }
        Ok(count)
    }

    pub fn prepare_temp(&mut self, id: &str, progress: &mut dyn Progress) -> Result<PathBuf> {
        let entry = self.catalog.entry(id)?.clone();
        if entry.kind != EntryKind::File {
            return Err(Error::Data);
        }
        let object = entry.object.as_deref().ok_or(Error::Damaged)?;
        let object_id = crate::storage::parse_object_name(object).ok_or(Error::Damaged)?;
        let dir = secure_temp_dir(&hex::encode(&self.vault_id[..4]))?;
        let destination = dir.join(&entry.name);
        let tmp = temporary_sibling(&destination)?;
        let copy = self.copy_object_to(&object_id, &self.paths.object(object), &tmp, progress);
        if let Err(error) = copy {
            let _ = fs::remove_file(&tmp);
            wipe_tree(&dir);
            return Err(error);
        }
        replace_file(&tmp, &destination)?;
        let len = fs::metadata(&destination)?.len();
        let modified = mtime_ms(&destination)?;
        self.temps.push(super::OpenTemp {
            entry_id: entry.id,
            path: destination.clone(),
            dir,
            len,
            modified_ms: modified,
        });
        diag("temp.create");
        Ok(destination)
    }

    pub fn launch_temp(&self, id: &str) -> Result<()> {
        let temp = self
            .temps
            .iter()
            .find(|item| item.entry_id == id)
            .ok_or(Error::NotFound)?;
        open_with_associated_app(&temp.path)
    }

    pub fn temp_status(&self) -> Result<Vec<TempStatus>> {
        let mut statuses = Vec::new();
        for temp in &self.temps {
            let name = self.catalog.entry(&temp.entry_id)?.name.clone();
            let changed = match fs::metadata(&temp.path) {
                Ok(metadata) => {
                    metadata.len() != temp.len
                        || mtime_ms(&temp.path).unwrap_or(temp.modified_ms) != temp.modified_ms
                }
                Err(_) => true,
            };
            statuses.push(TempStatus {
                entry_id: temp.entry_id.clone(),
                name,
                changed,
            });
        }
        Ok(statuses)
    }

    pub fn reimport_temp(&mut self, id: &str, progress: &mut dyn Progress) -> Result<EntryDto> {
        self.ensure_writable()?;
        let temp = self
            .temps
            .iter()
            .find(|item| item.entry_id == id)
            .ok_or(Error::NotFound)?;
        let source = temp.path.clone();
        let len = fs::metadata(&source)?.len();
        let entry = self.catalog.entry(id)?.clone();
        let old_object = entry.object.clone().ok_or(Error::Damaged)?;
        let object_id: [u8; 16] = random_bytes()?;
        let new_object = object_name(&object_id);
        let final_path = self.paths.object(&new_object);
        let tmp = temporary_sibling(&final_path)?;
        let sealed = self.seal_path(&source, len, &object_id, &tmp, progress);
        if let Err(error) = sealed {
            let _ = fs::remove_file(&tmp);
            return Err(error);
        }
        if let Err(error) = replace_file(&tmp, &final_path) {
            let _ = fs::remove_file(&tmp);
            return Err(error.into());
        }
        let id = id.to_string();
        let dto = match self.mutate(|catalog| {
            let entry = catalog.entry_mut(&id)?;
            entry.object = Some(new_object);
            entry.size = len;
            entry.modified_ms = now_ms();
            Ok(entry_dto(entry))
        }) {
            Ok(dto) => dto,
            Err(error) => {
                let _ = fs::remove_file(&final_path);
                return Err(error);
            }
        };
        let _ = fs::remove_file(self.paths.object(&old_object));
        self.discard_temp(&id);
        diag("temp.reimport");
        Ok(dto)
    }

    pub fn discard_temp(&mut self, id: &str) {
        if let Some(index) = self.temps.iter().position(|item| item.entry_id == id) {
            let temp = self.temps.remove(index);
            wipe_tree(&temp.dir);
            diag("temp.wipe");
        }
    }

    pub fn discard_temps(&mut self) {
        let temps = std::mem::take(&mut self.temps);
        let had_temps = !temps.is_empty();
        for temp in temps {
            wipe_tree(&temp.dir);
        }
        if had_temps {
            diag("temp.wipe");
        }
    }

    fn import_reader(
        &mut self,
        source: &Path,
        name: &str,
        len: u64,
        parent: Option<&str>,
        progress: &mut dyn Progress,
    ) -> Result<EntryDto> {
        self.catalog.ensure_available_name(parent, name)?;
        let object_id: [u8; 16] = random_bytes()?;
        let object = object_name(&object_id);
        fs::create_dir_all(self.paths.objects_dir())?;
        let final_path = self.paths.object(&object);
        let tmp = temporary_sibling(&final_path)?;
        if let Err(error) = self.seal_path(source, len, &object_id, &tmp, progress) {
            let _ = fs::remove_file(&tmp);
            return Err(error);
        }
        if let Err(error) = replace_file(&tmp, &final_path) {
            let _ = fs::remove_file(&tmp);
            return Err(error.into());
        }
        let parent = parent.map(str::to_string);
        let name = name.to_string();
        let object_for_catalog = object.clone();
        let added = self.mutate(|catalog| {
            let entry = catalog.add_file(parent, name, object_for_catalog, len, now_ms())?;
            Ok(entry_dto(&entry))
        });
        if added.is_err() {
            let _ = fs::remove_file(&final_path);
        } else {
            diag("object.write");
        }
        added
    }

    fn seal_path(
        &self,
        source: &Path,
        len: u64,
        object_id: &[u8; 16],
        tmp: &Path,
        progress: &mut dyn Progress,
    ) -> Result<()> {
        if let Some(parent) = tmp.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut input = File::open(source)?;
        let output = File::create(tmp)?;
        let mut writer = BufWriter::new(output);
        seal_to_writer(
            &self.master,
            &object_aad(&self.vault_id, object_id),
            &mut input,
            &mut writer,
            self.write_chunk_size,
            Some(len),
            progress,
        )?;
        writer.flush()?;
        let file = writer
            .into_inner()
            .map_err(|error| Error::Io(error.into_error()))?;
        file.sync_all()?;
        if let Some(parent) = tmp.parent() {
            let _ = crate::storage::fsync_dir(parent);
        }
        Ok(())
    }

    fn copy_object_to(
        &self,
        object_id: &[u8; 16],
        source: &Path,
        tmp: &Path,
        progress: &mut dyn Progress,
    ) -> Result<()> {
        if let Some(parent) = tmp.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut input = File::open(source)?;
        let output = File::create(tmp)?;
        let mut writer = BufWriter::new(output);
        open_from_file(
            &self.master,
            &object_aad(&self.vault_id, object_id),
            &mut input,
            &mut writer,
            progress,
        )?;
        writer.flush()?;
        let file = writer
            .into_inner()
            .map_err(|error| Error::Io(error.into_error()))?;
        file.sync_all()?;
        Ok(())
    }

    fn export_one(&self, id: &str, destination: &Path, progress: &mut dyn Progress) -> Result<u32> {
        let entry = self.catalog.entry(id)?.clone();
        match entry.kind {
            EntryKind::Folder => {
                let folder = destination.join(&entry.name);
                if folder.exists() {
                    return Err(Error::NameConflict);
                }
                fs::create_dir(&folder)?;
                let mut count = 0;
                let children: Vec<String> = self
                    .catalog
                    .entries
                    .iter()
                    .filter(|child| child.parent.as_deref() == Some(entry.id.as_str()) && !child.trashed)
                    .map(|child| child.id.clone())
                    .collect();
                for child in children {
                    count += self.export_one(&child, &folder, progress)?;
                }
                Ok(count)
            }
            EntryKind::File => {
                let object = entry.object.as_deref().ok_or(Error::Damaged)?;
                let object_id = crate::storage::parse_object_name(object).ok_or(Error::Damaged)?;
                let final_path = destination.join(&entry.name);
                if final_path.exists() {
                    return Err(Error::NameConflict);
                }
                let tmp = temporary_sibling(&final_path)?;
                if let Err(error) =
                    self.copy_object_to(&object_id, &self.paths.object(object), &tmp, progress)
                {
                    let _ = fs::remove_file(&tmp);
                    return Err(error);
                }
                if let Err(error) = replace_file(&tmp, &final_path) {
                    let _ = fs::remove_file(&tmp);
                    return Err(error.into());
                }
                diag("object.export");
                Ok(1)
            }
        }
    }

    fn import_tree_inner(
        &mut self,
        source: &Path,
        parent: Option<&str>,
        as_root: bool,
        progress: &mut dyn Progress,
        report: &mut ImportReport,
    ) -> Result<()> {
        if progress.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let metadata = fs::symlink_metadata(source)?;
        if metadata.file_type().is_symlink() {
            report.skipped += 1;
            return Ok(());
        }
        if metadata.is_file() {
            match self.import_file(source, parent, progress) {
                Ok(entry) => report.entries.push(entry),
                Err(Error::Cancelled) => return Err(Error::Cancelled),
                Err(_) => report.skipped += 1,
            }
            return Ok(());
        }
        if !metadata.is_dir() {
            report.skipped += 1;
            return Ok(());
        }
        let folder_id = if as_root {
            let name = file_name_from_path(source)?;
            let created = self.create_folder(parent, &name)?;
            report.entries.push(created.clone());
            created.id
        } else {
            parent.unwrap_or("").to_string()
        };
        let mut children = Vec::new();
        for entry in fs::read_dir(source)? {
            children.push(entry?.path());
        }
        children.sort();
        for child in children {
            self.import_tree_inner(&child, Some(&folder_id), true, progress, report)?;
        }
        Ok(())
    }
}
