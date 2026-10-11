use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::crypto::{Kek, MasterKey, WrappedKey};
use crate::crypto::{derive_kek, KdfParams, SALT_LEN};
use crate::error::{Error, Result};
use crate::platform::diag;
use crate::storage::write_atomic;

pub const FORMAT_VERSION: u16 = 1;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VaultMeta {
    pub format_version: u16,
    pub vault_id: String,
    pub created_at_ms: u64,
    pub kdf: KdfRecord,
    pub master_key: WrappedRecord,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KdfRecord {
    pub algorithm: String,
    pub version: String,
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub salt_hex: String,
    pub output_len: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WrappedRecord {
    pub algorithm: String,
    pub nonce_hex: String,
    pub ciphertext_hex: String,
}

impl VaultMeta {
    pub fn vault_id_bytes(&self) -> Result<[u8; 16]> {
        let id = Uuid::parse_str(&self.vault_id).map_err(|_| Error::Damaged)?;
        Ok(*id.as_bytes())
    }

    pub fn kdf_params(&self) -> Result<KdfParams> {
        if self.kdf.algorithm != "argon2id" || self.kdf.version != "0x13" || self.kdf.output_len != 32
        {
            return Err(Error::Damaged);
        }
        Ok(KdfParams {
            memory_kib: self.kdf.memory_kib,
            iterations: self.kdf.iterations,
            parallelism: self.kdf.parallelism,
        })
    }

    pub fn salt(&self) -> Result<[u8; SALT_LEN]> {
        let bytes = hex::decode(&self.kdf.salt_hex).map_err(|_| Error::Damaged)?;
        bytes.try_into().map_err(|_| Error::Damaged)
    }

    pub fn wrapped(&self) -> Result<([u8; 24], Vec<u8>)> {
        if self.master_key.algorithm != "xchacha20poly1305" {
            return Err(Error::Damaged);
        }
        let nonce = hex::decode(&self.master_key.nonce_hex).map_err(|_| Error::Damaged)?;
        let nonce: [u8; 24] = nonce.try_into().map_err(|_| Error::Damaged)?;
        let ciphertext = hex::decode(&self.master_key.ciphertext_hex).map_err(|_| Error::Damaged)?;
        if ciphertext.len() != 48 {
            return Err(Error::Damaged);
        }
        Ok((nonce, ciphertext))
    }
}

pub fn load_meta(path: &Path) -> Result<VaultMeta> {
    let bytes = fs::read(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::NotAVault
        } else {
            Error::Io(error)
        }
    })?;
    let meta: VaultMeta = serde_json::from_slice(&bytes).map_err(|_| Error::Damaged)?;
    if meta.format_version != FORMAT_VERSION {
        diag("vault.meta.unsupported");
        return Err(Error::UnsupportedFormat {
            found: meta.format_version,
        });
    }
    let _ = meta.vault_id_bytes()?;
    let _ = meta.kdf_params()?;
    let _ = meta.salt()?;
    let _ = meta.wrapped()?;
    Ok(meta)
}

pub fn save_meta(path: &Path, meta: &VaultMeta) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(meta).map_err(|_| Error::Data)?;
    bytes.push(b'\n');
    write_atomic(path, &bytes)?;
    diag("vault.meta.write");
    Ok(())
}

pub fn build_meta(
    vault_id: Uuid,
    created_at_ms: u64,
    params: &KdfParams,
    salt: &[u8; SALT_LEN],
    wrapped: &WrappedKey,
) -> VaultMeta {
    VaultMeta {
        format_version: FORMAT_VERSION,
        vault_id: vault_id.to_string(),
        created_at_ms,
        kdf: KdfRecord {
            algorithm: "argon2id".to_string(),
            version: "0x13".to_string(),
            memory_kib: params.memory_kib,
            iterations: params.iterations,
            parallelism: params.parallelism,
            salt_hex: hex::encode(salt),
            output_len: 32,
        },
        master_key: WrappedRecord {
            algorithm: "xchacha20poly1305".to_string(),
            nonce_hex: hex::encode(wrapped.nonce),
            ciphertext_hex: hex::encode(&wrapped.ciphertext),
        },
    }
}

pub fn unlock_master(meta: &VaultMeta, password: &[u8]) -> Result<MasterKey> {
    let kek = derive_kek(password, &meta.salt()?, &meta.kdf_params()?)?;
    let (nonce, ciphertext) = meta.wrapped()?;
    let master = crate::crypto::unwrap_master_key(
        &kek,
        &nonce,
        &ciphertext,
        &meta.vault_id_bytes()?,
        meta.format_version,
    );
    drop(kek);
    master
}

pub fn rewrap(meta: &mut VaultMeta, kek: &Kek, master: &MasterKey) -> Result<()> {
    let vault_id = meta.vault_id_bytes()?;
    let wrapped = crate::crypto::wrap_master_key(kek, master, &vault_id, meta.format_version)?;
    meta.master_key.nonce_hex = hex::encode(wrapped.nonce);
    meta.master_key.ciphertext_hex = hex::encode(wrapped.ciphertext);
    Ok(())
}
