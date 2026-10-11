//! Chiffrement de fichiers par la construction STREAM (BE32).
//!
//! L'algorithme authentifié est XChaCha20-Poly1305 (RustCrypto). Le découpage
//! en blocs est celui de `aead-stream`, décrit dans
//! « Online Authenticated-Encryption and its Nonce-Reuse Misuse-Resistance »
//! <https://eprint.iacr.org/2015/189>.
//!
//! Chaque fichier reçoit un préfixe de nonce aléatoire de 19 octets.
//! `aead-stream` y ajoute un compteur 32 bits big-endian et un octet
//! « dernier bloc ». Un nonce complet de 24 octets n'est donc jamais réutilisé
//! pour cette clé, y compris d'un bloc à l'autre.

use std::fs::File;
use std::io::{Read, Write};

use aead_stream::{DecryptorBE32, EncryptorBE32};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305};
use zeroize::Zeroize;

use super::keys::{random_bytes, MasterKey, KEY_LEN};
use crate::error::{Error, Result};

pub const OBJECT_LABEL: &[u8] = b"securekey.v1.object";
pub const INDEX_LABEL: &[u8] = b"securekey.v1.index";

pub const MAGIC: &[u8; 4] = b"SKO1";
pub const HEADER_VERSION: u16 = 1;
pub const STREAM_NONCE_LEN: usize = 19;
pub const TAG_LEN: usize = 16;
pub const HEADER_LEN: usize = 4 + 2 + 4 + 1 + STREAM_NONCE_LEN;
pub const DEFAULT_CHUNK_SIZE: u32 = 1024 * 1024;
pub const INDEX_CHUNK_SIZE: u32 = 64 * 1024;
pub const MIN_CHUNK_SIZE: u32 = 1024;
pub const MAX_CHUNK_SIZE: u32 = 16 * 1024 * 1024;

pub trait Progress {
    fn report(&mut self, done: u64, total: Option<u64>);
    fn is_cancelled(&self) -> bool;
}

pub struct NopProgress;

impl Progress for NopProgress {
    fn report(&mut self, _done: u64, _total: Option<u64>) {}
    fn is_cancelled(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    pub version: u16,
    pub chunk_size: u32,
    pub nonce: [u8; STREAM_NONCE_LEN],
}

type FileStream = aead_stream::StreamBE32<XChaCha20Poly1305>;
type FileNonce = aead_stream::Nonce<XChaCha20Poly1305, FileStream>;

pub fn object_aad(vault_id: &[u8; 16], object_id: &[u8; 16]) -> Vec<u8> {
    let mut aad = Vec::with_capacity(OBJECT_LABEL.len() + 32);
    aad.extend_from_slice(OBJECT_LABEL);
    aad.extend_from_slice(vault_id);
    aad.extend_from_slice(object_id);
    aad
}

pub fn index_aad(vault_id: &[u8; 16]) -> Vec<u8> {
    let mut aad = Vec::with_capacity(INDEX_LABEL.len() + 16);
    aad.extend_from_slice(INDEX_LABEL);
    aad.extend_from_slice(vault_id);
    aad
}

pub fn seal_to_writer(
    key: &MasterKey,
    aad: &[u8],
    reader: &mut dyn Read,
    writer: &mut dyn Write,
    chunk_size: u32,
    total: Option<u64>,
    progress: &mut dyn Progress,
) -> Result<u64> {
    if !(MIN_CHUNK_SIZE..=MAX_CHUNK_SIZE).contains(&chunk_size) {
        return Err(Error::Data);
    }
    let nonce_bytes: [u8; STREAM_NONCE_LEN] = random_bytes()?;
    write_header(
        writer,
        &Header {
            version: HEADER_VERSION,
            chunk_size,
            nonce: nonce_bytes,
        },
    )?;

    let mut encryptor = Some(EncryptorBE32::from_aead(
        cipher(key)?,
        &stream_nonce(&nonce_bytes)?,
    ));
    let chunk = usize::try_from(chunk_size).map_err(|_| Error::Data)?;
    let mut pending = read_up_to(reader, chunk)?;
    if pending.is_empty() {
        let mut buffer = Vec::new();
        let enc = encryptor.take().ok_or(Error::Data)?;
        enc.encrypt_last_in_place(aad, &mut buffer)
            .map_err(|_| Error::Data)?;
        writer.write_all(&buffer)?;
        buffer.zeroize();
        progress.report(0, total);
        return Ok(0);
    }

    let mut done = 0u64;
    loop {
        if progress.is_cancelled() {
            pending.zeroize();
            return Err(Error::Cancelled);
        }
        let mut next = read_up_to(reader, chunk)?;
        let plain_len = pending.len() as u64;
        if next.is_empty() {
            let enc = encryptor.take().ok_or(Error::Data)?;
            enc.encrypt_last_in_place(aad, &mut pending)
                .map_err(|_| Error::Data)?;
            writer.write_all(&pending)?;
            done += plain_len;
            pending.zeroize();
            progress.report(done, total);
            break;
        }
        {
            let enc = encryptor.as_mut().ok_or(Error::Data)?;
            enc.encrypt_next_in_place(aad, &mut pending)
                .map_err(|_| Error::Data)?;
        }
        writer.write_all(&pending)?;
        done += plain_len;
        pending.zeroize();
        progress.report(done, total);
        if progress.is_cancelled() {
            next.zeroize();
            return Err(Error::Cancelled);
        }
        pending = next;
    }
    Ok(done)
}

pub fn open_from_file(
    key: &MasterKey,
    aad: &[u8],
    file: &mut File,
    writer: &mut dyn Write,
    progress: &mut dyn Progress,
) -> Result<u64> {
    let len = file.metadata()?.len();
    let header = read_header(file)?;
    if header.version != HEADER_VERSION {
        return Err(Error::UnsupportedFormat {
            found: header.version,
        });
    }
    if !(MIN_CHUNK_SIZE..=MAX_CHUNK_SIZE).contains(&header.chunk_size) {
        return Err(Error::Damaged);
    }

    let mut decryptor = Some(DecryptorBE32::from_aead(
        cipher(key)?,
        &stream_nonce(&header.nonce)?,
    ));
    let mut pos = u64::try_from(HEADER_LEN).map_err(|_| Error::Data)?;
    if pos >= len {
        return Err(Error::Damaged);
    }
    let full = u64::from(header.chunk_size) + TAG_LEN as u64;
    let mut done = 0u64;

    while pos < len {
        if progress.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let remaining = len - pos;
        let last = remaining <= full;
        let take = if last { remaining } else { full };
        if take < TAG_LEN as u64 {
            return Err(Error::Damaged);
        }
        let take_usize = usize::try_from(take).map_err(|_| Error::Damaged)?;
        let mut buffer = vec![0u8; take_usize];
        file.read_exact(&mut buffer)?;
        pos += take;

        let decrypted = if last {
            let dec = decryptor.take().ok_or(Error::Damaged)?;
            dec.decrypt_last_in_place(aad, &mut buffer)
        } else {
            let dec = decryptor.as_mut().ok_or(Error::Damaged)?;
            dec.decrypt_next_in_place(aad, &mut buffer)
        };
        if decrypted.is_err() {
            buffer.zeroize();
            return Err(Error::AuthFailed);
        }
        writer.write_all(&buffer)?;
        done += buffer.len() as u64;
        buffer.zeroize();
        progress.report(done, None);
        if last {
            break;
        }
    }
    if pos != len {
        return Err(Error::Damaged);
    }
    Ok(done)
}

pub fn write_header(writer: &mut dyn Write, header: &Header) -> Result<()> {
    writer.write_all(MAGIC)?;
    writer.write_all(&header.version.to_le_bytes())?;
    writer.write_all(&header.chunk_size.to_le_bytes())?;
    writer.write_all(&[STREAM_NONCE_LEN as u8])?;
    writer.write_all(&header.nonce)?;
    Ok(())
}

pub fn read_header(file: &mut File) -> Result<Header> {
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(Error::Damaged);
    }
    let mut version_bytes = [0u8; 2];
    file.read_exact(&mut version_bytes)?;
    let version = u16::from_le_bytes(version_bytes);
    let mut chunk_bytes = [0u8; 4];
    file.read_exact(&mut chunk_bytes)?;
    let mut nonce_len = [0u8; 1];
    file.read_exact(&mut nonce_len)?;
    if nonce_len[0] as usize != STREAM_NONCE_LEN {
        return Err(Error::Damaged);
    }
    let mut nonce = [0u8; STREAM_NONCE_LEN];
    file.read_exact(&mut nonce)?;
    Ok(Header {
        version,
        chunk_size: u32::from_le_bytes(chunk_bytes),
        nonce,
    })
}

fn cipher(key: &MasterKey) -> Result<XChaCha20Poly1305> {
    let aead_key =
        aead::Key::<XChaCha20Poly1305>::try_from(key.as_bytes().as_slice()).map_err(|_| Error::Data)?;
    debug_assert_eq!(aead_key.len(), KEY_LEN);
    Ok(XChaCha20Poly1305::new(&aead_key))
}

fn stream_nonce(bytes: &[u8; STREAM_NONCE_LEN]) -> Result<FileNonce> {
    FileNonce::try_from(bytes.as_slice()).map_err(|_| Error::Data)
}

fn read_up_to(reader: &mut dyn Read, chunk: usize) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; chunk];
    let mut filled = 0;
    while filled < chunk {
        match reader.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        }
    }
    buf.truncate(filled);
    Ok(buf)
}
