use std::fs::File;
use std::io::Cursor;

use zeroize::Zeroize;

use crate::crypto::{index_aad, open_from_file, seal_to_writer, NopProgress, INDEX_CHUNK_SIZE};
use crate::error::Result;
use crate::metadata::Catalog;
use crate::platform::diag;
use crate::storage::write_atomic;

use super::VaultSession;

impl VaultSession {
    pub(crate) fn commit_catalog(&mut self) -> Result<()> {
        if self.read_only {
            return Err(crate::error::Error::ReadOnly);
        }
        let mut plain = self.catalog.to_bytes()?;
        let mut sealed = Vec::new();
        let mut cursor = Cursor::new(plain.as_slice());
        seal_to_writer(
            &self.master,
            &index_aad(&self.vault_id),
            &mut cursor,
            &mut sealed,
            INDEX_CHUNK_SIZE,
            Some(u64::try_from(plain.len()).unwrap_or(u64::MAX)),
            &mut NopProgress,
        )?;
        plain.zeroize();
        if self.paths.index().exists() {
            let previous = std::fs::read(self.paths.index())?;
            write_atomic(&self.paths.index_backup(), &previous)?;
        }
        write_atomic(&self.paths.index(), &sealed)?;
        sealed.zeroize();
        diag("index.commit");
        Ok(())
    }

    pub(crate) fn mutate<T>(
        &mut self,
        body: impl FnOnce(&mut Catalog) -> Result<T>,
    ) -> Result<T> {
        self.ensure_writable()?;
        let snapshot = self.catalog.clone();
        match body(&mut self.catalog) {
            Ok(value) => match self.commit_catalog() {
                Ok(()) => Ok(value),
                Err(error) => {
                    self.catalog = snapshot;
                    Err(error)
                }
            },
            Err(error) => {
                self.catalog = snapshot;
                Err(error)
            }
        }
    }
}

pub(crate) fn read_catalog(
    master: &crate::crypto::MasterKey,
    vault_id: &[u8; 16],
    path: &std::path::Path,
) -> Result<Catalog> {
    let mut file = File::open(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            crate::error::Error::Damaged
        } else {
            crate::error::Error::Io(error)
        }
    })?;
    let mut plain = Vec::new();
    open_from_file(master, &index_aad(vault_id), &mut file, &mut plain, &mut NopProgress).map_err(
        |error| match error {
            crate::error::Error::AuthFailed => crate::error::Error::Damaged,
            other => other,
        },
    )?;
    let catalog = Catalog::from_bytes(&plain);
    plain.zeroize();
    if catalog.is_err() {
        diag("index.rejected");
    }
    catalog
}
