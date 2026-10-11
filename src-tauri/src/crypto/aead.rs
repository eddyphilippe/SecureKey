use aead::Aead;
use aead::Payload;
use chacha20poly1305::{KeyInit, XChaCha20Poly1305};
use zeroize::Zeroize;

use super::keys::{random_bytes, Kek, MasterKey, KEY_LEN};
use crate::error::{Error, Result};

/// Libellé lié au coffre lors de l'enveloppe de la clé maître.
pub const WRAP_LABEL: &[u8] = b"securekey.v1.wrap";

const NONCE_LEN: usize = 24;

pub struct WrappedKey {
    pub nonce: [u8; NONCE_LEN],
    pub ciphertext: Vec<u8>,
}

pub fn wrap_master_key(
    kek: &Kek,
    master: &MasterKey,
    vault_id: &[u8; 16],
    format_version: u16,
) -> Result<WrappedKey> {
    let nonce_bytes: [u8; NONCE_LEN] = random_bytes()?;
    let nonce = aead_nonce(&nonce_bytes)?;
    let cipher = cipher_from(kek.as_bytes())?;
    let aad = wrap_aad(vault_id, format_version);
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: master.as_bytes(),
                aad: &aad,
            },
        )
        .map_err(|_| Error::Data)?;
    Ok(WrappedKey {
        nonce: nonce_bytes,
        ciphertext,
    })
}

pub fn unwrap_master_key(
    kek: &Kek,
    nonce: &[u8; NONCE_LEN],
    ciphertext: &[u8],
    vault_id: &[u8; 16],
    format_version: u16,
) -> Result<MasterKey> {
    let nonce_arr = aead_nonce(nonce)?;
    let cipher = cipher_from(kek.as_bytes())?;
    let aad = wrap_aad(vault_id, format_version);
    let mut plain = cipher
        .decrypt(
            &nonce_arr,
            Payload {
                msg: ciphertext,
                aad: &aad,
            },
        )
        .map_err(|_| Error::AuthFailed)?;
    if plain.len() != KEY_LEN {
        plain.zeroize();
        return Err(Error::Damaged);
    }
    let mut bytes = [0u8; KEY_LEN];
    bytes.copy_from_slice(&plain);
    plain.zeroize();
    Ok(MasterKey::new(bytes))
}

pub fn wrap_aad(vault_id: &[u8; 16], format_version: u16) -> Vec<u8> {
    let mut aad = Vec::with_capacity(WRAP_LABEL.len() + 16 + 2);
    aad.extend_from_slice(WRAP_LABEL);
    aad.extend_from_slice(vault_id);
    aad.extend_from_slice(&format_version.to_le_bytes());
    aad
}

fn cipher_from(key_bytes: &[u8; KEY_LEN]) -> Result<XChaCha20Poly1305> {
    let key = aead::Key::<XChaCha20Poly1305>::try_from(key_bytes.as_slice()).map_err(|_| Error::Data)?;
    Ok(XChaCha20Poly1305::new(&key))
}

fn aead_nonce(bytes: &[u8; NONCE_LEN]) -> Result<aead::Nonce<XChaCha20Poly1305>> {
    aead::Nonce::<XChaCha20Poly1305>::try_from(bytes.as_slice()).map_err(|_| Error::Data)
}
