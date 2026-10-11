use argon2::{Algorithm, Argon2, Params, Version};

use super::keys::{Kek, KEY_LEN, SALT_LEN};
use crate::error::{Error, Result};

/// Seconde recommandation du RFC 9106 : Argon2id, 64 Mio, 3 passes, parallélisme 4.
pub const RECOMMENDED_MEMORY_KIB: u32 = 64 * 1024;
pub const RECOMMENDED_ITERATIONS: u32 = 3;
pub const RECOMMENDED_PARALLELISM: u32 = 4;

/// Plafonds de cette implémentation, pour refuser un en-tête hostile qui
/// demanderait une dérivation interminable. Un coffre légitime reste en dessous.
const MAX_MEMORY_KIB: u32 = 256 * 1024;
const MAX_ITERATIONS: u32 = 8;
const MAX_PARALLELISM: u32 = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KdfParams {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl KdfParams {
    pub fn recommended() -> Self {
        Self {
            memory_kib: RECOMMENDED_MEMORY_KIB,
            iterations: RECOMMENDED_ITERATIONS,
            parallelism: RECOMMENDED_PARALLELISM,
        }
    }

    /// Paramètres réels mais rapides, réservés aux tests automatisés.
    /// L'application ne les utilise jamais pour un coffre créé par l'interface.
    pub fn for_tests() -> Self {
        Self {
            memory_kib: 1024,
            iterations: 1,
            parallelism: 1,
        }
    }
}

pub fn derive_kek(password: &[u8], salt: &[u8], params: &KdfParams) -> Result<Kek> {
    if salt.len() != SALT_LEN {
        return Err(Error::Damaged);
    }
    if params.memory_kib > MAX_MEMORY_KIB
        || params.iterations > MAX_ITERATIONS
        || params.parallelism > MAX_PARALLELISM
        || params.iterations == 0
        || params.parallelism == 0
    {
        return Err(Error::Data);
    }

    let argon_params = Params::new(
        params.memory_kib,
        params.iterations,
        params.parallelism,
        Some(KEY_LEN),
    )
    .map_err(|_| Error::Damaged)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);
    let mut out = [0u8; KEY_LEN];
    argon
        .hash_password_into(password, salt, &mut out)
        .map_err(|_| Error::Data)?;
    Ok(Kek::new(out))
}
